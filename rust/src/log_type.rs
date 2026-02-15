#[derive(Debug, Clone)]
pub struct LogType {
	static_text: String,
	variable_indices: Vec<(usize, String)>,
	cached: String,
}

struct EscapePercent<'a> {
	remaining: &'a str,
}

impl std::fmt::Display for LogType {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(&self.cached)
		/*
		let mut last_pos: usize = 0;
		for (i, (pos, variable_type)) in self.variable_indices.iter().enumerate() {
			let pos: usize = *pos;
			for s in escape(&self.static_text[last_pos..pos]) {
				fmt.write_str(s)?;
			}
			fmt.write_fmt(format_args!("%{i}:{variable_type}%"))?;
			last_pos = pos;
		}
		for s in escape(&self.static_text[last_pos..]) {
			fmt.write_str(s)?;
		}
		Ok(())
		*/
	}
}

impl LogType {
	pub fn new(static_text: String, variable_indices: Vec<(usize, String)>) -> Self {
		let cached: String = to_string(&static_text, &variable_indices);
		Self {
			static_text,
			variable_indices,
			cached,
		}
	}

	pub fn as_str(&self) -> &str {
		&self.cached
	}
}

fn to_string(static_text: &str, variable_indices: &[(usize, String)]) -> String {
	let mut buf: String = String::new();
	let mut last_pos: usize = 0;
	for (i, (pos, variable_type)) in variable_indices.iter().enumerate() {
		let pos: usize = *pos;
		for s in escape(&static_text[last_pos..pos]) {
			buf.push_str(s);
		}
		buf.push_str(&format!("%{i}:{variable_type}%"));
		last_pos = pos;
	}
	for s in escape(&static_text[last_pos..]) {
		buf.push_str(s);
	}
	buf
}

impl<'a> Iterator for EscapePercent<'a> {
	type Item = &'a str;

	fn next(&mut self) -> Option<Self::Item> {
		if self.remaining.is_empty() {
			return None;
		}
		Some(match self.remaining.find('%') {
			Some(0) => {
				self.remaining = &self.remaining["%".len()..];
				"%%"
			},
			Some(i) => {
				let (before, after): (&str, &str) = self.remaining.split_at(i);
				self.remaining = after;
				before
			},
			None => std::mem::replace(&mut self.remaining, ""),
		})
	}
}

fn escape(remaining: &str) -> EscapePercent<'_> {
	EscapePercent { remaining }
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
		assert_eq!(t.to_string(), "hel%0:int%lo %1:float%%% world");
	}
}
