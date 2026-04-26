use crate::log_event::Capture;
use crate::schema::Schema;
use std::num::NonZero;

/// A `LogType` is a "template string" for a [`LogEvent`](crate::log_event::LogEvent).
/// The string representation of a `LogType` (e.g. given by [`LogType::as_str`])
/// consists of:
///
/// - `'%'` characters escaped by doubling them,
/// - capture placeholders surrounded by a single `'%'` on each side;
///   a capture `bar` with capture id `2` in a variable `foo` with rule id `1`
///   shows up at `%1.2:foo.bar%` in the string representation.
///
#[derive(Clone, Eq)]
pub struct LogType {
	cached_representation: String,
}

impl std::fmt::Debug for LogType {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.debug_tuple("LogType").field(&self.cached_representation).finish()
	}
}

impl PartialEq for LogType {
	fn eq(&self, other: &Self) -> bool {
		self.cached_representation == other.cached_representation
	}
}

impl std::fmt::Display for LogType {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(&self.cached_representation)
	}
}

impl LogType {
	pub const BLANK: Self = Self {
		cached_representation: String::new(),
	};

	pub fn new<'a>(schema: &Schema, log_message: &str, captures: impl Iterator<Item = &'a Capture>) -> Self {
		let cached_representation: String = to_string(schema, log_message, captures);
		Self { cached_representation }
	}

	pub fn as_str(&self) -> &str {
		&self.cached_representation
	}
}

fn to_string<'a>(schema: &Schema, log_message: &str, captures: impl Iterator<Item = &'a Capture>) -> String {
	use std::fmt::Write;

	let mut buf: String = String::new();
	let mut last_pos: usize = 0;
	for capture in captures {
		let pos: usize = capture.range.start;
		for s in escape::<'%'>(&log_message[last_pos..pos]) {
			buf.push_str(s);
		}
		let (variable_name, capture_name): (&str, &str) = schema.names(capture);
		write!(
			&mut buf,
			"%{}.{}:{}.{}%",
			capture.rule_idx,
			capture.capture_id.map_or(0, NonZero::get),
			variable_name,
			capture_name,
		)
		.unwrap();
		last_pos = capture.range.end;
	}
	for s in escape::<'%'>(&log_message[last_pos..]) {
		buf.push_str(s);
	}
	buf
}

/// Escapes static text by duplicating each occurence of `CHAR`;
/// returns an iterator over escaped substrings;
/// concatenate the substrings for the final result.
fn escape<'a, const CHAR: char>(mut remaining: &'a str) -> impl Iterator<Item = &'a str> {
	std::iter::from_fn(move || {
		if remaining.is_empty() {
			return None;
		}
		Some(match remaining.find(CHAR) {
			Some(0) => {
				remaining = &remaining[CHAR.len_utf8()..];
				"%%"
			},
			Some(i) => {
				let (before, after): (&str, &str) = remaining.split_at(i);
				remaining = after;
				before
			},
			None => std::mem::replace(&mut remaining, ""),
		})
	})
}

#[cfg(test)]
mod test {
	// use super::*;

	#[test]
	fn basic() {
		// let t: LogType = LogType::new(
		// 	"hello % world",
		// 	&[(3, "int".to_owned()), (6, "float".to_owned())],
		// );
		// assert_eq!(t.to_string(), "hel%int%lo %float%%% world");
	}
}
