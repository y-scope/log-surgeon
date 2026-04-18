use crate::utils::Range;
use std::ffi::c_char;
use std::num::NonZero;

use crate::ffi::UncheckedCArray;
use crate::log_type::LogType;
use crate::schema::RuleIdx;

/// A `LogEvent` has a template [`LogType`](crate::log_type::LogType).
/// and a sequence of [`Capture`]s to interpolate.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LogEvent<'parser> {
	pub log_type: LogType,
	pub message: &'parser str,
	pub all_captures: &'parser [Capture],
	pub leaf_indices: &'parser [usize],
	pub variable_indices: &'parser [usize],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct Capture {
	pub rule_idx: RuleIdx,
	/// Capture ID, statically assigned left-to-right based on the regex pattern;
	/// e.g. the pattern `(?<start>[a-z]+(?<rest>\.[a-z]+)*)|(?<start>[0-9]+)` has three capture IDs.
	/// When this variable/pattern is actually matched,
	/// there may be multiple instances of capture ID 2 (corresponding to `"rest"`).
	/// The capture ID also differentiates between different capture groups given the same name,
	/// e.g. the two instances of `"start"` in the pattern.
	pub capture_id: Option<NonZero<u32>>,
	pub parent_id: Option<NonZero<u32>>,

	/// Index of the parent in the full list of captures (including variables).
	/// For a variable, the parent index equals its own index.
	pub parent_index: usize,

	/// Relative to the start of the log message.
	pub range: Range<usize>,

	pub is_leaf: bool,

	/// DANGEROUS fields for FFI.
	/// But it's not dangerous if you don't look at it.
	pub ffi_pointers: CaptureFfiPointers,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct CaptureFfiPointers {
	pub parent: *const Capture,
	pub lexeme: UncheckedCArray<c_char>,
	pub variable_name: UncheckedCArray<c_char>,
	pub capture_name: UncheckedCArray<c_char>,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl Send for CaptureFfiPointers {}
unsafe impl Sync for CaptureFfiPointers {}

impl<'parser> LogEvent<'parser> {
	/// Blank `LogEvent`; default value required for C FFI.
	pub const BLANK: Self = Self {
		log_type: LogType::BLANK,
		message: "",
		all_captures: &[],
		leaf_indices: &[],
		variable_indices: &[],
	};

	pub fn check_invariants(&self) {
		assert!(self.all_captures.is_sorted_by(|lhs, rhs| {
			lhs.range
				.start
				.cmp(&rhs.range.start)
				.then(lhs.range.end.cmp(&rhs.range.end).reverse())
				.then(lhs.capture_id.cmp(&rhs.capture_id))
				.is_lt()
		}));
	}
}

impl std::fmt::Display for Capture {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!(
			"Capture(rule: {}, id: {}, parent: {}, range: {})",
			self.rule_idx.index,
			self.capture_id.map_or(0, NonZero::get),
			self.parent_id.map_or(0, NonZero::get),
			self.range
		))
	}
}

impl Capture {
	pub unsafe fn show(&self) -> String {
		format!(
			"Capture(rule: {}, id: {}, parent: {}, {:?})",
			self.rule_idx.index,
			self.capture_id.map_or(0, NonZero::get),
			self.parent_id.map_or(0, NonZero::get),
			unsafe { self.ffi_pointers.lexeme.as_str() },
		)
	}
}

impl CaptureFfiPointers {
	pub const NULL: Self = Self {
		parent: std::ptr::null(),
		lexeme: UncheckedCArray::NULL,
		variable_name: UncheckedCArray::NULL,
		capture_name: UncheckedCArray::NULL,
	};
}
