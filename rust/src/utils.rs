#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum Escaped {
	NoEscape(char),
	NeedsEscape(char),
}

impl Escaped {
	pub fn escape_char(ch: char) -> Self {
		let ch: char = match ch {
			' ' => ' ',
			'\\' => '\\',
			'\t' => 't',
			'\r' => 'r',
			'\n' => 'n',
			_ => {
				return Self::NoEscape(ch);
			},
		};
		Self::NeedsEscape(ch)
	}

	pub fn append_to(&self, buffer: &mut String) {
		match *self {
			Self::NoEscape(ch) => {
				buffer.push(ch);
			},
			Self::NeedsEscape(ch) => {
				buffer.push('\\');
				buffer.push(ch);
			},
		}
	}
}

impl std::fmt::Display for Escaped {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NoEscape(ch) => ch.fmt(fmt),
			Self::NeedsEscape(ch) => {
				fmt.write_str("\\")?;
				ch.fmt(fmt)
			},
		}
	}
}
