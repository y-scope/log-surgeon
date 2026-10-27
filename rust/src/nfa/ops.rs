use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::graph::Csr;
use crate::graph::TarjanSccs;
use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::PolicyUnique;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::Tnfa;
use crate::nfa::Transitions;
use crate::parsing_spec::RuleIdx;

/// Used for [`Tnfa::intersect`].
#[derive(Debug, Clone, Copy)]
struct StatePair<'a> {
	nfas: (&'a Tnfa, &'a Tnfa),
	states: (NfaIdx, NfaIdx),
}

impl Eq for StatePair<'_> {}

impl Ord for StatePair<'_> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.states.cmp(&other.states)
	}
}

impl PartialEq for StatePair<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other).is_eq()
	}
}

impl PartialOrd for StatePair<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Tnfa {
	/// Splice `other` into `self`; as if
	/// `self[current]` spontaneously transitions to `other[NfaIdx::BEGIN]`,
	/// and `other`'s accepting states spontaneously transition to `self[target]`.
	///
	/// Currently only used to splice the intersection of encoding NFAs when constructing by regex.
	pub fn splice(&mut self, other: &Tnfa, current: NfaIdx, target: NfaIdx) {
		assert!(!other.states.is_empty());
		let offset: usize = self.states.len();

		assert_eq!(self[current].transitions.len(), 0);
		self[current].transitions = Transitions::Spontaneous(vec![NfaIdx(offset)]);

		self.states.extend(other.states.iter().map(|other_state| {
			let mut state: NfaState = other_state.offset_idxes(offset);
			if state.is_accepting() {
				assert_eq!(state.transitions.len(), 0);
				state.maybe_accepting_data = None;
				state.transitions = Transitions::Spontaneous(vec![target]);
			} else {
				assert!(
					!matches!(state.transitions, Transitions::Tagged { .. }),
					"encoding pattern should not have captures"
				);
			}
			state
		}));
	}
}

impl Tnfa {
	/// Return a new TNFA representing an intersection of `self` and `other`; conceptually:
	///
	/// - Each state in the intersection corresponds to a pair of states from `self` and `other`.
	/// - If `a` and `b` are states from `self`, `c` and `d` are states from `other`,
	///   with edges `a -> b` and `u -> v`:
	///   - If the two source edges are identical/overlapping symbol transitions,
	///     then there is an edge in the intersection `(a, u) -> (b, v)`
	///     for the corresponding input symbol(s).
	///   - If `a -> b` is a spontaneous transition,
	///     then there is an spontaneous transition edge in the intersection `(a, u) -> (b, u)`,
	///     and vice versa for `u -> v`.
	///   - If `a -> b` is a tagged transition,
	///     then there is a tagged transition edge in the intersection `(a, u) -> (b, u)`
	///     for the corresponding tag.
	///
	/// In particular, `other` should not contain tagged transitions,
	/// and this function will panic if so.
	///
	/// Only reachable state pairs are considered/created,
	/// and this function also filters out states/paths that cannot lead to an accepting state.
	///
	/// Furthermore, note that this function "exhausts" spontaneous transitions from `self`
	/// before considering/creating intersection states for spontaneous transitions from `other`;
	/// if `a -> b` and `u -> v` are as above and both spontaneous transitions,
	/// this intersect only contains `(a, u) -> (b, u)` and not `(a, u) -> (a, v)`.
	///
	/// Because of this asymmetry, empty [`NfaState::transitions`]s
	/// should be represented with an empty [`Transitions::Interval`]
	/// rather than an empty [`Transitions::Spontaneous`];
	/// otherwise, the case where both `a` and `u` as above "have" spontaneous transitions,
	/// but `a` actually has no outgoing transitions,
	/// needs to be handled separately, i.e. as `(a, u) -> (a, v)`.
	///
	/// When `FOR_SEARCH` set, non-literal interval transitions from `other`,
	/// i.e. those corresponding to a wildcard in the search,
	/// remain "maximal",
	/// in order to differentiate between literal edges that "come from"
	/// a parsing specification pattern (i.e. "would necessarily match"),
	/// as opposed to the search (i.e. "meaningful search value").
	///
	/// The intersection is always taken *to the end*: a state accepts only when both sides accept,
	/// so a match must consume all of `self` and all of `other`.
	/// A search that should be free to stop early says so in `other` itself,
	/// by ending in a wildcard -- see [`crate::search::SearchString::search_by_log_shapes`],
	/// where deciding how much of `self` even needs building
	/// is the caller's job rather than this function's.
	pub fn intersect<const FOR_SEARCH: bool>(&self, other: &Self) -> Self {
		let begin: NfaIdx = NfaIdx::BEGIN;

		let mut stack: Vec<(StatePair<'_>, NfaIdx)> =
			vec![(StatePair::new(self, other, begin, begin), begin)];
		// Note 2026-09-16 (de50fcd316304679841d0687cfac883d5e09287e):
		// No noticeable performance improvement by using `rustc::FxHashMap`.
		let mut seen: BTreeMap<StatePair<'_>, NfaIdx> = BTreeMap::from_iter(stack.iter().copied());

		let mut intersection: Self = Self {
			states: vec![NfaState {
				idx: NfaIdx::BEGIN,
				name: Cow::Borrowed("begin"),
				transitions: Transitions::Spontaneous(Vec::new()),
				maybe_accepting_data: None,
			}],
			tags: self.tags.clone(),
		};

		while let Some((pair, state)) = stack.pop() {
			if let Some(accepting_data) = pair.state1().maybe_accepting_data
				&& pair.state2().is_accepting()
			{
				intersection[state].maybe_accepting_data = Some(accepting_data);
				assert_eq!(intersection[state].transitions.len(), 0);
				continue;
			}

			let mut lookup_state = |target, combined: &mut Self| {
				*seen.entry(target).or_insert_with(|| {
					// Note 2026-09-16 (de50fcd316304679841d0687cfac883d5e09287e):
					// ~5% improvement not giving formatted (allocating) name.
					let next: NfaIdx = combined.new_state("intersection state");
					stack.push((target, next));
					next
				})
			};

			match (&pair.state1().transitions, &pair.state2().transitions) {
				(_, Transitions::Tagged { .. }) => {
					panic!("the right hand side of `Tnfa::intersect` should not contain tags");
				},
				(
					Transitions::Tagged {
						tag,
						positive,
						target,
					},
					_,
				) => {
					let next: NfaIdx = lookup_state(
						StatePair::new(self, other, *target, pair.states.1),
						&mut intersection,
					);
					intersection[state].transitions = Transitions::Tagged {
						tag: tag.clone(),
						positive: *positive,
						target: next,
					};
				},
				(Transitions::Spontaneous(transitions1), _) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions1
							.iter()
							.map(|&target1| {
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, target1, pair.states.1),
									&mut intersection,
								);
								next
							})
							.collect::<Vec<_>>(),
					);
				},
				(_, Transitions::Spontaneous(transitions2)) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions2
							.iter()
							.map(|&target2| {
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, pair.states.0, target2),
									&mut intersection,
								);
								next
							})
							.collect::<Vec<_>>(),
					);
				},
				(Transitions::Interval(transitions1), Transitions::Interval(transitions2)) => {
					let mut combined: IntervalTree<u32, NfaIdx> = IntervalTree::new();
					for (interval1, &target1) in transitions1.iter() {
						for (interval2, &target2) in transitions2.iter() {
							let Some(mut overlap): Option<Interval<u32>> =
								interval1.overlap(&interval2)
							else {
								continue;
							};
							let next: NfaIdx = lookup_state(
								StatePair::new(self, other, target1, target2),
								&mut intersection,
							);
							if FOR_SEARCH && (interval2.start() != interval2.end()) {
								overlap = Interval::new(0, u32::MAX);
							}
							combined.insert(overlap, next, PolicyUnique);
						}
					}
					intersection[state].transitions = Transitions::Interval(combined);
				},
			}
		}

		now!(t0);
		let can_accept: Vec<bool> = intersection.compute_live_states();
		now!(t1);
		trace!(
			"- computing live states for {} states took {}",
			intersection.states.len(),
			t1.duration_since(t0).as_millis(),
		);
		if !can_accept[0] {
			return Self::new();
		}
		let mut reachable_states: usize = 0;
		let new_state_indices: Vec<usize> = Vec::from_iter(can_accept.iter().map(|&b| {
			if b {
				let n: usize = reachable_states;
				reachable_states += 1;
				n
			} else {
				usize::MAX
			}
		}));
		for (old_index, state) in intersection.states.iter_mut().enumerate() {
			let new_index: usize = new_state_indices[old_index];

			state.idx = NfaIdx(new_index);

			if new_index == usize::MAX {
				continue;
			}

			match &mut state.transitions {
				Transitions::Interval(transitions) => {
					transitions.retain(|&(_interval, target)| can_accept[target.0]);
					for (_interval, target) in transitions.iter_mut() {
						*target = NfaIdx(new_state_indices[target.0]);
					}
				},
				Transitions::Spontaneous(transitions) => {
					transitions.retain(|&target| can_accept[target.0]);
					for target in transitions.iter_mut() {
						*target = NfaIdx(new_state_indices[target.0]);
					}
				},
				Transitions::Tagged { target, .. } => {
					assert_eq!(can_accept[old_index], can_accept[target.0]);
					*target = NfaIdx(new_state_indices[target.0]);
				},
			}
		}
		intersection
			.states
			.retain(|state| state.idx != NfaIdx(usize::MAX));

		if cfg!(debug_assertions) {
			assert!(
				intersection
					.compute_live_states()
					.into_iter()
					.all(|reachable| reachable)
			);
		}

		intersection
	}

	pub fn sccs(&self) -> TarjanSccs {
		TarjanSccs::tarjan_scc(&self.states, std::iter::once(0), |state| {
			state.transitions.successors().map(|idx| idx.0)
		})
	}

	/// `Tnfa`s constructed from regexes can always accept,
	/// but `Tnfa`s constructed from intersecting may not accept
	/// (i.e. the intersection of their accepting strings is empty).
	pub fn can_accept(&self) -> bool {
		self.compute_live_states()[0]
	}

	/// States that can reach an accepting state.
	fn compute_live_states(&self) -> Vec<bool> {
		// CSR for reversed edges (predecessors).
		let csr: Csr<()> = Csr::build(
			self.states.len(),
			self.states.iter().flat_map(|state| {
				state
					.transitions
					.successors()
					.map(move |target| (target.0, state.idx.0, ()))
			}),
		);

		let mut can_accept: Vec<bool> = vec![false; self.states.len()];
		let mut stack: Vec<usize> = Vec::new();
		for (i, state) in self.states.iter().enumerate() {
			if state.is_accepting() {
				can_accept[i] = true;
				stack.push(i);
			}
		}

		while let Some(i) = stack.pop() {
			for edge in csr[i].iter() {
				let predecessor: usize = edge.target;
				if !can_accept[predecessor] {
					can_accept[predecessor] = true;
					stack.push(predecessor);
				}
			}
		}

		can_accept
	}
}

impl Tnfa {
	/// Concatentate `self` with `other`; as if
	/// `self`'s accepting states spontaneously transition to `other`'s begin state.
	pub fn concat(&self, other: &Self) -> Tnfa {
		let mut new_states: Vec<NfaState> = self
			.states
			.iter()
			.cloned()
			.chain(
				other
					.states
					.iter()
					.map(|state| state.offset_idxes(self.states.len())),
			)
			.collect::<Vec<_>>();

		for my_state in new_states[..self.states.len()].iter_mut() {
			if let Some((rule_idx, _)) = my_state.maybe_accepting_data {
				assert_eq!(rule_idx, RuleIdx::NIL);
				assert_eq!(my_state.transitions.len(), 0);
				my_state.maybe_accepting_data = None;
				my_state.transitions = Transitions::Spontaneous(vec![NfaIdx(self.states.len())]);
			}
		}

		for other_state in new_states[self.states.len()..].iter_mut() {
			if let Some((rule_idx, _)) = other_state.maybe_accepting_data {
				assert_eq!(rule_idx, RuleIdx::NIL);
			}
		}

		Self {
			states: new_states,
			tags: &self.tags | &other.tags,
		}
	}

	/// Construct the alternation of `self` with `other`.
	pub fn alternate(&self, other: &Self) -> Self {
		let my_states: Vec<NfaState> = self
			.states
			.iter()
			.map(|state| state.offset_idxes(2))
			.collect::<Vec<_>>();
		let other_states: Vec<NfaState> = other
			.states
			.iter()
			.map(|state| state.offset_idxes(2 + self.states.len()))
			.collect::<Vec<_>>();

		let mut new_states: Vec<NfaState> =
			Vec::with_capacity(2 + self.states.len() + other_states.len());
		let end_idx: NfaIdx = NfaIdx(1);

		new_states.push(NfaState {
			idx: NfaIdx::BEGIN,
			name: Cow::Borrowed("begin"),
			transitions: Transitions::Spontaneous(vec![NfaIdx(2), NfaIdx(2 + self.states.len())]),
			maybe_accepting_data: None,
		});

		new_states.push(NfaState {
			idx: end_idx,
			name: Cow::Borrowed("end"),
			transitions: Transitions::Interval(IntervalTree::new()),
			maybe_accepting_data: Some((RuleIdx::NIL, None)),
		});

		new_states.extend(my_states.into_iter());
		new_states.extend(other_states.into_iter());

		for state in new_states[2..].iter_mut() {
			if state.is_accepting() {
				assert_eq!(state.transitions.len(), 0);
				state.maybe_accepting_data = None;
				state.transitions = Transitions::Spontaneous(vec![end_idx]);
			}
		}

		Self {
			states: new_states,
			tags: &self.tags | &other.tags,
		}
	}
}

impl NfaState {
	/// Shift all `NfaIdx`s ([`NfaState::idx`] and [`NfaState::transitions`] targets) by `offset`.
	fn offset_idxes(&self, offset: usize) -> Self {
		Self {
			idx: NfaIdx(offset + self.idx.0),
			name: self.name.clone(),
			transitions: {
				let mut transitions: Transitions = self.transitions.clone();
				match &mut transitions {
					Transitions::Interval(transitions) => {
						transitions.iter_mut().for_each(|(_interval, target)| {
							target.0 += offset;
						});
					},
					Transitions::Spontaneous(transitions) => {
						transitions.iter_mut().for_each(|target| {
							target.0 += offset;
						});
					},
					Transitions::Tagged { target, .. } => {
						target.0 += offset;
					},
				};
				transitions
			},
			maybe_accepting_data: self.maybe_accepting_data,
		}
	}
}

impl<'a> StatePair<'a> {
	fn new(nfa1: &'a Tnfa, nfa2: &'a Tnfa, state1: NfaIdx, state2: NfaIdx) -> Self {
		Self {
			nfas: (nfa1, nfa2),
			states: (state1, state2),
		}
	}

	fn state1(&self) -> &NfaState {
		&self.nfas.0[self.states.0]
	}

	fn state2(&self) -> &NfaState {
		&self.nfas.1[self.states.1]
	}
}
