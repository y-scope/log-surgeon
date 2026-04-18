#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum Escaped {
	NoEscape(char),
	NeedsEscape(char),
}

// TODO: replace with `std::range::Range` when stable.
/// Rust's `std::ops::Range` is not `Copy` for... reasons.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct Range<Idx> {
	pub start: Idx,
	pub end: Idx,
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

impl<Idx> std::fmt::Display for Range<Idx>
where
	Idx: std::fmt::Display,
{
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_fmt(format_args!("{}..{}", self.start, self.end))
	}
}

impl<Idx> Range<Idx>
where
	Idx: Copy,
{
	pub fn native(&self) -> std::ops::Range<Idx> {
		std::ops::Range {
			start: self.start,
			end: self.end,
		}
	}
}
