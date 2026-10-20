//! Based on Angelo Borsotti and Ulya Trafimovich. 2022. A closer look at TDFA.
//! - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
//! - <https://arxiv.org/abs/2206.01398>
//!

mod graph_dot_output;
mod ops;
mod regex_construction;
mod search_decomposition;
#[cfg(test)]
mod test;

use std::borrow::Cow;
use std::collections::BTreeSet;

pub use search_decomposition::Path;
pub use search_decomposition::PathComponent;

use crate::interval_tree::IntervalTree;
use crate::parsing_spec::EncodingIdx;
use crate::parsing_spec::RuleIdx;

#[derive(Debug, Clone)]
pub struct Tnfa {
	pub states: Vec<NfaState>,
	tags: BTreeSet<CaptureTag>,
}

#[derive(Debug, Clone)]
pub struct NfaState {
	/// ID and also an index into an [`Nfa`]'s list of states.
	pub idx: NfaIdx,
	pub transitions: Transitions,
	/// If this is an accepting state: the rule it accepts for,
	/// and the encoding (if any) of the matched lexeme.
	pub maybe_accepting_data: Option<(RuleIdx, Option<EncodingIdx>)>,
	/// Not strictly needed, but useful for debugging (including DOT output).
	///
	/// Note 2026-09-16 (0d98b9b9a09fa072fc676a069e55a3a07bdf5c74):
	/// No major performance impact of formatting all the detailed state names.
	pub name: Cow<'static, str>,
}

/// Newtype wrapper around a `usize` index.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NfaIdx(usize);

/// By construction, an NFA state has homogenous transitions of one of these kinds.
///
/// Extra spontaneous transitions can be used to preserve this property,
/// even if they aren't otherwise necessary.
///
/// Furthermore, the predecessor states of an NFA state should,
/// by construction, also have the same kind of transitions.
///
/// See [`crate::dfa::Kernel`] for the "use" of this invariant.
#[derive(Debug, Clone)]
pub enum Transitions {
	/// Empty transitions should use this variant;
	/// see [`Tnfa::intersect`] for the reason.
	Interval(IntervalTree<u32, NfaIdx>),
	/// Priority-ordered, untagged epsilon transitions.
	Spontaneous(Vec<NfaIdx>),
	/// A single tagged transition;
	/// by construction, tagged transitions are created on "fresh" states
	/// (with no existing transitions),
	/// and such states will not have other transitions.
	Tagged {
		tag: CaptureTag,
		positive: bool,
		target: NfaIdx,
	},
}

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CaptureTag {
	/// The root rule this capture belongs to; a capture ID is local to its rule.
	pub rule_idx: RuleIdx,
	pub capture_id: u16,
	pub maybe_encoding_idx: Option<EncodingIdx>,
	/// "Open" before "close" in the derived ordering.
	pub is_close: bool,
}

impl Tnfa {
	pub const BLANK: Self = Self {
		states: Vec::new(),
		tags: BTreeSet::new(),
	};

	pub fn new() -> Self {
		Self {
			states: vec![NfaState {
				idx: NfaIdx::BEGIN,
				name: Cow::Borrowed("begin"),
				transitions: Transitions::Interval(IntervalTree::new()),
				maybe_accepting_data: None,
			}],
			tags: BTreeSet::new(),
		}
	}

	pub fn epsilon() -> Self {
		Self {
			states: vec![NfaState {
				idx: NfaIdx::BEGIN,
				name: Cow::Borrowed("begin"),
				transitions: Transitions::Interval(IntervalTree::new()),
				maybe_accepting_data: Some((RuleIdx::NIL, None)),
			}],
			tags: BTreeSet::new(),
		}
	}

	/// Set the [`RuleIdx`] of all accepting states (clearing any encoding),
	/// leaving capture tags (which carry their own [`CaptureTag::rule_idx`]) untouched.
	pub fn with_accepting_rule(mut self, rule_idx: RuleIdx) -> Self {
		for state in self.states.iter_mut() {
			if let Some(accepting_data) = &mut state.maybe_accepting_data {
				*accepting_data = (rule_idx, None);
			}
		}
		self
	}

	// XXX: Can we refactor/redesign the outside code so this isn't needed?
	/// A constant-time check, sound for any `Tnfa`: `true` only if no string is accepted;
	/// i.e. there are no states, or the only state is non-accepting
	/// (any transitions can only loop back to itself).
	///
	/// Exact (equivalent to `!self.can_accept()`) for the output of [`Tnfa::intersect`],
	/// which prunes all states that cannot reach an accepting state,
	/// leaving a single non-accepting state if the intersection is empty.
	pub fn definitely_cannot_accept(&self) -> bool {
		match self.states.as_slice() {
			[] => true,
			[only] => !only.is_accepting(),
			_ => false,
		}
	}

	pub fn tags(&self) -> &BTreeSet<CaptureTag> {
		&self.tags
	}

	fn new_state<LikeString>(&mut self, name: LikeString) -> NfaIdx
	where
		LikeString: Into<Cow<'static, str>>,
	{
		let idx: NfaIdx = NfaIdx(self.states.len());
		let state: NfaState = NfaState {
			idx,
			name: name.into(),
			transitions: Transitions::Interval(IntervalTree::new()),
			maybe_accepting_data: None,
		};
		self.states.push(state);
		idx
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
		self.maybe_accepting_data.is_some()
	}
}

impl NfaIdx {
	pub const BEGIN: NfaIdx = Self(0);
}

impl std::fmt::Display for NfaIdx {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!("q{}", self.0))
	}
}

impl Transitions {
	pub fn len(&self) -> usize {
		match self {
			Self::Interval(transitions) => transitions.len(),
			Self::Spontaneous(transitions) => transitions.len(),
			Self::Tagged { .. } => 1,
		}
	}

	/// Returns an iterator of successor [`NfaIdx`]s.
	/// The elided `'_` lifetime in the return type refers to the lifetime of `&self`,
	/// and means that the returned `dyn Iterator` will/must be valid
	/// for at least the lifetime of `&self`.
	fn successors(&self) -> Box<dyn Iterator<Item = NfaIdx> + '_> {
		/*
		match self {
			Self::Interval(transitions) => Box::new(transitions.iter().map(|(_interval, target)| *target)),
			Self::Spontaneous(transitions) => Box::new(transitions.iter().copied()),
			Self::Tagged { target, .. } => Box::new(std::iter::once(*target)),
		}
		*/
		let iter: &mut dyn Iterator<Item = NfaIdx> = match self {
			Self::Interval(transitions) => {
				&mut transitions.iter().map(|(_interval, target)| *target)
			},
			Self::Spontaneous(transitions) => &mut transitions.iter().copied(),
			Self::Tagged { target, .. } => &mut std::iter::once(*target),
		};
		let mut successors: Vec<NfaIdx> = Vec::from_iter(iter);
		successors.sort();
		successors.dedup();
		Box::new(successors.into_iter())
	}
}
