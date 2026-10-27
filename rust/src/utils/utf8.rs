//! Lazy, validating iteration over the UTF-8 scalars in a byte slice.
//!
//! [`Utf8Chunks`](core::str::Utf8Chunks) is *eager*: each call to [`Iterator::next`] scans
//! from the slice start to the first invalid byte, or to the end of the slice.
//! Handing it a large tail on every decode would therefore cost O(tail) per scalar,
//! and O(n^2) over a whole input.
//! [`Utf8Chars`] instead validates only the bytes of the scalar it yields,
//! so its cost is independent of the tail length.
//!
//! This is the lazy counterpart to [`Utf8Chunks`](core::str::Utf8Chunks).
//! The standard library has the required building blocks
//! (`str::utf8_char_width`, `next_code_point`) but they are unstable,
//! and `next_code_point` additionally assumes its input is already valid.

use std::iter::FusedIterator;

/// Decode the UTF-8 scalar at the start of `bytes`, returning it with its length in bytes.
///
/// Only a single scalar (at most 4 bytes) is examined,
/// so the cost does not depend on `bytes.len()`.
///
/// # Panics
///
/// Panics if `bytes` is empty, or if its leading bytes are not a well-formed UTF-8 scalar.
#[inline(always)]
pub fn decode_scalar(bytes: &[u8]) -> (char, usize) {
	let Some(&lead): Option<&u8> = bytes.first() else {
		panic!("cannot decode a UTF-8 scalar from an empty slice");
	};

	// Fast path: ASCII is a single byte and is always valid UTF-8, so skip validation entirely.
	// This matters for hot loops over mostly-ASCII text (e.g. the lexer's static-text scan),
	// where it avoids both the `Utf8Chunks` machinery and the width table.
	if lead < 0x80 {
		return (char::from(lead), 1);
	}

	decode_scalar_non_ascii(bytes)
}

/// The non-ASCII half of [`decode_scalar`], kept out-of-line and marked cold
/// so that the ASCII fast path stays small
/// and its surrounding hot loop is laid out without this code in the way.
#[cold]
#[inline(never)]
fn decode_scalar_non_ascii(bytes: &[u8]) -> (char, usize) {
	let lead: u8 = bytes[0];

	// The length of the scalar, from the structural length prefix of the lead byte:
	//
	//     0xxxxxxx -> 1,  110xxxxx -> 2,  1110xxxx -> 3,  11110xxx -> 4
	//
	// This is an upper bound. RFC 3629 forbids some sequences that this structural
	// encoding admits -- `C0`/`C1` (overlong 2-byte), `F5`-`F7` (above U+10FFFF),
	// and `F8`-`FF` (never a lead byte) -- but every well-formed sequence has exactly
	// this length and no continuation byte is ever a lead,
	// so the window below is never shorter than the scalar it must contain.
	// `Utf8Chunks`, not this table, remains the single authority on validity.
	let width: usize = match lead {
		0xC0..=0xDF => 2,
		0xE0..=0xEF => 3,
		0xF0..=0xF7 => 4,
		_ => 1,
	};

	let mut chunks = bytes[..usize::min(width, bytes.len())].utf8_chunks();
	let chunk = chunks.next().unwrap();
	assert!(
		chunk.invalid().is_empty(),
		"invalid UTF-8 at start of input: {:?}",
		chunk.invalid()
	);
	let ch: char = chunk.valid().chars().next().unwrap();
	(ch, ch.len_utf8())
}

/// The lead byte of the UTF-8 encoding of `ch` (ASCII included).
///
/// `utf8_len` must be `ch.len_utf8()`, i.e. the width of the encoding `ch` takes in
/// [`str`]; it selects which of the (at most four) encoding bytes is the lead.
pub(crate) fn lead_byte_of(ch: u32, utf8_len: usize) -> u8 {
	match utf8_len {
		1 => u8::try_from(ch).unwrap(),
		2 => 0xC0 | u8::try_from(ch >> 6).unwrap(),
		3 => 0xE0 | u8::try_from(ch >> 12).unwrap(),
		_ => 0xF0 | u8::try_from(ch >> 18).unwrap(),
	}
}

/// A lazy iterator over the UTF-8 scalars of a byte slice, panicking on invalid UTF-8.
///
/// Unlike [`Chars`](core::str::Chars), this accepts a raw `&[u8]`.
/// Each call to [`Iterator::next`] or [`DoubleEndedIterator::next_back`]
/// validates only the scalar it yields, so neither depends on the length of the remaining input.
#[derive(Debug, Clone)]
pub struct Utf8Chars<'input> {
	remaining: &'input [u8],
}

impl<'input> Utf8Chars<'input> {
	pub fn new(input: &'input [u8]) -> Self {
		Self { remaining: input }
	}
}

impl Iterator for Utf8Chars<'_> {
	type Item = char;

	#[inline]
	fn next(&mut self) -> Option<char> {
		let (&lead, rest): (&u8, &[u8]) = self.remaining.split_first()?;
		// Fast path for ASCII, the overwhelmingly common case in log text:
		// it is a single byte and always valid,
		// so there is no need to enter [`decode_scalar`] at all.
		if lead < 0x80 {
			self.remaining = rest;
			return Some(char::from(lead));
		}
		let (ch, len): (char, usize) = decode_scalar(self.remaining);
		self.remaining = &self.remaining[len..];
		Some(ch)
	}
}

impl DoubleEndedIterator for Utf8Chars<'_> {
	#[inline]
	fn next_back(&mut self) -> Option<char> {
		if self.remaining.is_empty() {
			return None;
		}
		// Find the lead byte of the final scalar by skipping back over its continuation bytes.
		// A scalar has at most 4 bytes (3 continuation bytes), so this loop runs at most 3 times;
		// an over-long run of continuation bytes is caught by `decode_scalar` below.
		let mut start: usize = self.remaining.len() - 1;
		while start > 0 && self.remaining[start] & 0b1100_0000 == 0b1000_0000 {
			start -= 1;
		}
		let (ch, len): (char, usize) = decode_scalar(&self.remaining[start..]);
		// `decode_scalar` bounds its window to one scalar, so it cannot see trailing bytes;
		// any bytes after the decoded scalar would be an orphaned continuation byte.
		assert_eq!(
			start + len,
			self.remaining.len(),
			"invalid UTF-8 at end of input"
		);
		self.remaining = &self.remaining[..start];
		Some(ch)
	}
}

impl FusedIterator for Utf8Chars<'_> {}

#[cfg(test)]
mod test {
	use super::*;

	/// Every Unicode scalar round-trips through [`decode_scalar`] with its exact byte length.
	#[test]
	fn decode_scalar_all_scalars() {
		let mut buf: [u8; 4] = [0; 4];
		for code_point in 0..=0x10FFFF {
			let Some(ch): Option<char> = char::from_u32(code_point) else {
				continue;
			};
			let encoded: &[u8] = ch.encode_utf8(&mut buf).as_bytes();
			let (decoded, len): (char, usize) = decode_scalar(encoded);
			assert_eq!(decoded, ch, "U+{code_point:04X}");
			assert_eq!(len, ch.len_utf8(), "U+{code_point:04X}");
		}
	}

	#[test]
	fn utf8_chars_matches_str_chars() {
		for s in [
			"",
			"abc",
			"a\u{e9}b",
			"\u{4e16}\u{1f600}",
			"line1\nline2\u{e9}",
		] {
			let forward: Vec<char> = Utf8Chars::new(s.as_bytes()).collect();
			let expected: Vec<char> = s.chars().collect();
			assert_eq!(forward, expected, "s={s:?}");

			let backward: Vec<char> = Utf8Chars::new(s.as_bytes()).rev().collect();
			let expected: Vec<char> = s.chars().rev().collect();
			assert_eq!(backward, expected, "s={s:?}");
		}
	}

	/// Consuming from both ends at once must visit every scalar exactly once.
	#[test]
	fn utf8_chars_double_ended() {
		let s: &str = "a\u{e9}\u{4e16}\u{1f600}z";
		let mut chars: Utf8Chars<'_> = Utf8Chars::new(s.as_bytes());

		assert_eq!(chars.next(), Some('a'));
		assert_eq!(chars.next_back(), Some('z'));
		assert_eq!(chars.next(), Some('\u{e9}'));
		assert_eq!(chars.next_back(), Some('\u{1f600}'));
		assert_eq!(chars.next_back(), Some('\u{4e16}'));
		assert_eq!(chars.next_back(), None);
		assert_eq!(chars.next(), None);
	}

	/// A continuation byte or an invalid lead byte must panic, even when followed by plausible
	/// continuation bytes.
	#[test]
	fn decode_scalar_rejects_invalid_lead() {
		let invalid_leads: Vec<u8> = (0x80..=0xBF)
			.chain([0xC0, 0xC1])
			.chain(0xF5..=0xFF)
			.collect();
		for lead in invalid_leads {
			let bytes: [u8; 4] = [lead, 0x80, 0x80, 0x80];
			let result = std::panic::catch_unwind(|| decode_scalar(&bytes));
			assert!(result.is_err(), "lead=0x{lead:02X}");
		}
	}

	/// A truncated scalar at the end of the slice must panic, not read out of bounds.
	#[test]
	fn decode_scalar_rejects_truncated() {
		let truncated: [&[u8]; 5] = [
			&[0xC3],
			&[0xE4],
			&[0xE4, 0xB8],
			&[0xF0],
			&[0xF0, 0x9F, 0x98],
		];
		for bytes in truncated {
			let result = std::panic::catch_unwind(|| decode_scalar(bytes));
			assert!(result.is_err(), "bytes={bytes:?}");
		}
	}

	/// The reverse iterator must also reject invalid UTF-8 (here, an orphaned continuation byte).
	#[test]
	fn utf8_chars_rejects_invalid() {
		let result = std::panic::catch_unwind(|| Utf8Chars::new(b"a\x80").next_back());
		assert!(result.is_err());

		let result = std::panic::catch_unwind(|| Utf8Chars::new(b"a\xe4").next_back());
		assert!(result.is_err());
	}
}
