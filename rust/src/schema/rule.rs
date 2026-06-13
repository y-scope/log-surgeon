use std::num::NonZero;
use std::sync::Arc;

use crate::dfa::Tdfa;
use crate::regex::AnchoredRegex;
use crate::regex::Regex;

/// Index in the schema, offset by/starting at 1.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RuleIdx(NonZero<u16>);

#[derive(Debug, Clone)]
pub struct RootRule {
	pub idx: RuleIdx,
	pub name: Arc<str>,
	/// Priority level given by the user.
	pub priority: i32,

	pub regex: AnchoredRegex,
	pub rule_info: Vec<RuleInfo>,

	pub dfa: Tdfa,
}

impl Eq for RootRule {}

impl PartialEq for RootRule {
	fn eq(&self, other: &Self) -> bool {
		(self.idx, &self.name, self.priority, &self.regex)
			.cmp(&(other.idx, &other.name, other.priority, &other.regex))
			.is_eq()
	}
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SubRule {
	pub name: String,
	pub regex: Regex,

	/// ID statically assigned left-to-right based on the regex pattern.
	/// For example, the pattern `(?<start>[a-z]+(?<rest>\.[a-z]+)*)|(?<start>[0-9]+)` has three non-zero capture IDs.
	/// When the pattern is actually matched,
	/// there may be multiple instances of capture ID 2 (corresponding to `"rest"`).
	/// The capture ID also differentiates between different capture groups given the same name,
	/// e.g. the two instances of `"start"` in the pattern.
	pub id: NonZero<u16>,
	/// ID of the parent capture, if any.
	pub parent_id: Option<NonZero<u16>>,
	/// Total number of nested captures (recursively/arbitrarily deep);
	/// `0` iff this is a "leaf" capture.
	pub descendents: usize,

	/// Qualified name w.r.t captures including the leading dot;
	/// a top-level capture is ".a", a second-level capture is ".a.b".
	pub qualified_name: Arc<str>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuleInfo {
	pub root_idx: RuleIdx,
	pub root_name: Arc<str>,

	/// If this is not a root rule, additional sub-rule info.
	pub maybe_sub_rule: Option<Box<SubRule>>,

	pub fully_qualified_name: Arc<str>,

	pub encoding_idx: Option<NonZero<u16>>,
}

impl std::ops::Index<Option<NonZero<u16>>> for RootRule {
	type Output = RuleInfo;

	fn index(&self, i: Option<NonZero<u16>>) -> &Self::Output {
		let i: usize = usize::from(i.map_or(0, NonZero::get));
		&self.rule_info[i]
	}
}

impl RuleIdx {
	/// cbindgen:ignore
	pub const NIL: Self = Self(NonZero::<u16>::MAX);

	pub const fn new(idx: NonZero<u16>) -> Self {
		Self(idx)
	}
}

impl From<RuleIdx> for NonZero<u16> {
	fn from(rule_idx: RuleIdx) -> Self {
		rule_idx.0
	}
}

impl From<RuleIdx> for u16 {
	fn from(rule_idx: RuleIdx) -> Self {
		rule_idx.0.get()
	}
}

impl From<NonZero<u16>> for RuleIdx {
	fn from(rule_idx: NonZero<u16>) -> Self {
		Self(rule_idx)
	}
}

impl std::fmt::Display for RuleIdx {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(fmt)
	}
}

impl RootRule {
	pub fn has_captures(&self) -> bool {
		// Always has itself as the $0$th `RuleInfo`.
		self.rule_info.len() > 1
	}
}

impl RuleInfo {
	pub fn sub_rule_name(&self) -> &str {
		if let Some(sub_rule) = &self.maybe_sub_rule {
			&sub_rule.name
		} else {
			""
		}
	}

	pub fn is_root(&self) -> bool {
		self.maybe_sub_rule.is_none()
	}
}
