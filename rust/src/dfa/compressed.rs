use std::num::NonZero;

use crate::dfa::BackupState;
use crate::dfa::MatchedRule;
use crate::dfa::Tdfa;
use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::parsing_spec::RuleIdx;
use crate::utils::SerdeArray;
use crate::utils::TarjanSccs;

/*
mod serde_ {
	use crate::utils::SerdeArray;
	use serde::Deserialize;
	use serde::Deserializer;
	use serde::Serialize;
	use serde::Serializer;
	use serde::de::Error;
	use serde::de::SeqAccess;
	use serde::de::Visitor;
	use serde::ser::SerializeSeq;
	use serde::ser::SerializeTuple;
	use std::marker::PhantomData;

	fn serialize<T, const N: usize, S>(vec: &Vec<[T; N]>, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut seq: S::SerializeSeq = serializer.serialize_seq(Some(vec.len()))?;
		for array in self.0.iter() {
			let mut tup: S::SerializeTuple = serializer.serialize_tuple(N)?;
			for element in array.iter() {
				tup.serialize_element(element)?;
			}
			seq.serialize_element(&tup.end())?
		}
		seq.end()
	}

	fn deserialize<'de, D>(deserializer: D) -> Result<[T; N], D::Error>
	where
		D: Deserializer<'de>,
	{
		let array: [T; N] = deserializer.deserialize_tuple(N, ArrayVisitor::<T, N>(PhantomData))?;
		Ok(SerdeArray(array))
	}
}
*/

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedDfa {
	pub intervals: Vec<Interval<u32>>,
	pub accepts_for_rule: Vec<Option<RuleIdx>>,
	pub ascii_transitions: Vec<SerdeArray<[u16; 0x80]>>,
	pub non_ascii_transitions: Vec<u16>,
}

impl Tdfa {
	pub fn compress(&self) -> CompressedDfa {
		use crate::interval_tree::PolicyNoop;

		let tarjan: TarjanSccs = TarjanSccs::tarjan_scc(&self.states, |state| {
			state
				.transitions
				.iter()
				.map(|(_interval, transition)| transition.target)
		});

		let mut all_intervals: IntervalTree<u32, ()> = IntervalTree::new();
		for state in self.states.iter() {
			for (interval, _transition) in state.transitions.iter() {
				all_intervals.insert(interval, (), PolicyNoop);
			}
		}
		let mut all_intervals: Vec<Interval<u32>> = all_intervals
			.iter()
			.map(|(interval, _transition)| interval)
			.collect::<Vec<_>>();

		all_intervals.retain(|interval| interval.end() >= 0x80);
		if let Some(first) = all_intervals.first_mut() {
			if first.start() == 0x80 {
				*first = Interval::new(0x80, first.end());
			}
		}

		let mut accepts_for_rule: Vec<Option<RuleIdx>> = Vec::with_capacity(self.states.len());
		let mut ascii_transitions: Vec<SerdeArray<[u16; 0x80]>> = Vec::with_capacity(self.states.len());
		let mut non_ascii_transitions: Vec<u16> = Vec::with_capacity(self.states.len() * all_intervals.len());

		for state in self.states.iter() {
			accepts_for_rule.push(state.accepting_rule);
			let mut ascii: [u16; 0x80] = [0; 0x80];
			for (i, target) in ascii.iter_mut().enumerate() {
				if let Some(transition) = state.transitions.lookup(u32::try_from(i).unwrap()) {
					assert_ne!(transition.target, 0);
					*target = u16::try_from(transition.target).unwrap();
				}
			}
			ascii_transitions.push(SerdeArray(ascii));
			for &interval in all_intervals.iter() {
				if let Some(transition) = state.transitions.lookup(interval.start()) {
					let target: u16 = u16::try_from(transition.target).unwrap();
					assert_ne!(transition.target, 0);
					non_ascii_transitions.push(u16::try_from(transition.target).unwrap());
				} else {
					non_ascii_transitions.push(0);
				}
			}
		}

		CompressedDfa {
			intervals: all_intervals,
			accepts_for_rule,
			ascii_transitions,
			non_ascii_transitions,
		}
	}
}

impl CompressedDfa {
	pub fn execute<'input>(&self, input: &'input str, last_was_delimited: u32) -> Option<MatchedRule<'input>> {
		let anchor_transition: u16 = self.lookup_next_state(0, last_was_delimited)?.get();
		let mut current_state: u16 = anchor_transition;

		let mut maybe_backup: Option<BackupState> = None;

		for (pos, ch) in input.char_indices().chain(std::iter::once((input.len(), '\n'))) {
			if let Some(next_state) = self.lookup_next_state(current_state, u32::from(ch)) {
				current_state = next_state.get();
				if let Some(rule_idx) = self.accepts_for_rule[usize::from(current_state)] {
					maybe_backup = Some(BackupState {
						rule_idx,
						consumed: pos,
					});
				}
			} else {
				break;
			}
		}

		let backup: BackupState = maybe_backup?;

		Some(MatchedRule {
			rule_idx: backup.rule_idx,
			lexeme: &input[..backup.consumed],
		})
	}

	fn lookup_next_state(&self, current_state: u16, ch: u32) -> Option<NonZero<u16>> {
		let current_state: usize = usize::from(current_state);
		let next_state: u16 = if ch < 0x80 {
			self.ascii_transitions[current_state][ch as usize]
		} else {
			let char_class: usize = self.char_to_class(ch);
			if char_class == usize::MAX {
				return None;
			}
			self.non_ascii_transitions[(current_state * self.intervals.len()) + char_class]
		};
		NonZero::new(next_state)
	}

	fn char_to_class(&self, ch: u32) -> usize {
		// TODO refactor with interval tree
		let i: usize = self.intervals.partition_point(|interval| interval.end() < ch);
		if let Some(interval) = self.intervals.get(i) {
			if interval.start() <= ch {
				return i;
			}
		}
		usize::MAX
	}
}
