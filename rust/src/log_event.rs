use std::ffi::c_char;
use std::num::NonZero;

use crate::ffi::CArray;
use crate::ffi::CRange;
use crate::ffi::CUtf8;
use crate::ffi::UncheckedCArray;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::RuleIdx;

#[derive(Clone, Eq, PartialEq)]
#[repr(C)]
pub struct LogEvent<'parser> {
	/// Strictly speaking, this field is redundant;
	/// however, the spec is needed to get info about the rules,
	/// and is included here for convenience.
	///
	/// In particular, since [`Parser::next_event`](crate::parser::Parser::next_event)
	/// returns a `LogEvent` that `mut` (exclusively) borrows from the parser
	/// (see [`Parser`](crate::parser::Parser) for more details),
	/// the caller can't access the parser's spec and the event at the same time.
	/// So, `Parser::next_event` passes a reference to the spec through the returned `LogEvent`.
	pub spec: &'parser ParsingSpec,
	pub message: CUtf8<'parser>,
	/// Matches are sorted:
	///
	/// 1. left-to-right (lexicographically with respect to the input),
	/// 2. top-down; parent rules first (lexicographically with respect to the regex pattern).
	///
	/// In particular, root rule matches always come before their sub-rule matches.
	pub all_matches: CArray<'parser, Match>,
	pub root_indices: CArray<'parser, usize>,
	pub leaf_indices: CArray<'parser, usize>,
}

/// `Match`es are exposed to FFI, so they need to be `#[repr(C)]`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct Match {
	pub rule_idx: RuleIdx,
	/// Capture ID, local to the containing rule/variable/regex pattern;
	/// `None`/`0` for a root rule,
	/// See [`RuleInfo`](crate::parsing_spec::RuleInfo).
	pub sub_rule_id: Option<NonZero<u16>>,

	/// Index of the parent in the full list of matches (including variables/root rules).
	/// For a variable, the parent index equals its own index.
	pub parent_index: usize,

	/// Relative to the start of the log message.
	pub range: CRange<usize>,

	pub is_leaf: bool,

	/// This should be `Option<EncodingIdx>`,
	/// but cbindgen doesn't translate it to `uint16_t` properly.
	///
	/// See:
	/// - https://github.com/mozilla/cbindgen/issues/690
	/// - https://github.com/mozilla/cbindgen/issues/326
	/// - https://github.com/mozilla/cbindgen/pull/1029
	pub encoding_idx: Option<NonZero<u16>>,

	/// DANGEROUS fields exposed for FFI.
	/// But it's not dangerous if you don't look at it (in Rust).
	/// Safe Rust code should refer to the fields above and the corresponding [`LogEvent`]
	/// as necessary.
	///
	/// Note: [`LogEvent`] can borrow from [`crate::parser::Parser`] since it's an "external" value,
	/// but the [`Match`]es of a `LogEvent` live in a `Vec` inside `Parser`,
	/// so they can't safely reference the `Parser` (be self-referential).
	pub ffi_pointers: MatchFfiPointers,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct MatchFfiPointers {
	pub parent: *const Match,
	pub lexeme: UncheckedCArray<c_char>,
	/// Name of _this_ (root or sub-) rule.
	pub rule_name: UncheckedCArray<c_char>,
	/// Fully-qualified name, including the root rule and all nested regex capture expressions.
	pub fully_qualified_name: UncheckedCArray<c_char>,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl Send for MatchFfiPointers {}
unsafe impl Sync for MatchFfiPointers {}

impl std::fmt::Debug for LogEvent<'_> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut last_pos: usize = 0;
		for mat in self.all_matches.iter() {
			if !mat.is_leaf {
				continue;
			}
			for ch in self.message[last_pos..mat.range.start].chars() {
				if matches!(ch, '(' | ')') {
					fmt.write_str("\\")?;
				}
				std::fmt::Display::fmt(&ch.escape_default(), fmt)?;
			}
			fmt.write_str("(?<")?;
			fmt.write_str(&self.spec[mat.rule_idx][mat.sub_rule_id].fully_qualified_name)?;
			fmt.write_str(">")?;
			for ch in self.message[mat.range.start..mat.range.end].chars() {
				if matches!(ch, '(' | ')') {
					fmt.write_str("\\")?;
				}
				std::fmt::Display::fmt(&ch.escape_default(), fmt)?;
			}
			fmt.write_str(")")?;
			last_pos = mat.range.end;
		}
		for ch in self.message[last_pos..].chars() {
			if matches!(ch, '(' | ')') {
				fmt.write_str("\\")?;
			}
			std::fmt::Display::fmt(&ch.escape_default(), fmt)?;
		}
		Ok(())
	}
}

impl<'parser> LogEvent<'parser> {
	/// Blank `LogEvent`; default value required for C FFI.
	pub const BLANK: Self = Self {
		spec: &crate::parsing_spec::BLANK,
		message: CUtf8::EMPTY,
		all_matches: CArray::dangling(),
		leaf_indices: CArray::dangling(),
		root_indices: CArray::dangling(),
	};

	/// See the ordering for [`LogEvent::all_matches`].
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
			"Match(rule: {}, id: {}, range: {}..{})",
			self.rule_idx,
			self.sub_rule_id.map_or(0, NonZero::get),
			self.range.start,
			self.range.end,
		))
	}
}

impl Match {
	pub fn is_root(&self) -> bool {
		self.sub_rule_id.is_none()
	}

	/// Debugging function to stringify a `Match`;
	/// unsafe because the `Match` object may outlive the parser whose input
	/// this object implicitly refers to.
	pub unsafe fn show(&self) -> String {
		format!(
			"Match(rule: {}, id: {}, {:?})",
			self.rule_idx,
			self.sub_rule_id.map_or(0, NonZero::get),
			unsafe { self.ffi_pointers.lexeme.as_str() },
		)
	}
}

impl MatchFfiPointers {
	pub const NULL: Self = Self {
		parent: std::ptr::null(),
		lexeme: UncheckedCArray::EMPTY,
		rule_name: UncheckedCArray::EMPTY,
		fully_qualified_name: UncheckedCArray::EMPTY,
	};
}
