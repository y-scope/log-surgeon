use std::num::NonZero;
use std::sync::Arc;

use crate::dfa::Tdfa;
use crate::parsing_spec::Encoding;
use crate::regex::AnchoredRegex;
use crate::regex::Regex;

/// Index in the parsing specification, offset by/starting at 1.
/// `Option<RuleIdx>` is ABI equivalent to `u16` (for FFI);
/// `None`/`0` represents static text fragments.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct RuleIdx(NonZero<u16>);

/// A handle to a [`Regex::Capture`] within a rule.
///
/// A capture ID is local to its root rule, so both are needed to resolve one;
/// see [`ParsingSpec::index`](crate::parsing_spec::ParsingSpec).
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct CaptureRef {
	pub rule_idx: RuleIdx,
	pub capture_id: u16,
}

/// A [`CaptureRef`] together with the fully-qualified name resolved for it.
///
/// Carrying the name makes the value usable without a [`ParsingSpec`] in scope
/// (e.g. when rendering a path, or as a stable `Hash` key).
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct ResolvedCapture {
	pub capture: CaptureRef,
	pub fully_qualified_name: Arc<str>,
}

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

/// The metadata of a (root or sub-) rule, i.e. of the corresponding [`Regex::Capture`].
///
/// The root rule/capture has id `0` and is its own parent.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuleInfo {
	pub root_idx: RuleIdx,

	/// The capture ID of this (sub-)rule within `root_idx`; see [`CaptureRef`].
	pub id: u16,
	/// The name as written in the pattern.
	pub name: Arc<str>,
	/// ID of the parent capture; `0` for the root rule (which is its own parent).
	pub parent_id: u16,
	/// Total number of nested captures (recursively/arbitrarily deep);
	/// `0` iff this is a "leaf" capture.
	pub descendants: u16,

	/// Qualified name w.r.t captures including the leading dot;
	/// a top-level capture is ".a", a second-level capture is ".a.b".
	pub qualified_name: Arc<str>,
	/// Qualified name including the root rule, e.g. "root.a.b".
	pub fully_qualified_name: Arc<str>,
}

impl std::ops::Index<Option<NonZero<u16>>> for RootRule {
	type Output = RuleInfo;

	fn index(&self, i: Option<NonZero<u16>>) -> &Self::Output {
		let i: usize = usize::from(i.map_or(0, NonZero::get));
		&self.rule_info[i]
	}
}

impl std::ops::Index<u16> for RootRule {
	type Output = RuleInfo;

	fn index(&self, i: u16) -> &Self::Output {
		&self.rule_info[usize::from(i)]
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
	/// Construct a new root rule;
	/// initialize the [`RuleInfo`] for the root rule and any/all sub-rules.
	pub fn new(
		idx: RuleIdx,
		name: Arc<str>,
		priority: i32,
		regex: AnchoredRegex,
		encodings: &[Arc<Encoding>],
	) -> Self {
		let rule_info: Vec<RuleInfo> =
			Vec::from_iter(regex.captures.iter().map(|capture| RuleInfo {
				root_idx: idx,
				id: capture.id,
				name: capture.name.clone(),
				parent_id: capture.parent_id,
				descendants: capture.descendants,
				qualified_name: capture.qualified_name.clone(),
				fully_qualified_name: Arc::from(format!("{}{}", name, capture.qualified_name)),
			}));

		// The parsing DFA is built from the pattern *without* the implicit root capture: the
		// parser synthesizes the root match itself, so the root's tags/registers would be pure
		// per-rule overhead here. Shape/search automata build from `regex.regex` and do keep it,
		// so a capture-less root rule can be reported under its own name.
		let dfa: Tdfa = Tdfa::for_single_rule(idx, regex.root_item(), encodings);

		Self {
			idx,
			name,
			priority,
			regex,
			rule_info,
			dfa,
		}
	}

	/// Find matching (nested) captures matching exactly (the fragments of) a fully qualified name.
	pub fn find_capture<'a>(
		&'a self,
		current_regex: &'a Regex,
		first: &str,
		rest: &[&str],
		collect: &mut Vec<(&'a RuleInfo, &'a Regex)>,
	) {
		match current_regex {
			Regex::AnyChar | Regex::Literal(..) | Regex::BracketedRanges { .. } => (),
			Regex::Capture(capture) => {
				if &*capture.name == first {
					if let Some(first) = rest.first().copied() {
						self.find_capture(&capture.item, first, &rest[1..], collect);
					} else {
						collect.push((&self[capture.id], current_regex));
					}
				}
			},
			Regex::KleeneClosure(item)
			| Regex::KleenePlus(item)
			| Regex::BoundedRepetition { item, .. }
			| Regex::Placeholder { item, .. } => {
				self.find_capture(item, first, rest, collect);
			},
			Regex::Sequence(items) | Regex::Alternation(items) => {
				for item in items.iter() {
					self.find_capture(item, first, rest, collect);
				}
			},
		}
	}
}

impl RootRule {
	pub fn has_captures(&self) -> bool {
		// Always has itself as the $0$th `RuleInfo`.
		self.rule_info.len() > 1
	}
}

impl RuleInfo {
	pub fn is_root(&self) -> bool {
		self.parent_id == self.id
	}

	pub fn is_leaf(&self) -> bool {
		self.descendants == 0
	}
}
