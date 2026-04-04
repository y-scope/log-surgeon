use std::ffi::c_char;
use std::num::NonZero;

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

	/// `usize::MAX` if none.
	pub parent_index: usize,

	/// Offset of the capture in the log message.
	pub range: CaptureRange,

	pub is_leaf: bool,

	/// DANGEROUS fields for FFI.
	/// But it's not dangerous if you don't look at it.
	pub ffi_pointers: CaptureFfiPointers,
}

/// Only reason we don't use [`std::ops::Range`] is because it isn't `Copy`
/// (by questionable design reasons).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct CaptureRange {
	pub start: usize,
	pub end: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct CaptureFfiPointers {
	pub parent: *const Capture,
	pub lexeme: CapturePointerLength<c_char>,
	pub variable_name: CapturePointerLength<c_char>,
	pub capture_name: CapturePointerLength<c_char>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct CapturePointerLength<T> {
	pointer: *const T,
	length: usize,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl Send for Capture {}
unsafe impl Sync for Capture {}

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

impl CaptureFfiPointers {
	pub const NULL: Self = Self {
		parent: std::ptr::null(),
		lexeme: CapturePointerLength::NULL,
		variable_name: CapturePointerLength::NULL,
		capture_name: CapturePointerLength::NULL,
	};
}

impl<T> CapturePointerLength<T> {
	pub const NULL: Self = Self {
		pointer: std::ptr::null(),
		length: 0,
	};
}

impl CapturePointerLength<c_char> {
	pub fn from_str(s: &str) -> Self {
		Self {
			pointer: s.as_ptr().cast::<c_char>(),
			length: s.len(),
		}
	}
}
