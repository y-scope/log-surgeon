//! Based on Angelo Borsotti and Ulya Trafimovich. 2022. A closer look at TDFA.
//! - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
//! - <https://arxiv.org/abs/2206.01398>
//!
mod graph_dot_output;
mod search_decomposition;
pub use search_decomposition::*;

use std::borrow::Cow;
use std::collections::BTreeSet;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::PolicyUnique;
use crate::regex::Regex;
use crate::regex::SubRule;
use crate::schema::RootRule;
use crate::schema::RuleIdx;

#[derive(Debug, Clone)]
pub struct Tnfa {
	states: Vec<NfaState>,
	tags: Vec<Tag>,
}

#[derive(Debug, Clone)]
pub struct NfaState {
	/// ID and also an index into an [`Nfa`]'s list of states.
	pub idx: NfaIdx,
	pub transitions: Transitions,
	pub maybe_accepts_for_rule: Option<RuleIdx>,
	pub name: Cow<'static, str>,
}

/// Newtype wrapper around a `usize` index.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct NfaIdx(usize);

impl std::fmt::Display for NfaIdx {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!("q{}", self.0))
	}
}

#[derive(Debug, Clone)]
pub enum Transitions {
	Interval(IntervalTree<u32, NfaIdx>),
	Spontaneous(Vec<SpontaneousTransition>),
}

#[derive(Debug, Clone)]
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
	StartCapture(SubRule),
	StopCapture(SubRule),
}

impl Tnfa {
	pub fn for_single_rule(rule: RuleIdx, regex: &Regex) -> Self {
		let mut nfa: Self = Self {
			states: vec![NfaState {
				idx: NfaIdx::BEGIN,
				name: Cow::Borrowed("begin"),
				transitions: Transitions::Spontaneous(Vec::new()),
				maybe_accepts_for_rule: None,
			}],
			tags: Vec::new(),
		};

		let rule_start: NfaIdx = NfaIdx::BEGIN;
		let rule_end: NfaIdx = nfa.new_state("end");

		let tags: BTreeSet<Tag> = nfa.build::<true>(rule, &regex, rule_start, rule_end);
		nfa[rule_end].maybe_accepts_for_rule = Some(rule);

		nfa.tags = tags.into_iter().collect::<Vec<_>>();

		nfa
	}

	pub fn for_regex(regex: &Regex) -> Tnfa {
		Self::for_single_rule(RuleIdx::NIL, regex)
	}

	pub fn for_rules<'a, const WITH_CAPTURES: bool, Rules>(rules: Rules, delimiters: String) -> Self
	where
		Rules: IntoIterator<Item = &'a RootRule>,
	{
		let mut nfa: Self = Self {
			states: vec![NfaState {
				idx: NfaIdx::BEGIN,
				name: Cow::Borrowed("begin"),
				transitions: Transitions::Spontaneous(Vec::new()),
				maybe_accepts_for_rule: None,
			}],
			tags: Vec::new(),
		};

		let mut tags: BTreeSet<Tag> = BTreeSet::new();

		let mut spontaneous: Vec<SpontaneousTransition> = Vec::new();

		for rule in rules.into_iter() {
			let rule_start: NfaIdx = nfa.new_state(format!("rule '{}' start", rule.name));
			let rule_end: NfaIdx = nfa.new_state(format!("rule '{}' end", rule.name));

			spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target: rule_start,
			});

			let (rule_inner_start, rule_inner_end): (NfaIdx, NfaIdx) =
				if !WITH_CAPTURES {
					let rule_inner_start: NfaIdx = nfa.new_state(format!("rule '{}' inner start", rule.name));
					let rule_inner_end: NfaIdx = nfa.new_state(format!("rule '{}' inner end", rule.name));

					if rule.regex.anchor_before {
						nfa[rule_start].transitions =
							Transitions::Interval(IntervalTree::from_iter(delimiters.chars().map(|ch| {
								(
									Interval::new(u32::from(ch), u32::from(ch)),
									rule_inner_start,
									PolicyUnique,
								)
							})));
					} else {
						nfa[rule_start].transitions = Transitions::Interval(IntervalTree::from_iter(std::iter::once(
							(Interval::new(0, u32::from(char::MAX)), rule_inner_start, PolicyUnique),
						)));
					}
					(rule_inner_start, rule_inner_end)
				} else {
					(rule_start, rule_end)
				};

			tags = &tags | &nfa.build::<WITH_CAPTURES>(rule.idx, &rule.regex.inner, rule_inner_start, rule_inner_end);

			if !WITH_CAPTURES {
				if rule.regex.anchor_after {
					nfa[rule_inner_end].transitions = Transitions::Interval(IntervalTree::from_iter(
						delimiters
							.chars()
							.map(|ch| (Interval::new(u32::from(ch), u32::from(ch)), rule_end, PolicyUnique)),
					));
				} else {
					nfa[rule_inner_end].transitions = Transitions::Interval(IntervalTree::from_iter(std::iter::once(
						(Interval::new(0, u32::from(char::MAX)), rule_end, PolicyUnique),
					)));
				}
			}

			nfa[rule_end].maybe_accepts_for_rule = Some(rule.idx);
		}

		nfa[NfaIdx::BEGIN].transitions = Transitions::Spontaneous(spontaneous);

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
			transitions: Transitions::Spontaneous(Vec::new()),
			maybe_accepts_for_rule: None,
		};
		self.states.push(state);
		idx
	}

	fn build<const CAPTURE: bool>(
		&mut self,
		rule: RuleIdx,
		regex: &Regex,
		mut current: NfaIdx,
		target: NfaIdx,
	) -> BTreeSet<Tag> {
		assert_eq!(self[current].transitions.len(), 0);
		match regex {
			Regex::AnyChar => {
				self[current].transitions = Transitions::Interval(IntervalTree::from_iter(std::iter::once((
					Interval::new(0, u32::from(char::MAX)),
					target,
					PolicyUnique,
				))));
				BTreeSet::new()
			},
			&Regex::Literal(ch) => {
				self[current].transitions = Transitions::Interval(IntervalTree::from_iter(std::iter::once((
					Interval::new(u32::from(ch), u32::from(ch)),
					target,
					PolicyUnique,
				))));
				BTreeSet::new()
			},
			Regex::Capture(sub_rule) => {
				if CAPTURE {
					self.capture(rule, &sub_rule, current, target)
				} else {
					self.build::<false>(rule, &sub_rule.regex, current, target)
				}
			},
			Regex::Group { negated, items } => {
				if *negated {
					let mut intervals: Vec<Interval<u32>> = Vec::with_capacity(items.len());
					for &(start, end) in items.iter() {
						// Should have been verified during regex pattern parsing.
						assert!(start <= end);

						intervals.push(Interval::new(u32::from(start), u32::from(end)));
					}
					self[current].transitions = Transitions::Interval(IntervalTree::from_iter(
						Interval::complement(&mut intervals)
							.into_iter()
							.map(|interval| (interval, target, PolicyUnique)),
					));
				} else {
					self[current].transitions =
						Transitions::Interval(IntervalTree::from_iter(items.iter().map(|&(start, end)| {
							// Should have been verified during regex pattern parsing.
							assert!(start <= end);

							(Interval::new(u32::from(start), u32::from(end)), target, PolicyUnique)
						})));
				}
				BTreeSet::new()
			},
			Regex::KleeneClosure(item) => {
				let item_start: NfaIdx = self.new_state("kleene item start");
				let item_end: NfaIdx = self.new_state("kleene item end");
				let item_skip: NfaIdx = self.new_state("kleene skip");

				self[current].transitions = Transitions::Spontaneous(vec![
					SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target: item_start,
					},
					SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target: item_skip,
					},
				]);

				let tags: BTreeSet<Tag> = self.build::<CAPTURE>(rule, item, item_start, item_end);

				self[item_end].transitions = Transitions::Spontaneous(vec![
					SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target: item_start,
					},
					SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target,
					},
				]);

				let after_negative_tags: NfaIdx = self.negative_tags(tags.iter().cloned(), item_skip);
				self[after_negative_tags].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target,
				}]);

				tags
			},
			Regex::KleenePlus(item) => self.build::<CAPTURE>(rule, &item.into_kleene_plus(), current, target),
			Regex::BoundedRepetition { min, max, item } => {
				// Should have been verified during regex pattern parsing.
				assert!(*max > 0);
				assert!(min <= max);

				let middle: NfaIdx = self.new_state("bounded middle");

				let mut tags: BTreeSet<Tag> = BTreeSet::new();
				for _ in 0..*min {
					let sub_target: NfaIdx = self.new_state("bounded sub 1/2 target");
					tags.append(&mut self.build::<CAPTURE>(rule, item, current, sub_target));
					current = sub_target;
				}

				self[current].transitions = Transitions::Spontaneous(vec![
					SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target: middle,
					},
					SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target,
					},
				]);

				current = middle;
				for i in *min..*max {
					let sub_skip: NfaIdx = self.new_state("bounded sub skip");
					let sub_have: NfaIdx = self.new_state("bounded sub have");
					let sub_target: NfaIdx = if i + 1 < *max {
						self.new_state("bounded sub 2/2 target")
					} else {
						target
					};

					self[current].transitions = Transitions::Spontaneous(vec![
						SpontaneousTransition {
							kind: SpontaneousTransitionKind::Epsilon,
							target: sub_skip,
						},
						SpontaneousTransition {
							kind: SpontaneousTransitionKind::Epsilon,
							target: sub_have,
						},
					]);

					self[sub_skip].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target,
					}]);

					tags.append(&mut self.build::<CAPTURE>(rule, item, sub_have, sub_target));
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
					tags.append(&mut self.build::<CAPTURE>(rule, sub_item, current, sub_target));
					current = sub_target;
				}
				tags
			},
			Regex::Alternation(items) => self.alternate::<CAPTURE>(rule, items, current, target),
		}
	}

	fn capture(&mut self, rule: RuleIdx, sub_rule: &SubRule, current: NfaIdx, target: NfaIdx) -> BTreeSet<Tag> {
		let start_capture: Tag = Tag::StartCapture(sub_rule.clone());
		let end_capture: Tag = Tag::StopCapture(sub_rule.clone());

		let sub_start: NfaIdx = self.new_state("capture started");
		let sub_end: NfaIdx = self.new_state("capture before end");

		self[current].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(start_capture.clone()),
			target: sub_start,
		}]);

		let mut tags: BTreeSet<Tag> = self.build::<true>(rule, &sub_rule.regex, sub_start, sub_end);

		self[sub_end].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(end_capture.clone()),
			target,
		}]);

		tags.insert(start_capture);
		tags.insert(end_capture);

		tags
	}

	fn alternate<const CAPTURE: bool>(
		&mut self,
		rule: RuleIdx,
		items: &[Regex],
		current: NfaIdx,
		target: NfaIdx,
	) -> BTreeSet<Tag> {
		let mut tags: BTreeSet<Tag> = BTreeSet::new();
		let mut intermediate_states: Vec<(NfaIdx, BTreeSet<Tag>)> = Vec::new();

		let mut starts: Vec<SpontaneousTransition> = Vec::new();
		for sub_item in items.iter() {
			let sub_start: NfaIdx = self.new_state("alternate sub start");
			let sub_target: NfaIdx = self.new_state("alternate sub target");

			starts.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target: sub_start,
			});

			intermediate_states.push((sub_target, self.build::<CAPTURE>(rule, sub_item, sub_start, sub_target)));
		}
		self[current].transitions = Transitions::Spontaneous(starts);

		for (i, (sub_state, sub_tags)) in intermediate_states.iter().enumerate() {
			let mut sub_current: NfaIdx = *sub_state;
			for (other, (_, other_tags)) in intermediate_states.iter().enumerate() {
				if other == i {
					continue;
				}

				sub_current = self.negative_tags(other_tags.iter().cloned(), sub_current);
			}

			self[sub_current].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target,
			}]);

			// `&BTreeSet<_>` implements `BitOr`, `BTreeSet<_>` does not.
			// `sub_tags` from the loop is already a `&BTreeSet<_>`.
			tags = &tags | sub_tags;
		}

		tags
	}

	fn negative_tags(&mut self, tags: impl Iterator<Item = Tag>, mut current: NfaIdx) -> NfaIdx {
		for t in tags {
			let next: NfaIdx = self.new_state("negative tags");
			self[current].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
				kind: SpontaneousTransitionKind::Negative(t),
				target: next,
			}]);
			current = next;
		}
		current
	}
}

impl Tnfa {
	pub fn tags(&self) -> &[Tag] {
		&self.tags
	}

	pub fn begin(&self) -> NfaIdx {
		NfaIdx::BEGIN
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
	pub fn is_accepting(&self) -> bool {
		self.maybe_accepts_for_rule.is_some()
	}
}

impl NfaIdx {
	const BEGIN: NfaIdx = Self(0);
}

impl Transitions {
	pub fn len(&self) -> usize {
		match self {
			Self::Interval(transitions) => transitions.len(),
			Self::Spontaneous(transitions) => transitions.len(),
		}
	}

	fn successors(&self) -> Vec<NfaIdx> {
		match self {
			Self::Interval(transitions) => transitions
				.iter()
				.map(|(_interval, target)| *target)
				.collect::<Vec<_>>(),
			Self::Spontaneous(transitions) => transitions
				.iter()
				.map(|transition| transition.target)
				.collect::<Vec<_>>(),
		}
	}
}

impl Tag {
	pub fn capture(&self) -> &SubRule {
		let (Self::StartCapture(sub_rule) | Self::StopCapture(sub_rule)) = self;
		sub_rule
	}
}
