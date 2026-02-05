use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZero;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::regex::Regex;
use crate::schema::Schema;

#[derive(Debug)]
pub struct Nfa {
	states: Vec<NfaState>,
	tags: Vec<Tag>,
}

#[derive(Debug)]
pub struct NfaState {
	idx: usize,
	transitions: IntervalTree<u32, BTreeSet<NfaIdx>>,
	spontaneous: Vec<SpontaneousTransition>,
	/// Just to be cute, and for debugging, in that order.
	/// See [`Nfa::new_state`].
	name: Cow<'static, str>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct NfaIdx(usize);

#[derive(Debug)]
pub struct SpontaneousTransition {
	pub kind: SpontaneousTransitionKind,
	pub priority: usize,
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
	Start(Capture),
	End(Capture),
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Capture {
	/// Rule index from the schema.
	pub rule: usize,
	/// Capture ID local to the rule/regex pattern.
	/// It is `None` (i.e. `0`) iff it is the implicit top-level capture.
	pub id: Option<NonZero<u32>>,
	/// Capture name from the rule/regex pattern.
	/// Technically redundant (given a schema); exists for debugging.
	pub name: String,
}

impl std::fmt::Debug for Capture {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if !fmt.alternate() {
			fmt.write_fmt(format_args!("Capture({}, {})", self.rule, self.name))
		} else {
			fmt.write_fmt(format_args!(
				"Capture({}, {}, {})",
				self.rule,
				self.id.map_or(0, NonZero::get),
				self.name
			))
		}
	}
}

struct NfaBuilder<'a> {
	nfa: &'a mut Nfa,
	current: NfaIdx,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct CaptureIndices {
	offsets: BTreeMap<Capture, (Vec<usize>, Vec<usize>)>,
}

impl Nfa {
	pub fn for_schema(schema: &Schema) -> Self {
		let mut nfa: Self = Nfa {
			states: Vec::new(),
			tags: Vec::new(),
		};
		let start: NfaIdx = nfa.new_state("start");
		let end: NfaIdx = nfa.new_state("end");

		let mut builder: NfaBuilder = NfaBuilder {
			nfa: &mut nfa,
			current: start,
		};
		builder.current = start;
		let tags: BTreeSet<Tag> = builder.alternate(
			schema.rules().iter().enumerate().skip(1).map(|(i, rule)| {
				move |builder: &mut NfaBuilder<'_>, target| {
					builder.capture(i, None, rule.name.clone(), &rule.regex, target)
				}
			}),
			end,
		);
		nfa.tags = tags.into_iter().collect::<Vec<_>>();

		nfa
	}

	/// - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
	/// - <https://arxiv.org/abs/2206.01398>
	///
	/// Algorithm 1.
	pub fn simulate(&self, input: &str) -> Option<usize> {
		let start: (NfaIdx, Vec<CaptureIndices>) = (
			NfaIdx(0),
			vec![CaptureIndices {
				offsets: BTreeMap::new(),
			}],
		);

		let mut state_set: BTreeSet<(NfaIdx, Vec<CaptureIndices>)> = BTreeSet::new();
		state_set.insert(start);
		state_set = self.epsilon_closure(state_set, 0);

		let mut maybe_last_match: Option<(usize, Vec<CaptureIndices>)> = None;

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
			for (state, matches) in state_set.iter() {
				// TODO more general
				if state.0 == 1 {
					debug!("=== got match!");
					assert_eq!(matches.len(), 1);
					maybe_last_match = Some((i, matches.clone()));
				}
			}
		}

		for (state, matches) in state_set.iter() {
			// TODO more general
			if state.0 == 1 {
				debug!("=== got match!");
				assert_eq!(matches.len(), 1);
				let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
				for (capture, (starts, ends)) in matches.last().unwrap().offsets.iter() {
					debug!("[matched] {capture:?} {starts:?} {ends:?}");
					for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
						m.push((capture.rule, &capture.name, x, y));
					}
				}
				return Some(input.len());
			}
		}

		if let Some((pos, last_match)) = maybe_last_match {
			let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
			for (capture, (starts, ends)) in last_match.last().unwrap().offsets.iter() {
				debug!("[matched] {capture:?} {starts:?} {ends:?}");
				for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
					m.push((capture.rule, &capture.name, x, y));
				}
			}
			return Some(pos);
		}

		None
	}

	fn epsilon_closure(
		&self,
		state_set: BTreeSet<(NfaIdx, Vec<CaptureIndices>)>,
		pos: usize,
	) -> BTreeSet<(NfaIdx, Vec<CaptureIndices>)> {
		let mut closure: BTreeSet<(NfaIdx, Vec<CaptureIndices>)> = BTreeSet::new();
		let mut states_in_closure: BTreeSet<NfaIdx> = BTreeSet::new();

		let mut stack: Vec<(NfaIdx, Vec<CaptureIndices>)> = state_set.into_iter().collect::<Vec<_>>();

		while let Some((state, matches)) = stack.pop() {
			closure.insert((state, matches.clone()));
			states_in_closure.insert(state);

			let state: &NfaState = &self[state];

			for transition in state.spontaneous.iter() {
				if states_in_closure.contains(&transition.target) {
					continue;
				}

				// TODO always paired?
				let mut matches: Vec<CaptureIndices> = matches.clone();
				match &transition.kind {
					SpontaneousTransitionKind::Positive(tag) => match tag {
						Tag::Start(capture) => {
							matches
								.last_mut()
								.unwrap()
								.offsets
								.entry(capture.clone())
								.or_insert((Vec::new(), Vec::new()))
								.0
								.push(pos);
						},
						Tag::End(capture) => {
							matches
								.last_mut()
								.unwrap()
								.offsets
								.entry(capture.clone())
								.or_insert((Vec::new(), Vec::new()))
								.1
								.push(pos);
						},
					},
					SpontaneousTransitionKind::Negative(_) => (),
					SpontaneousTransitionKind::Epsilon => (),
				}
				stack.push((transition.target, matches));
			}
		}

		closure
	}

	fn step_on_symbol(
		&self,
		state_set: &BTreeSet<(NfaIdx, Vec<CaptureIndices>)>,
		ch: char,
	) -> BTreeSet<(NfaIdx, Vec<CaptureIndices>)> {
		let mut next_states: BTreeSet<(NfaIdx, Vec<CaptureIndices>)> = BTreeSet::new();

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
		let n: usize = self.states.len();
		let state: NfaState = NfaState {
			idx: n,
			name: name.into(),
			transitions: IntervalTree::new(),
			spontaneous: Vec::new(),
		};
		self.states.push(state);
		NfaIdx(n)
	}
}

impl Nfa {
	pub fn tags(&self) -> &[Tag] {
		&self.tags
	}
}

impl NfaBuilder<'_> {
	fn build(&mut self, rule: usize, regex: &Regex, target: NfaIdx) -> BTreeSet<Tag> {
		match regex {
			Regex::AnyChar => {
				self.current().transitions.insert(
					Interval::new(0, u32::from(char::MAX)),
					BTreeSet::from([target]),
					merge_sets,
				);
				BTreeSet::new()
			},
			&Regex::Literal(ch) => {
				self.current().transitions.insert(
					Interval::new(u32::from(ch), u32::from(ch)),
					BTreeSet::from([target]),
					merge_sets,
				);
				BTreeSet::new()
			},
			Regex::Capture { name, item, meta } => {
				// TODO report unwrap fails if number_captures not called
				self.capture(rule, Some(meta.id.unwrap()), name.clone(), item, target)
			},
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
						self.current()
							.transitions
							.insert(interval, BTreeSet::from([target]), merge_sets);
					}
				} else {
					for &(start, end) in items.iter() {
						if start > end {
							todo!("warn invalid range");
						}
						self.current().transitions.insert(
							Interval::new(u32::from(start), u32::from(end)),
							BTreeSet::from([target]),
							merge_sets,
						);
					}
				}
				BTreeSet::new()
			},
			Regex::KleeneClosure(item) => {
				let item_start: NfaIdx = self.nfa.new_state("kleene item start");
				let item_end: NfaIdx = self.nfa.new_state("kleene item end");
				let item_skip: NfaIdx = self.nfa.new_state("kleene skip");

				self.current().spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					priority: 0,
					target: item_start,
				});
				self.current().spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					priority: 1,
					target: item_skip,
				});

				self.current = item_start;
				let tags: BTreeSet<Tag> = self.build(rule, item, item_end);

				self.current = item_end;
				self.current().spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					priority: 0,
					target: item_start,
				});
				self.current().spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					priority: 1,
					target,
				});

				self.current = item_skip;
				self.negative_tags(tags.iter().cloned(), target);

				tags
			},
			Regex::BoundedRepetition { start, end, item } => {
				if start > end {
					todo!("warn invalid repetition");
				}
				let middle: NfaIdx = self.nfa.new_state("bounded middle");
				// let pre_end: usize = self.nfa.new_state();

				let mut tags: BTreeSet<Tag> = BTreeSet::new();
				for i in 0..*start {
					// let sub_target: usize = if i + 1 < *start { self.nfa.new_state() } else { pre_end };
					let sub_target: NfaIdx = self.nfa.new_state("bounded sub 1/2 target");
					tags.append(&mut self.build(rule, item, sub_target));
					self.current = sub_target;
				}

				self.current().spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					priority: 0,
					target: middle,
				});
				self.current().spontaneous.push(SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					priority: 1,
					target,
				});

				self.current = middle;
				for i in *start..*end {
					let sub_target: NfaIdx = if i + 1 < *end {
						self.nfa.new_state("bounded sub 2/2 target")
					} else {
						target
					};
					// let sub_target: usize = self.nfa.new_state();
					self.current().spontaneous.push(SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						priority: 0,
						target,
					});
					tags.append(&mut self.build(rule, item, sub_target));
					self.current = sub_target;
				}
				// self.current = pre_end;
				// self.current().spontaneous.push(SpontaneousTransition {
				// 	tag: NfaTag::Epsilon,
				// 	priority: 0,
				// 	target,
				// });
				// self.current = target;
				tags
			},
			Regex::Sequence(items) => {
				let mut tags: BTreeSet<Tag> = BTreeSet::new();
				for (i, sub_item) in items.iter().enumerate() {
					let sub_target: NfaIdx = if i + 1 < items.len() {
						self.nfa.new_state("sequence sub target")
					} else {
						target
					};
					tags.append(&mut self.build(rule, sub_item, sub_target));
					self.current = sub_target;
				}
				tags
			},
			Regex::Alternation(items) => self.alternate(
				items
					.iter()
					.map(|sub_item| |builder: &mut Self, sub_target| builder.build(rule, sub_item, sub_target)),
				target,
			),
		}
	}

	fn capture(
		&mut self,
		rule: usize,
		id: Option<NonZero<u32>>,
		name: String,
		item: &Regex,
		target: NfaIdx,
	) -> BTreeSet<Tag> {
		let capture: Capture = Capture { rule, id, name };

		let start_capture: Tag = Tag::Start(capture.clone());
		let end_capture: Tag = Tag::End(capture);

		let sub_start: NfaIdx = self.nfa.new_state("capture started");
		let sub_end: NfaIdx = self.nfa.new_state("capture before end");

		self.current().spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(start_capture.clone()),
			priority: 0,
			target: sub_start,
		});

		self.current = sub_start;
		let mut tags: BTreeSet<Tag> = self.build(rule, item, sub_end);

		self.current = sub_end;
		self.current().spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(end_capture.clone()),
			priority: 0,
			target,
		});

		tags.insert(start_capture);
		tags.insert(end_capture);

		tags
	}

	fn alternate<I, F>(&mut self, items: I, target: NfaIdx) -> BTreeSet<Tag>
	where
		I: Iterator<Item = F>,
		F: Fn(&mut Self, NfaIdx) -> BTreeSet<Tag>,
	{
		let orig_start: NfaIdx = self.current;

		let mut tags: BTreeSet<Tag> = BTreeSet::new();
		let mut intermediate_states: Vec<(NfaIdx, BTreeSet<Tag>)> = Vec::new();

		for (i, sub_item) in items.enumerate() {
			let sub_start: NfaIdx = self.nfa.new_state("alternate sub start");
			let sub_target: NfaIdx = self.nfa.new_state("alternate sub target");

			self.current = orig_start;
			self.current().spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				priority: i,
				target: sub_start,
			});

			self.current = sub_start;
			intermediate_states.push((sub_target, sub_item(self, sub_target)));
		}

		for (i, (sub_state, sub_tags)) in intermediate_states.iter().enumerate() {
			self.current = *sub_state;
			for (other, (_, other_tags)) in intermediate_states.iter().enumerate() {
				let sub_target: NfaIdx = self.nfa.new_state("alternate negate tags");

				if other == i {
					continue;
				}

				self.negative_tags(other_tags.iter().cloned(), sub_target);
				self.current = sub_target;
			}

			self.current().spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				priority: 0,
				target,
			});

			// `&BTreeSet<_>` implements `BitOr`.
			// `sub_tags` is already a reference from the iteration.
			tags = &tags | sub_tags;
		}

		tags
	}

	fn negative_tags(&mut self, tags: impl Iterator<Item = Tag>, target: NfaIdx) {
		for t in tags {
			let next: NfaIdx = self.nfa.new_state("negative tags");
			self.current().spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Negative(t),
				priority: 0,
				target: next,
			});
			self.current = next;
		}
		self.current().spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Epsilon,
			priority: 0,
			target,
		});
	}

	fn current(&mut self) -> &mut NfaState {
		&mut self.nfa.states[self.current.0]
	}
}

impl Nfa {
	pub fn start(&self) -> NfaIdx {
		NfaIdx(0)
	}

	pub fn end(&self) -> NfaIdx {
		NfaIdx(1)
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

impl NfaIdx {
	pub fn is_end(&self) -> bool {
		self.0 == 1
	}
}

impl NfaState {
	pub fn transitions(&self) -> &IntervalTree<u32, BTreeSet<NfaIdx>> {
		&self.transitions
	}

	pub fn spontaneous(&self) -> &[SpontaneousTransition] {
		&self.spontaneous
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

	#[test]
	fn stuff() {
		let r: Regex = Regex::from_pattern("0((?<foobar>1(2[a-zA-Z])*)|(?<baz>xyz))world").unwrap();
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", r);
		let nfa: Nfa = Nfa::for_schema(&schema);
		let b: bool = nfa.simulate("012a2b2cworld").is_some();
		assert!(b);
		let b: bool = nfa.simulate("0xyzworld").is_some();
		assert!(b);
	}
}
