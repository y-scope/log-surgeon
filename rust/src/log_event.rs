use std::ffi::c_char;
use std::num::NonZero;

use crate::ffi::UncheckedCArray;
use crate::parsing_spec::RuleIdx;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LogEvent<'parser> {
	pub message: &'parser str,
	pub all_matches: &'parser [Match],
	pub leaf_indices: &'parser [usize],
	pub variable_indices: &'parser [usize],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct Match {
	pub rule_idx: RuleIdx,
	/// SubRule ID, local to the containing rule/variable/regex pattern;
	/// `None`/`0` for a root rule,
	/// See [`SubRule`](crate::parsing_spec::SubRule).
	pub sub_rule_id: Option<NonZero<u16>>,
	/// Parent SubRule ID, if any;
	/// `None` for both a root rule and a top-level capture in a regex pattern.
	pub parent_id: Option<NonZero<u16>>,

	/// Index of the parent in the full list of matches (including variables/root rules).
	/// For a variable, the parent index equals its own index.
	pub parent_index: usize,

	/// Relative to the start of the log message.
	pub range: Range<usize>,

	pub is_leaf: bool,

	pub encoding_idx: Option<NonZero<u16>>,

	/// DANGEROUS fields for FFI.
	/// But it's not dangerous if you don't look at it.
	pub ffi_pointers: MatchFfiPointers,
}

/// Need this to be `#[repr(C)]`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct Range<Idx> {
	pub start: Idx,
	pub end: Idx,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct MatchFfiPointers {
	pub parent: *const Match,
	pub lexeme: UncheckedCArray<c_char>,
	pub root_rule_name: UncheckedCArray<c_char>,
	/// Name of _this_ (root or sub-) rule.
	pub rule_name: UncheckedCArray<c_char>,
	/// Fully-qualified name, including the root rule and all nested regex capture expressions.
	pub fully_qualified_name: UncheckedCArray<c_char>,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl Send for MatchFfiPointers {}
unsafe impl Sync for MatchFfiPointers {}

impl<'parser> LogEvent<'parser> {
	/// Blank `LogEvent`; default value required for C FFI.
	pub const BLANK: Self = Self {
		message: "",
		all_matches: &[],
		leaf_indices: &[],
		variable_indices: &[],
	};

	pub fn check_invariants(&self) {
		assert!(self.all_matches.is_sorted_by(|lhs, rhs| {
			lhs.range
				.start
				.cmp(&rhs.range.start)
				.then(lhs.range.end.cmp(&rhs.range.end).reverse())
				.then(lhs.sub_rule_id.cmp(&rhs.sub_rule_id))
				.is_lt()
		}));
	}
}

impl std::fmt::Display for Match {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!(
			"Match(rule: {}, id: {}, parent: {}, range: {}..{})",
			self.rule_idx,
			self.sub_rule_id.map_or(0, NonZero::get),
			self.parent_id.map_or(0, NonZero::get),
			self.range.start,
			self.range.end,
		))
	}
}

impl Match {
	pub unsafe fn show(&self) -> String {
		format!(
			"Match(rule: {}, id: {}, parent: {}, {:?})",
			self.rule_idx,
			self.sub_rule_id.map_or(0, NonZero::get),
			self.parent_id.map_or(0, NonZero::get),
			unsafe { self.ffi_pointers.lexeme.as_str() },
		)
	}
}

impl MatchFfiPointers {
	pub const NULL: Self = Self {
		parent: std::ptr::null(),
		lexeme: UncheckedCArray::NULL,
		root_rule_name: UncheckedCArray::NULL,
		rule_name: UncheckedCArray::NULL,
		fully_qualified_name: UncheckedCArray::NULL,
	};
}
