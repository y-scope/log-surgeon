//! Based on Angelo Borsotti and Ulya Trafimovich. 2022. A closer look at TDFA.
//! - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
//! - <https://arxiv.org/abs/2206.01398>
//!
mod graph_dot_output;
mod regex_construction;
mod search_decomposition;

use std::borrow::Cow;
use std::collections::BTreeSet;

pub use search_decomposition::Path;
pub use search_decomposition::PathComponent;
pub use search_decomposition::TarjanSccData;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::PolicyUnique;
use crate::regex::Regex;
use crate::schema::RootRule;
use crate::schema::RuleIdx;
use crate::schema::SubRule;

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
	pub fn tags(&self) -> &[Tag] {
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
			transitions: Transitions::Spontaneous(Vec::new()),
			maybe_accepts_for_rule: None,
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
		self.maybe_accepts_for_rule.is_some()
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
		}
	}

	fn successors(&self) -> Box<dyn Iterator<Item = NfaIdx> + '_> {
		match self {
			Self::Interval(transitions) => Box::new(transitions.iter().map(|(_interval, target)| *target)),
			Self::Spontaneous(transitions) => Box::new(transitions.iter().map(|transition| transition.target)),
		}
	}
}

impl Tag {
	pub fn sub_rule(&self) -> &SubRule {
		let (Self::StartCapture(sub_rule) | Self::StopCapture(sub_rule)) = self;
		sub_rule
	}
}
