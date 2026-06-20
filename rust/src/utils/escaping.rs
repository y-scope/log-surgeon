use std::str::Chars;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Escaped {
	ch: char,
	escape_space: bool,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvalidEscape {
	/// Reached EOF after initial backslash (empty input).
	Eof,
	/// Malformed Unicode code point (as hexadecimal digit pairs).
	Malformed,
	/// Unknown escape character.
	Unknown(char),
	/// Invalid Unicode code point.
	BadCodePoint(u32),
}

impl Escaped {
	pub fn escape(ch: char) -> Self {
		Self { ch, escape_space: true }
	}

	pub fn escape_space(mut self, b: bool) -> Self {
		self.escape_space = b;
		self
	}

	/// Parses the following "common" escape sequences, after the backslash
	/// (switches on the first character of `input`):
	///
	/// - ` ` for a literal space (for usages that need to avoid ambiguity).
	/// - a (second) backslash for a literal backslash.
	/// - `'` for a literal single quote.
	/// - `"'` for a literal double quote.
	/// - `t`, `r`, `n`: tab, carriage return, and newline respectively.
	/// - `u{xx}`, `u{xxyy}`, `u{xxyyzz}` for a Unicode code point in hexadecimal representation.
	///   - Hex digits may be upper or lower case, and must come in pairs.
	pub fn unescape(input: &str) -> Result<(&str, char), InvalidEscape> {
		let mut chars: Chars<'_> = input.chars();

		let Some(ch): Option<char> = chars.next() else {
			return Err(InvalidEscape::Eof);
		};

		let ch: char = match ch {
			' ' | '\\' | '\'' | '"' => ch,
			't' => '\t',
			'r' => '\r',
			'n' => '\n',
			'u' => {
				const MAX_BITS_PER_CODE_POINT: u32 = (char::MAX as u32).ilog2() + 1;
				assert_eq!(MAX_BITS_PER_CODE_POINT, 21);
				const MAX_BYTES_PER_CODE_POINT: u32 = MAX_BITS_PER_CODE_POINT.div_ceil(u8::BITS);
				assert_eq!(MAX_BYTES_PER_CODE_POINT, 3);

				let Some(ch): Option<char> = chars.next() else {
					return Err(InvalidEscape::Malformed);
				};
				if ch != '{' {
					return Err(InvalidEscape::Malformed);
				}

				let Some(mut code_point): Option<u32> = parse_hex_digit_pair(&mut chars) else {
					return Err(InvalidEscape::Malformed);
				};

				for _ in 1..=MAX_BYTES_PER_CODE_POINT {
					let backup: Chars<'_> = chars.clone();
					if let Some(byte) = parse_hex_digit_pair(&mut chars) {
						code_point = (code_point << u8::BITS) | byte;
					} else {
						chars = backup;
						break;
					}
				}

				let Some(ch): Option<char> = chars.next() else {
					return Err(InvalidEscape::Malformed);
				};
				if ch != '}' {
					return Err(InvalidEscape::Malformed);
				}

				return if let Some(ch) = char::from_u32(code_point) {
					Ok((chars.as_str(), ch))
				} else {
					Err(InvalidEscape::BadCodePoint(code_point))
				};
			},
			_ => {
				return Err(InvalidEscape::Unknown(ch));
			},
		};

		Ok((chars.as_str(), ch))
	}
}

impl std::fmt::Display for Escaped {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let ch: char = self.ch;
		if self.escape_space && (ch == ' ') {
			// We escape space because it can easily be "lost" at the start or end of a pattern,
			// e.g. a text editor may trim trailing whitespace when saving a parsing spec file.
			// Tabs, carriage returns, newlines, and other (unicode) whitespace will be escaped below.
			fmt.write_str("\\ ")
		} else if (ch == '\'') || (ch == '"') {
			// [`char::escape_default`] also escapes quotes, which aren't relevant to us;
			// we pass through quotes as is.
			ch.fmt(fmt)
		} else {
			// `\t`, `\r`, `\n`, `\\`, non-printable ASCII, non-ASCII unicode characters.
			ch.escape_default().fmt(fmt)
		}
	}
}

fn parse_hex_digit_pair(chars: &mut Chars<'_>) -> Option<u32> {
	if let Some(upper) = chars.next()
		&& let Some(lower) = chars.next()
	{
		match (upper.to_digit(16), lower.to_digit(16)) {
			(Some(upper), Some(lower)) => {
				return Some((upper << 4) + lower);
			},
			_ => (),
		}
	}

	None
}
