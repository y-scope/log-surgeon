/// Based on Angelo Borsotti and Ulya Trafimovich. 2022. A closer look at TDFA.
/// - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
/// - <https://arxiv.org/abs/2206.01398>
///
use std::borrow::Cow;
use std::collections::BTreeSet;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::Policy;
use crate::regex::Regex;
use crate::regex::RegexCapture;
use crate::schema::Rule;
use crate::schema::RuleIdx;

#[derive(Debug)]
pub struct Tnfa {
	states: Vec<NfaState>,
	tags: Vec<Tag>,
	delimiters: String,
	// rules: Vec<(RuleIdx, bool)>,
}

#[derive(Debug)]
pub struct NfaState {
	/// ID and also an index into an [`Nfa`]'s list of states.
	#[allow(unused)]
	idx: NfaIdx,
	transitions: IntervalTree<u32, Vec<NfaIdx>>,
	spontaneous: Vec<SpontaneousTransition>,
	maybe_accepts_for_rule: Option<(RuleIdx, bool)>,
	/// Just to be cute, and for debugging, in that order.
	/// See [`Nfa::new_state`].
	#[allow(unused)]
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
	pub rule: RuleIdx,
	pub capture_info: RegexCapture,
}

#[derive(Debug)]
struct PolicyExtendUnique;

impl Tnfa {
	pub fn for_rules<'a, Rules>(rules: Rules, delimiters: String) -> Self
	where
		Rules: IntoIterator<Item = &'a Rule>,
	{
		let mut nfa: Self = Self {
			states: vec![NfaState::BEGIN],
			tags: Vec::new(),
			delimiters,
			// rules: Vec::new(),
		};

		let mut tags: BTreeSet<Tag> = BTreeSet::new();

		for rule in rules {
			let rule_start: NfaIdx = nfa.new_state(format!("rule '{}' start", rule.name));
			let rule_end: NfaIdx = nfa.new_state(format!("rule '{}' end", rule.name));
			nfa[NfaState::BEGIN.idx].spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target: rule_start,
			});
			tags = &tags | &nfa.build(rule.idx, &rule.regex, rule_start, rule_end);
			nfa[rule_end].maybe_accepts_for_rule = Some((rule.idx, rule.regex.ends_with_anchor()));
			// nfa.rules.push((rule.idx, rule.regex.ends_with_anchor()));
		}

		nfa.tags = tags.into_iter().collect::<Vec<_>>();

		nfa
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
			maybe_accepts_for_rule: None,
		};
		self.states.push(state);
		idx
	}

	fn build(&mut self, rule: RuleIdx, regex: &Regex, mut current: NfaIdx, target: NfaIdx) -> BTreeSet<Tag> {
		match regex {
			&Regex::Anchor(_) => {
				for ch in self.delimiters.clone().chars() {
					self[current].transitions.insert(
						Interval::new(u32::from(ch), u32::from(ch)),
						vec![target],
						PolicyExtendUnique,
					);
				}
				BTreeSet::new()
			},
			Regex::AnyChar => {
				self[current].transitions.insert(
					Interval::new(0, u32::from(char::MAX)),
					vec![target],
					PolicyExtendUnique,
				);
				BTreeSet::new()
			},
			&Regex::Literal(ch) => {
				self[current].transitions.insert(
					Interval::new(u32::from(ch), u32::from(ch)),
					vec![target],
					PolicyExtendUnique,
				);
				BTreeSet::new()
			},
			Regex::Capture { info, item } => self.capture(rule, info.clone(), item, current, target),
			Regex::Group { negated, items } => {
				if *negated {
					let mut intervals: Vec<Interval<u32>> = Vec::with_capacity(items.len());
					for &(start, end) in items.iter() {
						// Should have been verified during regex pattern parsing.
						assert!(start <= end);

						intervals.push(Interval::new(u32::from(start), u32::from(end)));
					}
					for interval in Interval::complement(&mut intervals).into_iter() {
						self[current]
							.transitions
							.insert(interval, vec![target], PolicyExtendUnique);
					}
				} else {
					for &(start, end) in items.iter() {
						// Should have been verified during regex pattern parsing.
						assert!(start <= end);

						self[current].transitions.insert(
							Interval::new(u32::from(start), u32::from(end)),
							vec![target],
							PolicyExtendUnique,
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
			Regex::BoundedRepetition { min, max, item } => {
				// Should have been verified during regex pattern parsing.
				assert!(*max > 0);
				assert!(min <= max);

				let middle: NfaIdx = self.new_state("bounded middle");

				let mut tags: BTreeSet<Tag> = BTreeSet::new();
				for _ in 0..*min {
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
				for i in *min..*max {
					let sub_target: NfaIdx = if i + 1 < *max {
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
		rule: RuleIdx,
		capture_info: RegexCapture,
		item: &Regex,
		current: NfaIdx,
		target: NfaIdx,
	) -> BTreeSet<Tag> {
		let capture: AutomataCapture = AutomataCapture { rule, capture_info };

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

	fn alternate(&mut self, rule: RuleIdx, items: &[Regex], current: NfaIdx, target: NfaIdx) -> BTreeSet<Tag> {
		let mut tags: BTreeSet<Tag> = BTreeSet::new();
		let mut intermediate_states: Vec<(NfaIdx, BTreeSet<Tag>)> = Vec::new();

		for sub_item in items.iter() {
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

			// `&BTreeSet<_>` implements `BitOr`, `BTreeSet<_>` does not.
			// `sub_tags` from the loop is already a `&BTreeSet<_>`.
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

impl Tnfa {
	pub fn tags(&self) -> &[Tag] {
		&self.tags
	}

	pub fn begin(&self) -> NfaIdx {
		NfaState::BEGIN.idx
	}
}

impl std::ops::Index<NfaIdx> for Tnfa {
	type Output = NfaState;

	fn index(&self, i: NfaIdx) -> &Self::Output {
		&self.states[i.0]
	}
}

impl std::ops::IndexMut<NfaIdx> for Tnfa {
	fn index_mut(&mut self, i: NfaIdx) -> &mut Self::Output {
		&mut self.states[i.0]
	}
}

impl NfaState {
	const BEGIN: Self = Self {
		idx: NfaIdx(0),
		name: Cow::Borrowed("begin"),
		transitions: IntervalTree::new(),
		spontaneous: Vec::new(),
		maybe_accepts_for_rule: None,
	};

	pub fn transitions(&self) -> &IntervalTree<u32, Vec<NfaIdx>> {
		&self.transitions
	}

	pub fn spontaneous(&self) -> &[SpontaneousTransition] {
		&self.spontaneous
	}

	pub fn accepts_for_rule(&self) -> Option<(RuleIdx, bool)> {
		self.maybe_accepts_for_rule
	}

	pub fn is_accepting(&self) -> bool {
		self.maybe_accepts_for_rule.is_some()
	}
}

impl<T> Policy<Vec<T>> for PolicyExtendUnique
where
	T: Ord + Clone,
{
	fn combine(&mut self, existing: &mut Vec<T>, mut new: Vec<T>) {
		let mut seen: BTreeSet<T> = BTreeSet::from_iter(existing.iter().cloned());
		new.retain(|x| seen.insert(x.clone()));
		existing.extend(new);
	}
}
