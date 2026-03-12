use std::num::NonZero;

use crate::log_type::LogType;

/// A `LogEvent` has a template [`LogType`](crate::log_type::LogType).
/// and a sequence of [`Variable`]s to interpolate.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LogEvent<'parser> {
	pub log_type: LogType,
	pub variables: Vec<Variable<'parser>>,
	/// Whether this `LogEvent` was delimited by a ``"header"` variable (otherwise, just by a newline);
	/// if so, the header is necessarily the first variable.
	pub have_header: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Variable<'parser> {
	/// Rule ID/index from the schema.
	pub rule: usize,
	/// Rule name from the schema.
	pub name: &'parser str,
	/// Matched text.
	pub lexeme: &'parser str,
	/// Flattened list of captures; sorted top-down (nesting), left-to-right (input order).
	pub captures: Vec<Capture<'parser>>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Capture<'a> {
	/// Capture name from the regex pattern.
	pub name: &'a str,
	/// Matched text.
	pub lexeme: &'a str,
	/// Capture ID, statically assigned left-to-right based on the regex pattern;
	/// e.g. the pattern `(?<start>[a-z]+(?<rest>\.[a-z]+)*)|(?<start>[0-9]+)` has three capture IDs.
	/// When this variable/pattern is actually matched,
	/// there may be multiple instances of capture ID 2 (corresponding to `"rest"`).
	/// The capture ID also differentiates between different capture groups given the same name,
	/// e.g. the two instances of `"start"` in the pattern.
	pub id: NonZero<u32>,
	/// Parent capture ID; `None` if this is a top-level capture.
	pub parent_id: Option<NonZero<u32>>,
}

impl<'parser> LogEvent<'parser> {
	/// Blank `LogEvent`; default value required for C FFI.
	pub fn blank() -> Self {
		Self {
			log_type: LogType::new(String::new(), Vec::new()),
			variables: Vec::new(),
			have_header: false,
		}
	}
}
