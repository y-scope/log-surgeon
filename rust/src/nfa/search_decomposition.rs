use std::collections::BTreeMap;
use std::num::NonZero;
use std::sync::Arc;

use super::*;
use crate::search::SymbolicChar;

#[derive(Debug, Clone)]
pub struct Path(Vec<PathComponent>);

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PathComponent {
	Literal(Vec<SymbolicChar>),
	Capture {
		rule_idx: RuleIdx,
		maybe_sub_rule_id: Option<NonZero<u16>>,
		contents: Vec<SymbolicChar>,
	},
}

#[derive(Debug, Clone, Copy)]
pub struct TarjanSccData {
	pub index: Option<usize>,
	pub low_link: usize,
	pub on_stack: bool,
	pub scc: usize,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum PathEdge {
	Literal(char),
	Capture {
		sub_rule_id: NonZero<u16>,
		/// Exists for debugging.
		qualified_name: Arc<str>,
		is_start: bool,
	},
	/// Search query allows for any character.
	/// TODO... explain
	QueryWildcard,
	/// Schema pattern allows for any character.
	PatternWildcard,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct PartialPath {
	edges: Vec<PathEdge>,
}

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

impl std::fmt::Display for Path {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for part in self.0.iter() {
			part.fmt(fmt)?;
		}
		Ok(())
	}
}

impl PartialPath {
	fn new(edges: Vec<PathEdge>) -> Self {
		Self { edges }
	}

	fn push(&mut self, edge: PathEdge) {
		if edge.is_wildcard()
			&& let Some(last) = self.edges.last()
			&& last.is_wildcard()
		{
			return;
		}
		self.edges.push(edge);
	}

	fn iter(&self) -> std::slice::Iter<'_, PathEdge> {
		self.edges.iter()
	}
}

impl Extend<PathEdge> for PartialPath {
	fn extend<T>(&mut self, iter: T)
	where
		T: IntoIterator<Item = PathEdge>,
	{
		let iter: T::IntoIter = iter.into_iter();
		self.edges.reserve(match iter.size_hint() {
			(_, Some(n)) => n,
			(n, None) => n,
		});
		for edge in iter {
			self.push(edge);
		}
	}
}

impl Path {
	pub fn iter(&self) -> std::slice::Iter<'_, PathComponent> {
		self.0.iter()
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn first(&self) -> Option<&PathComponent> {
		self.0.first()
	}

	pub fn last(&self) -> Option<&PathComponent> {
		self.0.last()
	}

	fn new(components: Vec<PathComponent>) -> Self {
		Self(components)
	}

	fn invariants(components: &Vec<PathComponent>) {
		#[derive(Debug, Eq, PartialEq)]
		enum Kind {
			Start,
			Literal,
			Capture,
		}

		let mut last: Kind = Kind::Start;
		for component in components.iter() {
			let kind: Kind = match component {
				PathComponent::Literal(_) => Kind::Literal,
				PathComponent::Capture { .. } => Kind::Capture,
			};
			assert_ne!(kind, last);
			last = kind;

			match component {
				PathComponent::Literal(contents) | PathComponent::Capture { contents, .. } => {
					let mut last_was_star: bool = false;
					for &ch in contents.iter() {
						if ch == SymbolicChar::WildcardStar {
							assert!(!last_was_star);
							last_was_star = true;
						} else {
							last_was_star = false;
						}
					}
				},
			}
		}
	}
}

impl std::fmt::Display for PathComponent {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Literal(symbols) => {
				for &ch in symbols.iter() {
					match ch {
						SymbolicChar::Literal(ch) => {
							match ch {
								'(' | ')' | '*' | '\\' => {
									fmt.write_str("\\")?;
								},
								_ => (),
							}
							ch.fmt(fmt)?;
						},
						SymbolicChar::WildcardStar => {
							'*'.fmt(fmt)?;
						},
						SymbolicChar::WildcardOne => {
							unreachable!();
						},
					}
				}
			},
			Self::Capture {
				rule_idx: _,
				maybe_sub_rule_id,
				contents,
			} => {
				// TODO
				fmt.write_fmt(format_args!("(?<{maybe_sub_rule_id:?}>{contents:?})"))?;
			},
		}
		Ok(())
	}
}

impl PathEdge {
	fn is_wildcard(&self) -> bool {
		matches!(self, Self::QueryWildcard | Self::PatternWildcard)
	}
}

impl Tnfa {
	pub fn intersect(&self, other: &Self) -> Self {
		let begin: NfaIdx = NfaIdx::BEGIN;

		let mut stack: Vec<(StatePair<'_>, NfaIdx)> = vec![(StatePair::new(self, other, begin, begin), begin)];
		let mut seen: BTreeMap<StatePair<'_>, NfaIdx> = BTreeMap::from_iter(stack.iter().copied());

		let mut intersection: Self = Self {
			states: vec![NfaState {
				idx: NfaIdx::BEGIN,
				name: Cow::Borrowed("begin"),
				transitions: Transitions::Spontaneous(Vec::new()),
				maybe_accepts_for_rule: None,
			}],
			tags: Vec::new(),
		};

		while let Some((pair, state)) = stack.pop() {
			if let Some(rule) = pair.state1().maybe_accepts_for_rule
				&& pair.state2().is_accepting()
			{
				intersection[state].maybe_accepts_for_rule = Some(rule);
				assert_eq!(intersection[state].transitions.len(), 0);
				continue;
			}

			let mut lookup_state = |target, combined: &mut Self| {
				*seen.entry(target).or_insert_with(|| {
					let next: NfaIdx = combined.new_state(format!("({}, {})", target.states.0, target.states.1));
					stack.push((target, next));
					next
				})
			};

			// TODO: nicer way to combine the branches?
			match (&pair.state1().transitions, &pair.state2().transitions) {
				(Transitions::Spontaneous(transitions1), Transitions::Spontaneous(transitions2)) => {
					if !transitions1.is_empty() {
						intersection[state].transitions = Transitions::Spontaneous(
							transitions1
								.iter()
								.map(|spontaneous1| {
									let next: NfaIdx = lookup_state(
										StatePair::new(self, other, spontaneous1.target, pair.states.1),
										&mut intersection,
									);
									SpontaneousTransition {
										kind: spontaneous1.kind.clone(),
										target: next,
									}
								})
								.collect::<Vec<_>>(),
						);
					} else {
						intersection[state].transitions = Transitions::Spontaneous(
							transitions2
								.iter()
								.map(|spontaneous2| {
									assert_eq!(spontaneous2.kind, SpontaneousTransitionKind::Epsilon);
									let target: NfaIdx = lookup_state(
										StatePair::new(self, other, pair.states.0, spontaneous2.target),
										&mut intersection,
									);
									SpontaneousTransition {
										kind: SpontaneousTransitionKind::Epsilon,
										target,
									}
								})
								.collect::<Vec<_>>(),
						);
					}
				},
				(Transitions::Spontaneous(transitions1), _) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions1
							.iter()
							.map(|spontaneous1| {
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, spontaneous1.target, pair.states.1),
									&mut intersection,
								);
								SpontaneousTransition {
									kind: spontaneous1.kind.clone(),
									target: next,
								}
							})
							.collect::<Vec<_>>(),
					);
				},
				(_, Transitions::Spontaneous(transitions2)) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions2
							.iter()
							.map(|spontaneous2| {
								assert_eq!(spontaneous2.kind, SpontaneousTransitionKind::Epsilon);
								let target: NfaIdx = lookup_state(
									StatePair::new(self, other, pair.states.0, spontaneous2.target),
									&mut intersection,
								);
								SpontaneousTransition {
									kind: SpontaneousTransitionKind::Epsilon,
									target,
								}
							})
							.collect::<Vec<_>>(),
					);
				},
				(Transitions::Interval(transitions1), Transitions::Interval(transitions2)) => {
					let mut combined: IntervalTree<u32, NfaIdx> = IntervalTree::new();
					for (interval1, &target1) in transitions1.iter() {
						for (interval2, &target2) in transitions2.iter() {
							if interval2.start() != interval2.end() {
								// Query wildcard.
								assert_eq!((interval2.start(), interval2.end()), (0, u32::from(char::MAX)));
								let next: NfaIdx =
									lookup_state(StatePair::new(self, other, target1, target2), &mut intersection);
								combined.insert(Interval::new(0, u32::MAX), next, PolicyUnique);
							} else {
								// Query literal character.
								assert_eq!(interval2.start(), interval2.end());
								let Some(overlap): Option<Interval<u32>> = interval1.overlap(&interval2) else {
									continue;
								};
								assert_eq!(overlap, interval2);
								let next: NfaIdx =
									lookup_state(StatePair::new(self, other, target1, target2), &mut intersection);
								combined.insert(overlap, next, PolicyUnique);
							}
						}
					}
					intersection[state].transitions = Transitions::Interval(combined);
				},
			}
		}

		let can_accept: Vec<bool> = intersection.compute_live_states();
		for state in intersection.states.iter_mut() {
			match &mut state.transitions {
				Transitions::Interval(transitions) => {
					transitions.retain(|&(_interval, target)| can_accept[target.0]);
				},
				Transitions::Spontaneous(transitions) => {
					transitions.retain(|transition| can_accept[transition.target.0]);
				},
			}
		}

		intersection
	}

	pub fn can_accept(&self) -> bool {
		self.compute_live_states()[0]
	}

	/// States that can reach an accepting state.
	fn compute_live_states(&self) -> Vec<bool> {
		let mut acceptable: Vec<bool> = vec![false; self.states.len()];

		for state in self.states.iter() {
			if state.is_accepting() {
				acceptable[state.idx.0] = true;
			}
		}

		let mut changed: bool = true;
		while changed {
			changed = false;
			for state in self.states.iter() {
				for target_idx in state.transitions.successors() {
					if acceptable[target_idx.0] {
						let old_can_accept: bool = std::mem::replace(&mut acceptable[state.idx.0], true);
						if !old_can_accept {
							changed = true;
						}
					}
				}
			}
		}

		acceptable
	}

	pub fn tarjan_scc(&self) -> (Vec<Vec<NfaIdx>>, Vec<TarjanSccData>, Vec<NfaIdx>) {
		let mut data: Vec<TarjanSccData> = vec![
			TarjanSccData {
				index: None,
				low_link: 0,
				on_stack: false,
				scc: usize::MAX,
			};
			self.states.len()
		];

		let mut indices: Vec<NfaIdx> = Vec::with_capacity(self.states.len());
		let mut stack: Vec<NfaIdx> = Vec::new();

		let mut sccs: Vec<Vec<NfaIdx>> = Vec::new();

		for state in self.states.iter() {
			self.strong_connect(&mut data, &mut stack, &mut indices, state.idx, &mut sccs);
		}

		sccs.reverse();
		for data in data.iter_mut() {
			data.scc = sccs.len() - data.scc - 1;
		}

		(sccs, data, indices)
	}

	fn strong_connect(
		&self,
		data: &mut [TarjanSccData],
		stack: &mut Vec<NfaIdx>,
		indices: &mut Vec<NfaIdx>,
		idx: NfaIdx,
		sccs: &mut Vec<Vec<NfaIdx>>,
	) {
		if data[idx.0].index.is_some() {
			return;
		}

		let i: usize = indices.len();
		data[idx.0] = TarjanSccData {
			index: Some(i),
			low_link: i,
			on_stack: true,
			scc: usize::MAX,
		};
		stack.push(idx);
		indices.push(idx);

		let state: &NfaState = &self[idx];
		for jdx in state.transitions.successors() {
			if let Some(j) = data[jdx.0].index {
				if data[jdx.0].on_stack {
					data[idx.0].low_link = std::cmp::min(data[idx.0].low_link, j);
				}
			} else {
				self.strong_connect(data, stack, indices, jdx, sccs);
				data[idx.0].low_link = std::cmp::min(data[idx.0].low_link, data[jdx.0].low_link);
			}
		}

		if data[idx.0].low_link == i {
			let mut scc: Vec<NfaIdx> = Vec::new();
			loop {
				let jdx: NfaIdx = stack.pop().unwrap();
				data[jdx.0].on_stack = false;
				data[jdx.0].scc = sccs.len();
				scc.push(jdx);
				if jdx == idx {
					break;
				}
			}
			sccs.push(scc);
		}
	}

	pub fn compute_paths(&self) -> (Vec<Path>, Vec<RuleIdx>) {
		let (sccs, data, _indices): (Vec<Vec<NfaIdx>>, Vec<TarjanSccData>, Vec<NfaIdx>) = self.tarjan_scc();

		let mut cache: Vec<Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>> = vec![None; self.states.len()];

		if self[NfaIdx::BEGIN].transitions.len() == 0 {
			return (Vec::new(), Vec::new());
		}

		let prefix: PartialPath = PartialPath::new(Vec::new());
		let mut seen_prefix: BTreeMap<NfaIdx, BTreeSet<PartialPath>> = BTreeMap::new();
		let mut finished: Vec<(PartialPath, RuleIdx)> = Vec::new();
		self.compute_paths_internal(
			&self[NfaIdx::BEGIN],
			&prefix,
			&mut seen_prefix,
			&sccs,
			&data,
			&mut cache,
			&mut finished,
		);
		let mut finished: Vec<Vec<PathComponent>> = Vec::new();
		let mut rules_potentially_without_captures: Vec<RuleIdx> = Vec::new();
		let paths: &Vec<(NfaIdx, PartialPath, RuleIdx)> = cache[0].as_ref().unwrap();

		for (_, edges, rule_idx) in paths.iter() {
			if !edges.iter().any(|e| matches!(e, PathEdge::Capture { .. })) {
				rules_potentially_without_captures.push(*rule_idx);
			}

			let mut path: Vec<PathComponent> = Vec::new();
			let mut maybe_capture: Option<NonZero<u16>> = None;
			let mut symbols: Vec<SymbolicChar> = Vec::new();
			// let mut buf: String = String::new();

			// TODO no captures -> capture full
			for edge in edges.iter().rev() {
				match edge {
					&PathEdge::Literal(ch) => {
						symbols.push(SymbolicChar::Literal(ch));
					},
					PathEdge::Capture {
						sub_rule_id, is_start, ..
					} => {
						if let Some(current_rule) = maybe_capture {
							// Closing capture.
							assert!(!is_start);
							assert_eq!(sub_rule_id, &current_rule);
							assert!(!symbols.is_empty());
							if symbols != &[SymbolicChar::WildcardStar] {
								path.push(PathComponent::Capture {
									rule_idx: *rule_idx,
									maybe_sub_rule_id: Some(*sub_rule_id),
									contents: symbols,
								});
							}
							symbols = Vec::new();
							maybe_capture = None;
						} else {
							// Starting capture.
							assert!(is_start);
							if !symbols.is_empty() {
								if let Some(PathComponent::Literal(last)) = path.last_mut() {
									if (last.last() == Some(&SymbolicChar::WildcardStar))
										&& (symbols.first() == Some(&SymbolicChar::WildcardStar))
									{
										last.extend(symbols.drain(1..));
									} else {
										last.extend(symbols.into_iter());
									}
								} else {
									path.push(PathComponent::Literal(symbols));
								}
							}
							symbols = Vec::new();
							maybe_capture = Some(*sub_rule_id);
						}
					},
					PathEdge::QueryWildcard | PathEdge::PatternWildcard => {
						symbols.push(SymbolicChar::WildcardStar);
					},
				}
			}
			assert_eq!(maybe_capture, None);
			// TODO duped above?
			if !symbols.is_empty() {
				if let Some(PathComponent::Literal(last)) = path.last_mut() {
					if (last.last() == Some(&SymbolicChar::WildcardStar))
						&& (symbols.first() == Some(&SymbolicChar::WildcardStar))
					{
						last.extend(symbols.drain(1..));
					} else {
						last.extend(symbols.into_iter());
					}
				} else {
					path.push(PathComponent::Literal(symbols));
				}
			}

			finished.push(path);
		}
		finished.iter().for_each(Path::invariants);

		rules_potentially_without_captures.sort();
		rules_potentially_without_captures.dedup();

		(
			finished.into_iter().map(Path::new).collect::<Vec<_>>(),
			rules_potentially_without_captures,
		)
	}

	fn compute_paths_internal<'a>(
		&self,
		entry: &NfaState,
		prefix: &PartialPath,
		seen_prefix: &mut BTreeMap<NfaIdx, BTreeSet<PartialPath>>,
		sccs: &[Vec<NfaIdx>],
		data: &[TarjanSccData],
		cache: &'a mut [Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>],
		finished: &mut Vec<(PartialPath, RuleIdx)>,
	) -> Vec<(NfaIdx, PartialPath, RuleIdx)> {
		// ) {
		if seen_prefix
			.entry(entry.idx)
			.or_insert_with(BTreeSet::new)
			.contains(prefix)
		{
			return Vec::new();
		}
		seen_prefix.get_mut(&entry.idx).unwrap().insert(prefix.clone());
		if let Some(cached) = cache[entry.idx.0].as_ref() {
			return cached.clone();
			// return;
		}

		let mut paths: Vec<(NfaIdx, PartialPath, RuleIdx)> = Vec::new();
		let scc: &Vec<NfaIdx> = &sccs[data[entry.idx.0].scc];
		assert!(!scc.is_empty());
		if let Some(rule) = entry.maybe_accepts_for_rule {
			finished.push((prefix.clone(), rule));
			return cache[entry.idx.0]
				.insert(vec![(entry.idx, PartialPath::new(Vec::new()), rule)])
				.clone();
			// cache[entry.idx.0].insert(vec![(entry.idx, Vec::new(), rule)]);
			// return;
		}
		if scc.len() == 1 {
			assert!(entry.transitions.len() > 0);
			match &entry.transitions {
				Transitions::Interval(transitions) => {
					assert_eq!(transitions.len(), 1);
					// println!("- state {} has {} interval transitions", entry.idx, transitions.len());
					for ((interval, &target), mut prefix) in transitions
						.iter()
						.zip(std::iter::repeat_n(prefix.clone(), transitions.len()))
					{
						assert!(data[target.0].index > data[entry.idx.0].index);
						assert!(data[target.0].scc > data[entry.idx.0].scc);
						let ch: PathEdge = if interval.start() == interval.end() {
							let ch: char = char::try_from(interval.start()).unwrap();
							PathEdge::Literal(ch)
						} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
							PathEdge::QueryWildcard
						} else {
							PathEdge::PatternWildcard
						};
						prefix.push(ch.clone());
						// self.compute_paths_internal(&self[target], &prefix, seen_prefix, sccs, data, cache, finished);

						let mut partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self
							.compute_paths_internal(&self[target], &prefix, seen_prefix, sccs, data, cache, finished);
						for (_state, path, _rule) in partials.iter_mut() {
							path.push(ch.clone());
						}
						partials.sort();
						partials.dedup();
						assert!(cache[entry.idx.0].is_none());
						return cache[entry.idx.0].insert(partials).clone();
					}
					unreachable!();
				},
				Transitions::Spontaneous(transitions) => {
					// println!(
					// 	"- state {} has {} spontaneous transitions",
					// 	entry.idx,
					// 	transitions.len()
					// );
					for transition in transitions.iter() {
						assert!(data[transition.target.0].scc > data[entry.idx.0].scc);
						match &transition.kind {
							SpontaneousTransitionKind::Positive(
								tag @ (Tag::StartCapture(sub_rule) | Tag::StopCapture(sub_rule)),
							) => {
								if sub_rule.is_leaf() {
									let is_start: bool = matches!(tag, Tag::StartCapture(_));
									// paths.push((
									// 	&self[transition.target],
									// 	vec![PathEdge::Capture {
									// 		sub_rule: sub_rule.clone(),
									// 		start,
									// 	}],
									// ));
									let mut prefix: PartialPath = prefix.clone();
									let edge: PathEdge = PathEdge::Capture {
										sub_rule_id: sub_rule.id,
										qualified_name: sub_rule.qualified_name.clone(),
										is_start,
									};
									prefix.push(edge.clone());
									let mut partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self
										.compute_paths_internal(
											&self[transition.target],
											&prefix,
											seen_prefix,
											sccs,
											data,
											cache,
											finished,
										);
									for (_state, path, _rule) in partials.iter_mut() {
										path.push(edge.clone());
									}
									paths.extend(partials.into_iter());
									continue;
								}
							},
							SpontaneousTransitionKind::Negative(_) => (),
							SpontaneousTransitionKind::Epsilon => (),
						}
						let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self.compute_paths_internal(
							&self[transition.target],
							prefix,
							seen_prefix,
							sccs,
							data,
							cache,
							finished,
						);
						paths.extend(partials.into_iter());
						// paths.push((&self[transition.target], Vec::new()));
					}
					paths.sort();
					paths.dedup();
					assert!(cache[entry.idx.0].is_none());
					return cache[entry.idx.0].insert(paths).clone();
				},
			}
		} else {
			return self.compute_scc_path(entry, prefix, seen_prefix, &sccs, &data, cache, finished);
		}
	}

	fn compute_scc_path<'a>(
		&self,
		entry: &NfaState,
		prefix: &PartialPath,
		seen_prefix: &mut BTreeMap<NfaIdx, BTreeSet<PartialPath>>,
		sccs: &[Vec<NfaIdx>],
		data: &[TarjanSccData],
		cache: &'a mut [Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>],
		finished: &mut Vec<(PartialPath, RuleIdx)>,
	) -> Vec<(NfaIdx, PartialPath, RuleIdx)> {
		let mut scc_finished: Vec<(NfaIdx, PartialPath, RuleIdx)> = Vec::new();

		let mut stack: Vec<(&NfaState, PartialPath, BTreeSet<NfaIdx>)> = vec![(
			entry,
			PartialPath::new(vec![PathEdge::PatternWildcard]),
			BTreeSet::from([entry.idx]),
		)];

		while let Some((state, path, seen)) = stack.pop() {
			assert!(state.transitions.len() > 0);
			match &state.transitions {
				Transitions::Interval(transitions) => {
					assert_eq!(transitions.len(), 1);
					for ((interval, &target), (mut path, mut seen)) in transitions
						.iter()
						.zip(std::iter::repeat_n((path, seen), transitions.len()))
					{
						assert_eq!(data[target.0].scc, data[entry.idx.0].scc);
						let inserted: bool = seen.insert(target);
						assert!(inserted);
						let ch: PathEdge = if interval.start() == interval.end() {
							let ch: char = char::try_from(interval.start()).unwrap();
							PathEdge::Literal(ch)
						} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
							PathEdge::QueryWildcard
						} else {
							PathEdge::PatternWildcard
						};
						path.push(ch);
						stack.push((&self[target], path, seen));
					}
				},
				Transitions::Spontaneous(transitions) => {
					for (transition, (mut path, mut seen)) in transitions
						.iter()
						.zip(std::iter::repeat_n((path, seen), transitions.len()))
					{
						assert!(data[transition.target.0].scc >= data[entry.idx.0].scc);
						if data[transition.target.0].scc != data[entry.idx.0].scc {
							let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self.compute_paths_internal(
								&self[transition.target],
								prefix,
								seen_prefix,
								sccs,
								data,
								cache,
								finished,
							);
							for (end, mut remaining, rule) in partials.into_iter() {
								remaining.push(PathEdge::PatternWildcard);
								remaining.extend(path.iter().rev().cloned());
								scc_finished.push((end, remaining, rule));
							}
							continue;
						}
						if seen.contains(&transition.target) {
							continue;
						}
						let inserted: bool = seen.insert(transition.target);
						assert!(inserted);
						match &transition.kind {
							SpontaneousTransitionKind::Positive(
								tag @ (Tag::StartCapture(sub_rule) | Tag::StopCapture(sub_rule)),
							) => {
								if sub_rule.is_leaf() {
									path.push(PathEdge::Capture {
										sub_rule_id: sub_rule.id,
										qualified_name: sub_rule.qualified_name.clone(),
										is_start: matches!(tag, Tag::StartCapture(_)),
									});
									stack.push((&self[transition.target], path, seen));
									continue;
								}
							},
							SpontaneousTransitionKind::Negative(_) => (),
							SpontaneousTransitionKind::Epsilon => (),
						}
						stack.push((&self[transition.target], path, seen));
					}
				},
			}
		}
		scc_finished.sort();
		scc_finished.dedup();
		assert!(cache[entry.idx.0].is_none());
		cache[entry.idx.0].insert(scc_finished).clone()
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn nfa_decomp() {
		// let paths = nfa_for("abc(?<hello>(foo*|bar|ba*z)(qux|quux))def").to_wildcard_string();
		// let paths = nfa_for("abc((?<var>foo*|bar|ba*z)0(qux|quux)*)def").to_wildcard_string();
		let nfa = nfa_for(r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)");
		let search = nfa_for("abc@.*mail.*example.*");
		// nfa.to_wildcard_string();
		// search.to_wildcard_string();
		// let nfa = nfa_for(r"@((?<parts>\w+)\.)*(?<tld>\w+)");
		// let nfa = nfa_for(r"@\.(?<tld>\w+)");
		// let search = nfa_for("@.*mail.*example.*");
		// let nfa = nfa_for(r"(?<hello>a.)*");
		// let search = nfa_for("(ab)*");
		// println!("{}", nfa.intersect(&search).to_dot_output());
		// return;
		let (paths, _) = nfa.intersect(&search).compute_paths();
		let mut paths = paths.iter().map(ToString::to_string).collect::<Vec<_>>();
		paths.sort();
		paths.dedup();
		println!("===");
		for path in paths.iter() {
			println!("- {path}");
		}
		println!("=== {} paths", paths.len());
	}

	fn nfa_for(pattern: &str) -> Tnfa {
		let regex: Regex = Regex::from_pattern(pattern).unwrap().inner;
		Tnfa::for_regex(&regex)
	}
}
