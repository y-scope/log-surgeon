use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZero;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::regex::Regex;
use crate::schema::Schema;

#[derive(Debug)]
pub struct Nfa<'schema> {
	states: Vec<NfaState<'schema>>,
	tags: Vec<Tag<'schema>>,
}

#[derive(Debug)]
pub struct NfaError {}

#[derive(Debug)]
pub struct NfaState<'schema> {
	idx: usize,
	/// Just to be cute and for debugging, in that order.
	name: Cow<'static, str>,
	transitions: IntervalTree<u32, BTreeSet<NfaIdx>>,
	spontaneous: Vec<SpontaneousTransition<'schema>>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct NfaIdx(usize);

#[derive(Debug)]
pub struct SpontaneousTransition<'schema> {
	pub kind: SpontaneousTransitionKind<'schema>,
	pub priority: usize,
	pub target: NfaIdx,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum SpontaneousTransitionKind<'schema> {
	Epsilon,
	Positive(Tag<'schema>),
	Negative(Tag<'schema>),
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum Tag<'schema> {
	StartCapture(Variable<'schema>),
	EndCapture(Variable<'schema>),
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Variable<'schema> {
	/// Rule index from the schema.
	pub rule: usize,
	/// Capture ID local to the rule/regex pattern.
	/// It is `None` (i.e. `0`) iff it is the implicit top-level capture.
	pub id: Option<NonZero<u32>>,
	/// Capture name from the rule/regex pattern.
	/// Technically redundant; exists for debugging.
	pub name: &'schema str,
}

impl std::fmt::Debug for Variable<'_> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if !fmt.alternate() {
			fmt.debug_tuple("Variable").field(&self.rule).field(&self.name).finish()
		} else {
			fmt.write_fmt(format_args!(
				"Variable({}, {}, {})",
				self.rule,
				self.id.map_or(0, NonZero::get),
				self.name
			))
		}
	}
}

struct NfaBuilder<'a, 'schema> {
	nfa: &'a mut Nfa<'schema>,
	current: NfaIdx,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct TagMatches<'schema> {
	offsets: BTreeMap<Variable<'schema>, (Vec<usize>, Vec<usize>)>,
}

impl<'schema> Nfa<'schema> {
	pub fn for_schema(schema: &'schema Schema) -> Result<Self, NfaError> {
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
		let tags: BTreeSet<Tag<'_>> = builder.alternate(
			schema.rules().iter().enumerate().skip(1).map(|(i, rule)| {
				move |builder: &mut NfaBuilder<'_, 'schema>, target| {
					builder.capture(i, None, &*rule.name, &rule.regex, target)
				}
			}),
			end,
		)?;
		nfa.tags = tags.into_iter().collect::<Vec<_>>();

		Ok(nfa)
	}

	/// - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
	/// - <https://arxiv.org/abs/2206.01398>
	///
	/// Algorithm 1.
	pub fn simulate<'input>(&self, input: &'input str) -> Option<usize> {
		let start: (NfaIdx, Vec<TagMatches>) = (
			NfaIdx(0),
			vec![TagMatches {
				offsets: BTreeMap::new(),
			}],
		);

		let mut state_set: BTreeSet<(NfaIdx, Vec<TagMatches>)> = BTreeSet::new();
		state_set.insert(start);
		state_set = self.epsilon_closure(state_set, 0);

		let mut maybe_last_match: Option<(usize, Vec<TagMatches>)> = None;

		for (i, ch) in input.char_indices() {
			println!("=== step {i}, ch {}", u32::from(ch));
			println!("state set is {state_set:#?}");
			state_set = self.step_on_symbol(&state_set, ch);
			println!("after step is {state_set:#?}");
			state_set = self.epsilon_closure(state_set, i + 1);
			println!("after closure is {state_set:#?}");
			if state_set.is_empty() {
				break;
			}
			for (state, matches) in state_set.iter() {
				// TODO more general
				if state.0 == 1 {
					println!("=== got match!");
					assert_eq!(matches.len(), 1);
					maybe_last_match = Some((i, matches.clone()));
				}
			}
		}

		for (state, matches) in state_set.iter() {
			// TODO more general
			if state.0 == 1 {
				println!("=== got match!");
				assert_eq!(matches.len(), 1);
				let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
				for (variable, (starts, ends)) in matches.last().unwrap().offsets.iter() {
					println!("[matched variable {variable:?} {starts:?} {ends:?}");
					for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
						m.push((variable.rule, variable.name, x, y));
					}
				}
				return Some(input.len());
			}
		}

		if let Some((pos, last_match)) = maybe_last_match {
			let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
			for (variable, (starts, ends)) in last_match.last().unwrap().offsets.iter() {
				println!("[matched variable {variable:?} {starts:?} {ends:?}");
				for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
					m.push((variable.rule, variable.name, x, y));
				}
			}
			return Some(pos);
		}

		None
	}

	fn epsilon_closure(
		&self,
		state_set: BTreeSet<(NfaIdx, Vec<TagMatches<'schema>>)>,
		pos: usize,
	) -> BTreeSet<(NfaIdx, Vec<TagMatches<'schema>>)> {
		let mut closure: BTreeSet<(NfaIdx, Vec<TagMatches>)> = BTreeSet::new();
		let mut states_in_closure: BTreeSet<NfaIdx> = BTreeSet::new();

		let mut stack: Vec<(NfaIdx, Vec<TagMatches>)> = state_set.into_iter().collect::<Vec<_>>();

		while let Some((state, matches)) = stack.pop() {
			closure.insert((state, matches.clone()));
			states_in_closure.insert(state);

			let state: &NfaState<'_> = &self[state];

			for transition in state.spontaneous.iter() {
				if states_in_closure.contains(&transition.target) {
					continue;
				}

				let mut matches: Vec<TagMatches> = matches.clone();
				match transition.kind {
					SpontaneousTransitionKind::Positive(tag) => match tag {
						Tag::StartCapture(variable) => {
							matches
								.last_mut()
								.unwrap()
								.offsets
								.entry(variable)
								.or_insert((Vec::new(), Vec::new()))
								.0
								.push(pos);
						},
						Tag::EndCapture(variable) => {
							matches
								.last_mut()
								.unwrap()
								.offsets
								.entry(variable)
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
		state_set: &BTreeSet<(NfaIdx, Vec<TagMatches<'schema>>)>,
		ch: char,
	) -> BTreeSet<(NfaIdx, Vec<TagMatches<'schema>>)> {
		let mut next_states: BTreeSet<(NfaIdx, Vec<TagMatches<'schema>>)> = BTreeSet::new();

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

impl<'schema> Nfa<'schema> {
	pub fn tags(&self) -> &[Tag<'schema>] {
		&self.tags
	}
}

impl<'schema> NfaBuilder<'_, 'schema> {
	fn build(
		&mut self,
		rule: usize,
		regex: &'schema Regex,
		target: NfaIdx,
	) -> Result<BTreeSet<Tag<'schema>>, NfaError> {
		match regex {
			Regex::AnyChar => {
				self.current().transitions.insert(
					Interval::new(0, u32::from(char::MAX)),
					BTreeSet::from([target]),
					merge_sets,
				);
				Ok(BTreeSet::new())
			},
			&Regex::Literal(ch) => {
				self.current().transitions.insert(
					Interval::new(u32::from(ch), u32::from(ch)),
					BTreeSet::from([target]),
					merge_sets,
				);
				Ok(BTreeSet::new())
			},
			Regex::Capture { name, item, meta } => {
				// TODO report unwrap fails if number_captures not called
				self.capture(rule, Some(meta.id.unwrap()), name, item, target)
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
				Ok(BTreeSet::new())
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
				let tags: BTreeSet<Tag<'_>> = self.build(rule, item, item_end)?;

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
				self.negative_tags(tags.iter().copied(), target);

				Ok(tags)
			},
			Regex::BoundedRepetition { start, end, item } => {
				if start > end {
					todo!("warn invalid repetition");
				}
				let middle: NfaIdx = self.nfa.new_state("bounded middle");
				// let pre_end: usize = self.nfa.new_state();

				let mut tags: BTreeSet<Tag<'_>> = BTreeSet::new();
				for i in 0..*start {
					// let sub_target: usize = if i + 1 < *start { self.nfa.new_state() } else { pre_end };
					let sub_target: NfaIdx = self.nfa.new_state("bounded sub 1/2 target");
					tags.append(&mut self.build(rule, item, sub_target)?);
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
					tags.append(&mut self.build(rule, item, sub_target)?);
					self.current = sub_target;
				}
				// self.current = pre_end;
				// self.current().spontaneous.push(SpontaneousTransition {
				// 	tag: NfaTag::Epsilon,
				// 	priority: 0,
				// 	target,
				// });
				// self.current = target;
				Ok(tags)
			},
			Regex::Sequence(items) => {
				let mut tags: BTreeSet<Tag<'_>> = BTreeSet::new();
				for (i, sub_item) in items.iter().enumerate() {
					let sub_target: NfaIdx = if i + 1 < items.len() {
						self.nfa.new_state("sequence sub target")
					} else {
						target
					};
					tags.append(&mut self.build(rule, sub_item, sub_target)?);
					self.current = sub_target;
				}
				Ok(tags)
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
		name: &'schema str,
		item: &'schema Regex,
		target: NfaIdx,
	) -> Result<BTreeSet<Tag<'schema>>, NfaError> {
		let variable: Variable<'_> = Variable { rule, id, name };

		let start_capture: Tag<'_> = Tag::StartCapture(variable);
		let end_capture: Tag<'_> = Tag::EndCapture(variable);

		let sub_start: NfaIdx = self.nfa.new_state("capture started");
		let sub_end: NfaIdx = self.nfa.new_state("capture before end");

		self.current().spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(start_capture),
			priority: 0,
			target: sub_start,
		});

		self.current = sub_start;
		let mut tags: BTreeSet<Tag<'_>> = self.build(rule, item, sub_end)?;

		self.current = sub_end;
		self.current().spontaneous.push(SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(end_capture),
			priority: 0,
			target,
		});

		tags.insert(start_capture);
		tags.insert(end_capture);

		Ok(tags)
	}

	fn alternate<I, F>(&mut self, items: I, target: NfaIdx) -> Result<BTreeSet<Tag<'schema>>, NfaError>
	where
		I: Iterator<Item = F>,
		F: Fn(&mut Self, NfaIdx) -> Result<BTreeSet<Tag<'schema>>, NfaError>,
	{
		let orig_start: NfaIdx = self.current;

		let mut tags: BTreeSet<Tag<'_>> = BTreeSet::new();
		let mut intermediate_states: Vec<(NfaIdx, BTreeSet<Tag<'_>>)> = Vec::new();

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
			intermediate_states.push((sub_target, sub_item(self, sub_target)?));
		}

		for (i, (sub_state, sub_tags)) in intermediate_states.iter().enumerate() {
			self.current = *sub_state;
			for (other, (_, other_tags)) in intermediate_states.iter().enumerate() {
				let sub_target: NfaIdx = self.nfa.new_state("alternate negate tags");

				if other == i {
					continue;
				}

				self.negative_tags(other_tags.iter().copied(), sub_target);
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

		Ok(tags)
	}

	fn negative_tags(&mut self, tags: impl Iterator<Item = Tag<'schema>>, target: NfaIdx) {
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

	fn current(&mut self) -> &mut NfaState<'schema> {
		&mut self.nfa.states[self.current.0]
	}
}

impl<'schema> Nfa<'schema> {
	pub fn start(&self) -> NfaIdx {
		NfaIdx(0)
	}

	pub fn end(&self) -> NfaIdx {
		NfaIdx(1)
	}
}

impl<'schema> std::ops::Index<NfaIdx> for Nfa<'schema> {
	type Output = NfaState<'schema>;

	fn index(&self, i: NfaIdx) -> &Self::Output {
		&self.states[i.0]
	}
}

impl<'schema> std::ops::IndexMut<NfaIdx> for Nfa<'schema> {
	fn index_mut(&mut self, i: NfaIdx) -> &mut Self::Output {
		&mut self.states[i.0]
	}
}

impl NfaIdx {
	pub fn is_end(&self) -> bool {
		self.0 == 1
	}
}

impl<'schema> NfaState<'schema> {
	pub fn transitions(&self) -> &IntervalTree<u32, BTreeSet<NfaIdx>> {
		&self.transitions
	}

	pub fn spontaneous(&self) -> &[SpontaneousTransition<'schema>] {
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
		let nfa: Nfa<'_> = Nfa::for_schema(&schema).unwrap();
		let b: bool = nfa.simulate("012a2b2cworld").is_some();
		assert!(b);
		let b: bool = nfa.simulate("0xyzworld").is_some();
		assert!(b);
	}
}
