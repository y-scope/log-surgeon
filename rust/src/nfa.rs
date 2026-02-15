use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZero;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::regex::Regex;
use crate::regex::RegexCapture;
use crate::schema::Rule;

#[derive(Debug)]
pub struct Nfa {
	states: Vec<NfaState>,
	tags: Vec<Tag>,
}

#[derive(Debug)]
pub struct NfaState {
	idx: NfaIdx,
	transitions: IntervalTree<u32, BTreeSet<NfaIdx>>,
	spontaneous: Vec<SpontaneousTransition>,
	/// Just to be cute, and for debugging, in that order.
	/// See [`Nfa::new_state`].
	name: Cow<'static, str>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct NfaIdx(usize);

impl std::fmt::Debug for NfaIdx {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.debug_tuple("NfaIdx").field(&(self.0 as isize)).finish()
	}
}

#[derive(Debug)]
pub struct SpontaneousTransition {
	pub kind: SpontaneousTransitionKind,
	pub target: NfaIdx,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum SpontaneousTransitionKind {
	Epsilon,
	Positive(Tag),
	Negative(Tag),
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum Tag {
	StartCapture(AutomataCapture),
	StopCapture(AutomataCapture),
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct AutomataCapture {
	/// Rule index from the schema.
	pub rule: usize,
	// /// Capture name from the rule/regex pattern.
	// /// Technically redundant (given a schema); exists for debugging.
	// pub name: String,
	pub regex: RegexCapture,
}

// impl std::fmt::Debug for AutomataCapture {
// 	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// 		if !fmt.alternate() {
// 			fmt.write_fmt(format_args!("CaptureInfo({}, {})", self.rule, self.name))
// 		} else {
// 			fmt.write_fmt(format_args!(
// 				"CaptureInfo({}, {}, {})",
// 				self.rule,
// 				self.id.map_or(0, NonZero::get),
// 				self.name
// 			))
// 		}
// 	}
// }

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct NfaSimulationData {
	captures: BTreeMap<AutomataCapture, (Vec<usize>, Vec<usize>)>,
}

impl Nfa {
	pub fn new() -> Self {
		Self {
			states: vec![NfaState::BEGIN],
			tags: Vec::new(),
		}
	}

	pub fn for_rules(rules: &[Rule]) -> Self {
		assert_ne!(rules.len(), 0);

		let mut nfa: Self = Self {
			states: vec![NfaState::BEGIN],
			tags: Vec::new(),
		};

		let mut tags: BTreeSet<Tag> = nfa.build(0, &rules[0].regex, NfaIdx::BEGIN, NfaIdx::end(0));

		for rule in rules[1..].iter() {
			let state: NfaIdx = nfa.new_state(format!("rule {} start", rule.name));
			// assert_eq!(state.0, rule.idx);
			nfa[NfaIdx::BEGIN].spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target: state,
			});
			tags = &tags | &nfa.build(rule.idx, &rule.regex, state, NfaIdx::end(rule.idx));
		}

		nfa.tags = tags.into_iter().collect::<Vec<_>>();

		nfa
	}

	/// - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
	/// - <https://arxiv.org/abs/2206.01398>
	///
	/// Algorithm 1.
	pub fn simulate(&self, input: &str) -> Option<usize> {
		let start: (NfaIdx, Vec<NfaSimulationData>) = (
			NfaIdx(0),
			vec![NfaSimulationData {
				captures: BTreeMap::new(),
			}],
		);

		let mut state_set: BTreeSet<(NfaIdx, Vec<NfaSimulationData>)> = BTreeSet::new();
		state_set.insert(start);
		state_set = self.epsilon_closure(state_set, 0);

		let mut maybe_last_match: Option<(usize, Vec<(NfaIdx, Vec<NfaSimulationData>)>)> = None;

		for (i, ch) in input.char_indices() {
			debug!("=== step {i}, ch {}", u32::from(ch));
			debug!("state set is {state_set:#?}");
			state_set = self.step_on_symbol(&state_set, ch);
			debug!("after step is {state_set:#?}");
			state_set = self.epsilon_closure(state_set, i + 1);
			debug!("after closure is {state_set:#?}");
			if state_set.is_empty() {
				break;
			}
			for (state, data) in state_set.iter() {
				let mut any_final_states: Vec<(NfaIdx, Vec<NfaSimulationData>)> = Vec::new();
				if state.is_end() {
					debug!("=== got match!");
					// TODO pretty sure this nolonger true
					assert_eq!(data.len(), 1);
					any_final_states.push((*state, data.clone()));
				}
				if !any_final_states.is_empty() {
					maybe_last_match = Some((i + ch.len_utf8(), any_final_states));
				}
			}
		}

		for (state, data) in state_set.iter() {
			if state.is_end() {
				debug!("=== got match!");
				// TODO pretty sure nolonger true
				assert_eq!(data.len(), 1);
				let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
				for (capture, (starts, ends)) in data.last().unwrap().captures.iter() {
					debug!("[matched] {capture:?} {starts:?} {ends:?}");
					for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
						m.push((capture.rule, &capture.regex.name, x, y));
					}
				}
				return Some(input.len());
			}
		}

		if let Some((pos, last_match)) = maybe_last_match {
			let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
			for (state, last_match) in last_match.iter() {
				for data in last_match.iter() {
					for (capture, (starts, ends)) in data.captures.iter() {
						debug!("[matched] {capture:?} {starts:?} {ends:?}");
						for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
							m.push((capture.rule, &capture.regex.name, x, y));
						}
					}
				}
			}
			return Some(pos);
		}

		None
	}

	fn epsilon_closure(
		&self,
		state_set: BTreeSet<(NfaIdx, Vec<NfaSimulationData>)>,
		pos: usize,
	) -> BTreeSet<(NfaIdx, Vec<NfaSimulationData>)> {
		let mut closure: BTreeSet<(NfaIdx, Vec<NfaSimulationData>)> = BTreeSet::new();
		let mut states_in_closure: BTreeSet<NfaIdx> = BTreeSet::new();

		let mut stack: Vec<(NfaIdx, Vec<NfaSimulationData>)> = state_set.into_iter().collect::<Vec<_>>();

		while let Some((state, data)) = stack.pop() {
			closure.insert((state, data.clone()));
			states_in_closure.insert(state);

			if state.is_end() {
				continue;
			}
			let state: &NfaState = &self[state];

			for transition in state.spontaneous.iter() {
				if states_in_closure.contains(&transition.target) {
					continue;
				}

				let mut data: Vec<NfaSimulationData> = data.clone();
				match &transition.kind {
					SpontaneousTransitionKind::Positive(tag) => {
						let (Tag::StartCapture(capture) | Tag::StopCapture(capture)): &Tag = tag;
						let (starts, ends): &mut (Vec<usize>, Vec<usize>) = data
							.last_mut()
							.unwrap()
							.captures
							.entry(capture.clone())
							.or_insert((Vec::new(), Vec::new()));
						let indices: &mut Vec<usize> = match tag {
							Tag::StartCapture(_) => starts,
							Tag::StopCapture(_) => ends,
						};
						indices.push(pos);
					},
					SpontaneousTransitionKind::Negative(_) => (),
					SpontaneousTransitionKind::Epsilon => (),
				}
				stack.push((transition.target, data));
			}
		}

		closure
	}

	fn step_on_symbol(
		&self,
		state_set: &BTreeSet<(NfaIdx, Vec<NfaSimulationData>)>,
		ch: char,
	) -> BTreeSet<(NfaIdx, Vec<NfaSimulationData>)> {
		let mut next_states: BTreeSet<(NfaIdx, Vec<NfaSimulationData>)> = BTreeSet::new();

		for (state, matches) in state_set.iter() {
			if let Some(targets) = self[*state].transitions.lookup(u32::from(ch)) {
				for &next in targets.iter() {
					next_states.insert((next, matches.clone()));
				}
			}
		}

		next_states
	}

	fn new_state<LikeString>(&mut self, name: LikeString) -> NfaIdx
	where
		LikeString: Into<Cow<'static, str>>,
	{
		let idx: NfaIdx = NfaIdx(self.states.len());
		let state: NfaState = NfaState {
			idx,
			name: name.into(),
			transitions: IntervalTree::new(),
			spontaneous: Vec::new(),
		};
		self.states.push(state);
		idx
	}

	fn build(&mut self, rule: usize, regex: &Regex, mut current: NfaIdx, target: NfaIdx) -> BTreeSet<Tag> {
		match regex {
			Regex::AnyChar => {
				self[current].transitions.insert(
					Interval::new(0, u32::from(char::MAX)),
					BTreeSet::from([target]),
					merge_sets,
				);
				BTreeSet::new()
			},
			&Regex::Literal(ch) => {
				self[current].transitions.insert(
					Interval::new(u32::from(ch), u32::from(ch)),
					BTreeSet::from([target]),
					merge_sets,
				);
				BTreeSet::new()
			},
			Regex::Capture(capture) => self.capture(rule, capture.clone(), &capture.item, current, target),
			Regex::Group { negated, items } => {
				if *negated {
					let mut intervals: Vec<Interval<u32>> = Vec::with_capacity(items.len());
					for &(start, end) in items.iter() {
						if start > end {
							todo!("warn invalid range");
						}
						intervals.push(Interval::new(u32::from(start), u32::from(end)));
					}
					for interval in Interval::complement(&mut intervals).into_iter() {
						self[current]
							.transitions
							.insert(interval, BTreeSet::from([target]), merge_sets);
					}
				} else {
					for &(start, end) in items.iter() {
						if start > end {
							todo!("warn invalid range");
						}
						self[current].transitions.insert(
							Interval::new(u32::from(start), u32::from(end)),
							BTreeSet::from([target]),
							merge_sets,
						);
					}
				}
				BTreeSet::new()
			},
			Regex::KleeneClosure(item) => {
				let item_start: NfaIdx = self.new_state("kleene item start");
				let item_end: NfaIdx = self.new_state("kleene item end");
				let item_skip: NfaIdx = self.new_state("kleene skip");

				self[current].spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target: item_start,
				});
				self[current].spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target: item_skip,
				});

				let tags: BTreeSet<Tag> = self.build(rule, item, item_start, item_end);

				self[item_end].spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target: item_start,
				});
				self[item_end].spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target,
				});

				self.negative_tags(tags.iter().cloned(), item_skip, target);

				tags
			},
			Regex::BoundedRepetition { lo, hi, item } => {
				if lo > hi {
					todo!("warn invalid repetition");
				}
				let middle: NfaIdx = self.new_state("bounded middle");

				let mut tags: BTreeSet<Tag> = BTreeSet::new();
				for i in 0..*lo {
					let sub_target: NfaIdx = self.new_state("bounded sub 1/2 target");
					tags.append(&mut self.build(rule, item, current, sub_target));
					current = sub_target;
				}

				self[current].spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target: middle,
				});
				self[current].spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target,
				});

				current = middle;
				for i in *lo..*hi {
					let sub_target: NfaIdx = if i + 1 < *hi {
						self.new_state("bounded sub 2/2 target")
					} else {
						target
					};
					self[current].spontaneous.push(SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target,
					});
					tags.append(&mut self.build(rule, item, current, sub_target));
					current = sub_target;
				}
				tags
			},
			Regex::Sequence(items) => {
				let mut tags: BTreeSet<Tag> = BTreeSet::new();
				for (i, sub_item) in items.iter().enumerate() {
					let sub_target: NfaIdx = if i + 1 < items.len() {
						self.new_state("sequence sub target")
					} else {
						target
					};
					tags.append(&mut self.build(rule, sub_item, current, sub_target));
					current = sub_target;
				}
				tags
			},
			Regex::Alternation(items) => self.alternate(rule, items, current, target),
		}
	}

	fn capture(
		&mut self,
		rule: usize,
		capture: RegexCapture,
		item: &Regex,
		current: NfaIdx,
		target: NfaIdx,
	) -> BTreeSet<Tag> {
		let capture: AutomataCapture = AutomataCapture { rule, regex: capture };

		let start_capture: Tag = Tag::StartCapture(capture.clone());
		let end_capture: Tag = Tag::StopCapture(capture);

		let sub_start: NfaIdx = self.new_state("capture started");
		let sub_end: NfaIdx = self.new_state("capture before end");

		self[current].spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(start_capture.clone()),
			target: sub_start,
		});

		let mut tags: BTreeSet<Tag> = self.build(rule, item, sub_start, sub_end);

		self[sub_end].spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(end_capture.clone()),
			target,
		});

		tags.insert(start_capture);
		tags.insert(end_capture);

		tags
	}

	fn alternate(&mut self, rule: usize, items: &[Regex], current: NfaIdx, target: NfaIdx) -> BTreeSet<Tag> {
		let mut tags: BTreeSet<Tag> = BTreeSet::new();
		let mut intermediate_states: Vec<(NfaIdx, BTreeSet<Tag>)> = Vec::new();

		for (i, sub_item) in items.iter().enumerate() {
			let sub_start: NfaIdx = self.new_state("alternate sub start");
			let sub_target: NfaIdx = self.new_state("alternate sub target");

			self[current].spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target: sub_start,
			});

			intermediate_states.push((sub_target, self.build(rule, sub_item, sub_start, sub_target)));
		}

		for (i, (sub_state, sub_tags)) in intermediate_states.iter().enumerate() {
			let mut sub_current: NfaIdx = *sub_state;
			for (other, (_, other_tags)) in intermediate_states.iter().enumerate() {
				let sub_target: NfaIdx = self.new_state("alternate negate tags");

				if other == i {
					continue;
				}

				self.negative_tags(other_tags.iter().cloned(), sub_current, sub_target);
				sub_current = sub_target;
			}

			self[sub_current].spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target,
			});

			// `&BTreeSet<_>` implements `BitOr`.
			// `sub_tags` is already a reference from the iteration.
			tags = &tags | sub_tags;
		}

		tags
	}

	fn negative_tags(&mut self, tags: impl Iterator<Item = Tag>, mut current: NfaIdx, target: NfaIdx) {
		for t in tags {
			let next: NfaIdx = self.new_state("negative tags");
			self[current].spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Negative(t),
				target: next,
			});
			current = next;
		}
		self[current].spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Epsilon,
			target,
		});
	}
}

impl Nfa {
	pub fn tags(&self) -> &[Tag] {
		&self.tags
	}

	pub fn begin(&self) -> NfaIdx {
		NfaIdx::BEGIN
	}
}

impl std::ops::Index<NfaIdx> for Nfa {
	type Output = NfaState;

	fn index(&self, i: NfaIdx) -> &Self::Output {
		&self.states[i.0]
	}
}

impl std::ops::IndexMut<NfaIdx> for Nfa {
	fn index_mut(&mut self, i: NfaIdx) -> &mut Self::Output {
		&mut self.states[i.0]
	}
}

impl NfaState {
	const BEGIN: Self = Self {
		idx: NfaIdx::BEGIN,
		name: Cow::Borrowed("begin"),
		transitions: IntervalTree::new(),
		spontaneous: Vec::new(),
	};

	const END: Self = Self {
		idx: NfaIdx::END,
		name: Cow::Borrowed("end"),
		transitions: IntervalTree::new(),
		spontaneous: Vec::new(),
	};

	pub fn transitions(&self) -> &IntervalTree<u32, BTreeSet<NfaIdx>> {
		&self.transitions
	}

	pub fn spontaneous(&self) -> &[SpontaneousTransition] {
		&self.spontaneous
	}
}

// TODO explain isize/usize
impl NfaIdx {
	const BEGIN: Self = Self(0);
	const END: Self = Self::end(0);

	const fn end(n: usize) -> Self {
		Self(usize::MAX - n)
	}

	pub fn final_rule(&self) -> Option<usize> {
		if self.is_end() { Some(usize::MAX - self.0) } else { None }
	}

	pub fn is_end(&self) -> bool {
		self.0 > (isize::MAX as usize)
	}
}

/// Ideally, we could write:
///
/// ```rust,ignore
/// type MergeFn = fn(&BTreeSet<NfaIdx>, &BTreeSet<NfaIdx>) -> BTreeSet<NfaIdx>;
/// const MERGE_SETS: MergeFn = <&BTreeSet<NfaIdx> as std::ops::BitOr>::bitor;
/// ```
///
/// But Rust can't do this.
fn merge_sets(v1: &BTreeSet<NfaIdx>, v2: &BTreeSet<NfaIdx>) -> BTreeSet<NfaIdx> {
	v1 | v2
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::schema::Schema;

	#[test]
	fn stuff() {
		let r: Regex = Regex::from_pattern("0((?<foobar>1(2[a-zA-Z])*)|(?<baz>xyz))world").unwrap();
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", r);
		let nfa: Nfa = Nfa::for_rules(schema.rules());
		let b: bool = nfa.simulate("012a2b2cworld").is_some();
		assert!(b);
		let b: bool = nfa.simulate("0xyzworld").is_some();
		assert!(b);
	}
}
