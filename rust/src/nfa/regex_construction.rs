use std::collections::BTreeSet;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::PolicyUnique;
use crate::nfa::CaptureTag;
use crate::nfa::NfaIdx;
use crate::nfa::SpontaneousTransition;
use crate::nfa::SpontaneousTransitionKind;
use crate::nfa::Tnfa;
use crate::nfa::Transitions;
use crate::parsing_spec::RootRule;
use crate::parsing_spec::RuleIdx;
use crate::parsing_spec::SubRule;
use crate::regex::Regex;

impl Tnfa {
	pub fn from_single_rule(rule_idx: RuleIdx, regex: &Regex) -> Self {
		let mut nfa: Self = Self::new();

		let rule_start: NfaIdx = NfaIdx::BEGIN;
		let rule_end: NfaIdx = nfa.new_state("end");

		let tags: BTreeSet<CaptureTag> = nfa.build_regex_nfa::<true>(rule_idx, regex, rule_start, rule_end);
		nfa[rule_end].maybe_accepts_for_rule = Some(rule_idx);

		nfa.tags = tags;

		nfa
	}

	pub fn from_regex(regex: &Regex) -> Tnfa {
		Self::from_single_rule(RuleIdx::NIL, regex)
	}

	/// `WITH_CAPTURES` also controls "with(out) anchors"; with captures <=> without anchors.
	/// A `Tnfa` without captures additionally has transitions for "before" and "after" characters,
	/// used to match anchors/delimiters/any character (if the rule is unanchored).
	///
	/// See [`crate::dfa::Tdfa::execute_without_captures`] for more details.
	pub fn from_rules<'a, const WITH_CAPTURES: bool, Rules>(rules: Rules, delimiters: &str) -> Self
	where
		Rules: IntoIterator<Item = &'a RootRule>,
	{
		let mut nfa: Self = Self::new();

		let mut tags: BTreeSet<CaptureTag> = BTreeSet::new();

		let mut spontaneous: Vec<SpontaneousTransition> = Vec::new();

		for rule in rules.into_iter() {
			let anchored_rule_start: NfaIdx = nfa.new_state(format!(
				"anchored rule {} ('{}') start",
				rule.idx,
				rule.name.escape_default()
			));
			let anchored_rule_end: NfaIdx = nfa.new_state(format!(
				"anchored rule {} ('{}') end",
				rule.idx,
				rule.name.escape_default()
			));
			let rule_start: NfaIdx = nfa.new_state(format!("rule '{}' start", rule.name));
			let rule_end: NfaIdx = nfa.new_state(format!("rule '{}' end", rule.name));

			spontaneous.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target: anchored_rule_start,
			});

			if rule.regex.anchor_before {
				nfa[anchored_rule_start].transitions = Transitions::Interval(IntervalTree::from_iter(
					delimiters
						.chars()
						.map(|ch| (Interval::new(u32::from(ch), u32::from(ch)), rule_start, PolicyUnique)),
				));
			} else {
				nfa[anchored_rule_start].transitions = Transitions::Interval(IntervalTree::from_iter(std::iter::once(
					(Interval::new(0, u32::from(char::MAX)), rule_start, PolicyUnique),
				)));
			}

			tags = &tags | &nfa.build_regex_nfa::<WITH_CAPTURES>(rule.idx, &rule.regex.regex, rule_start, rule_end);

			if rule.regex.anchor_after {
				nfa[rule_end].transitions =
					Transitions::Interval(IntervalTree::from_iter(delimiters.chars().map(|ch| {
						(
							Interval::new(u32::from(ch), u32::from(ch)),
							anchored_rule_end,
							PolicyUnique,
						)
					})));
			} else {
				nfa[rule_end].transitions = Transitions::Interval(IntervalTree::from_iter(std::iter::once((
					Interval::new(0, u32::from(char::MAX)),
					anchored_rule_end,
					PolicyUnique,
				))));
			}

			nfa[anchored_rule_end].maybe_accepts_for_rule = Some(rule.idx);
		}

		nfa[NfaIdx::BEGIN].transitions = Transitions::Spontaneous(spontaneous);

		nfa.tags = tags;

		nfa
	}

	fn build_regex_nfa<const WITH_CAPTURES: bool>(
		&mut self,
		rule_idx: RuleIdx,
		regex: &Regex,
		mut current: NfaIdx,
		target: NfaIdx,
	) -> BTreeSet<CaptureTag> {
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
				if WITH_CAPTURES {
					self.capture(rule_idx, sub_rule, current, target)
				} else {
					self.build_regex_nfa::<false>(rule_idx, &sub_rule.regex, current, target)
				}
			},
			Regex::BracketedRanges { negated, items } => {
				let intervals: Vec<Interval<u32>> = if *negated {
					let mut intervals: Vec<Interval<u32>> = Vec::with_capacity(items.len());
					for &(start, end) in items.iter() {
						// Should have been verified during regex pattern parsing.
						assert!(start <= end);

						intervals.push(Interval::new(u32::from(start), u32::from(end)));
					}
					Interval::complement(&mut intervals)
				} else {
					items
						.iter()
						.map(|&(start, end)| Interval::new(u32::from(start), u32::from(end)))
						.collect::<Vec<_>>()
				};
				self[current].transitions = Transitions::Interval(IntervalTree::from_iter(
					intervals.into_iter().map(|interval| (interval, target, PolicyUnique)),
				));
				BTreeSet::new()
			},
			Regex::KleeneClosure(item) => {
				let item_start: NfaIdx = self.new_state(format!("kleene item start ({item})"));
				let item_end: NfaIdx = self.new_state(format!("kleene item end ({item})"));
				let item_skip: NfaIdx = self.new_state(format!("kleene item skip ({item})"));

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

				let tags: BTreeSet<CaptureTag> =
					self.build_regex_nfa::<WITH_CAPTURES>(rule_idx, item, item_start, item_end);

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
			Regex::KleenePlus(item) => {
				self.build_regex_nfa::<WITH_CAPTURES>(rule_idx, &item.wrap_as_desugared_kleene_plus(), current, target)
			},
			Regex::BoundedRepetition { min, max, item } => {
				// Should have been verified during regex pattern parsing.
				assert!(*max > 0);
				assert!(min <= max);

				let middle: NfaIdx = self.new_state(format!("bounded middle {min}..={max} ({item})"));

				let mut tags: BTreeSet<CaptureTag> = BTreeSet::new();
				for i in 0..*min {
					let sub_target: NfaIdx = self.new_state(format!("bounded {i} of {min}..={max} ({item})"));
					tags.append(&mut self.build_regex_nfa::<WITH_CAPTURES>(rule_idx, item, current, sub_target));
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
					let sub_skip: NfaIdx = self.new_state(format!("bounded {i} of {min}..={max} break ({item})"));
					let sub_have: NfaIdx = self.new_state(format!("bounded {i} of {min}..={max} continue ({item})"));
					let sub_target: NfaIdx = self.new_state(format!("bounded {i} of {min}..={max} success ({item})"));

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

					tags.append(&mut self.build_regex_nfa::<WITH_CAPTURES>(rule_idx, item, sub_have, sub_target));
					current = sub_target;
				}

				self[current].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
					kind: SpontaneousTransitionKind::Epsilon,
					target,
				}]);
				tags
			},
			Regex::Sequence(items) => {
				let mut tags: BTreeSet<CaptureTag> = BTreeSet::new();
				// TODO this is for search... handle it better?
				if items.is_empty() {
					self[current].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
						kind: SpontaneousTransitionKind::Epsilon,
						target,
					}]);
				}
				for (i, sub_item) in items.iter().enumerate() {
					let sub_target: NfaIdx = if i + 1 < items.len() {
						self.new_state(format!("sequence sub target (of {sub_item})"))
					} else {
						target
					};
					tags.append(&mut self.build_regex_nfa::<WITH_CAPTURES>(rule_idx, sub_item, current, sub_target));
					current = sub_target;
				}
				tags
			},
			Regex::Alternation(items) => self.alternate::<WITH_CAPTURES>(rule_idx, items, current, target),
			Regex::Placeholder { item, .. } => self.build_regex_nfa::<WITH_CAPTURES>(rule_idx, item, current, target),
		}
	}

	fn capture(&mut self, rule: RuleIdx, sub_rule: &SubRule, current: NfaIdx, target: NfaIdx) -> BTreeSet<CaptureTag> {
		let start_capture: CaptureTag = CaptureTag::StartCapture(sub_rule.clone());
		let end_capture: CaptureTag = CaptureTag::StopCapture(sub_rule.clone());

		let sub_start: NfaIdx = self.new_state(format!("capture {} started", sub_rule.name.escape_default()));
		let sub_end: NfaIdx = self.new_state(format!("capture {} ended", sub_rule.name.escape_default()));

		self[current].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
			kind: SpontaneousTransitionKind::Positive(start_capture.clone()),
			target: sub_start,
		}]);

		let mut tags: BTreeSet<CaptureTag> = self.build_regex_nfa::<true>(rule, &sub_rule.regex, sub_start, sub_end);

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
		rule_idx: RuleIdx,
		items: &[Regex],
		current: NfaIdx,
		target: NfaIdx,
	) -> BTreeSet<CaptureTag> {
		let mut tags: BTreeSet<CaptureTag> = BTreeSet::new();
		let mut intermediate_states: Vec<(NfaIdx, BTreeSet<CaptureTag>)> = Vec::new();

		let mut starts: Vec<SpontaneousTransition> = Vec::new();
		for sub_item in items.iter() {
			let sub_start: NfaIdx = self.new_state("alternate sub start (of {sub_item})");
			let sub_target: NfaIdx = self.new_state("alternate sub target (of {sub_item})");

			starts.push(SpontaneousTransition {
				kind: SpontaneousTransitionKind::Epsilon,
				target: sub_start,
			});

			intermediate_states.push((
				sub_target,
				self.build_regex_nfa::<CAPTURE>(rule_idx, sub_item, sub_start, sub_target),
			));
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

	fn negative_tags(&mut self, tags: impl IntoIterator<Item = CaptureTag>, mut current: NfaIdx) -> NfaIdx {
		for t in tags {
			let next: NfaIdx = self.new_state("negative tag ({t:?})");
			self[current].transitions = Transitions::Spontaneous(vec![SpontaneousTransition {
				kind: SpontaneousTransitionKind::Negative(t),
				target: next,
			}]);
			current = next;
		}
		current
	}
}
