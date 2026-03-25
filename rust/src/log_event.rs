use std::num::NonZero;

use crate::log_type::LogType;
use crate::schema::Rule;
use crate::schema::RuleIdx;
use crate::schema::Schema;

/// A `LogEvent` has a template [`LogType`](crate::log_type::LogType).
/// and a sequence of [`Capture`]s to interpolate.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LogEvent<'parser> {
	pub log_type: LogType,
	pub message: &'parser str,
	pub leaf_captures: &'parser [Capture],
	pub all_captures: &'parser [Capture],
	pub variables: &'parser [Capture],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Capture {
	pub rule_idx: RuleIdx,
	/// Capture ID, statically assigned left-to-right based on the regex pattern;
	/// e.g. the pattern `(?<start>[a-z]+(?<rest>\.[a-z]+)*)|(?<start>[0-9]+)` has three capture IDs.
	/// When this variable/pattern is actually matched,
	/// there may be multiple instances of capture ID 2 (corresponding to `"rest"`).
	/// The capture ID also differentiates between different capture groups given the same name,
	/// e.g. the two instances of `"start"` in the pattern.
	pub capture_id: Option<NonZero<u32>>,
	/// Parent capture ID; `None` if this is a top-level capture.
	pub parent_id: Option<NonZero<u32>>,
	/// Offset of the capture in the log message.
	pub range: (usize, usize),
}

impl<'parser> LogEvent<'parser> {
	/// Blank `LogEvent`; default value required for C FFI.
	pub const BLANK: Self = Self {
		log_type: LogType::BLANK,
		message: "",
		leaf_captures: &[],
		all_captures: &[],
		variables: &[],
	};
}

impl Capture {
	pub fn names<'schema>(&self, schema: &'schema Schema) -> (&'schema str, &'schema str) {
		let rule: &Rule = &schema[self.rule_idx];
		let capture_id: usize = self.capture_id.map_or(0, NonZero::get) as usize;
		(&rule.name, &rule.capture_info[capture_id].name)
	}
}
