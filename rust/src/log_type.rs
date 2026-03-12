/// A `LogType` is a "template string" for a [`LogEvent`](crate::log_event::LogEvent).
/// The string representation of a `LogType` (e.g. given by [`LogType::as_str`])
/// consists of:
///
/// - `'%'` characters escaped by doubling them,
/// - variable placeholders surrounded by a single `'%'` on each side;
///   a variable named `foo` shows up as `%foo%` in the string representation.
///
#[derive(Clone, Eq)]
pub struct LogType {
	// static_text: String,
	// variable_indices: Vec<(usize, String)>,
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
	pub fn new(static_text: String, variable_indices: Vec<(usize, String)>) -> Self {
		let cached_representation: String = to_string(&static_text, &variable_indices);
		Self {
			// static_text,
			// variable_indices,
			cached_representation,
		}
	}

	pub fn as_str(&self) -> &str {
		&self.cached_representation
	}
}

fn to_string(static_text: &str, variable_indices: &[(usize, String)]) -> String {
	let mut buf: String = String::new();
	let mut last_pos: usize = 0;
	for (pos, variable_type) in variable_indices.iter() {
		let pos: usize = *pos;
		for s in escape::<'%'>(&static_text[last_pos..pos]) {
			buf.push_str(s);
		}
		buf.push_str("%");
		buf.push_str(variable_type);
		buf.push_str("%");
		last_pos = pos;
	}
	for s in escape::<'%'>(&static_text[last_pos..]) {
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
	use super::*;

	#[test]
	fn basic() {
		let t: LogType = LogType::new(
			"hello % world".to_owned(),
			vec![(3, "int".to_owned()), (6, "float".to_owned())],
		);
		assert_eq!(t.to_string(), "hel%int%lo %float%%% world");
	}
}
