//! SIMD scans over byte sets,
//! for the lexer's delimiter and unanchored-first-byte prefilters
//! (see `docs/leftmost-longest-lexing.md`, design B).
//!
//! [`ByteSet`] precomputes whatever representation fits the set:
//! - `Eq` compares each byte in the set individually (broadcast + `simd_eq`).
//! - `Nibbles` uses SSSE3-style nibble tables.
//!   A byte `b` is in the set iff `lo[b & 0xF]` and `hi[b >> 4]` share a bit.
//!   Bits encode *classes* of high nibbles;
//!   when more than 8 distinct row masks occur, rows are merged,
//!   which makes membership an over-approximation.
//!   [`u8x16::shuffle_zeroing`] guarantees zero lookups for out-of-range indices
//!   even when AVX2's `vpshufb` is used, so bytes >= 0x80 (high bit) never match
//!   from a wrapped index.

#[cfg(test)]
mod test;

use wide::u8x16;

/// The largest number of bytes compared individually in [`ByteSet::Eq`].
const MAX_EQ_VALUES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSet {
	/// Matches nothing; a `const`-constructible default for statics.
	Empty,
	/// Compare with each byte individually; broadcast vectors are precomputed.
	Eq { values: Vec<u8x16> },
	/// Two 16-entry nibble tables; low indexes `b & 0xF`, high indexes `b >> 4`.
	Nibbles { low: u8x16, high: u8x16 },
}

impl ByteSet {
	/// Members of a set need not be unique or sorted.
	pub fn from_bytes(values: impl IntoIterator<Item = u8>) -> Self {
		let values: Vec<u8> = Vec::from_iter(values);
		if values.len() <= MAX_EQ_VALUES {
			return Self::Eq {
				values: values.iter().map(|&v| u8x16::splat(v)).collect(),
			};
		}

		// Column masks: low nibble -> bitmask of member high nibbles.
		// For any row order `row_bit` (1, 2, ..., 128 or truncated), tables
		//   hi_table[h] = row_bit[h]
		//   lo_table[l] = OR_{h in column l} row_bit[h]
		// give member(h, l) iff `lo_table[l] & hi_table[h] != 0`, *provided rows are distinct*:
		//   member: shared bit by construction; non-member: `h1 != h2` have different bits.
		// Rows beyond 8 share the low bits (over-approximation).
		let mut col_mask: [u16; 16] = [0; 16];
		for &byte in values.iter() {
			col_mask[usize::from(byte & 0xF)] |= 1u16 << (byte >> 4);
		}
		let mut row_bit_of: [u8; 16] = [0; 16];
		let mut next_bit: u8 = 1;
		for (hi, bit) in row_bit_of.iter_mut().enumerate() {
			if col_mask.iter().any(|mask| mask >> hi & 1 == 1) {
				*bit = next_bit;
				next_bit = if next_bit == 0x80 { 1 } else { next_bit << 1 };
			}
		}
		let mut low: [u8; 16] = [0; 16];
		for (lo, entry) in low.iter_mut().enumerate() {
			let mut bits: u8 = 0;
			let mut column: u16 = col_mask[lo];
			while column != 0 {
				let hi: u32 = column.trailing_zeros();
				bits |= row_bit_of[hi as usize];
				column &= column - 1;
			}
			*entry = bits;
		}
		let high: [u8; 16] = row_bit_of;
		Self::Nibbles {
			low: u8x16::from(low),
			high: u8x16::from(high),
		}
	}

	/// Whether `byte` matches the set (possibly a superset for `Nibbles`, see above).
	/// Callers use this only when they already know the set is exact
	/// or when a superset is acceptable as a filter.
	pub fn contains(&self, byte: u8) -> bool {
		match self {
			Self::Empty => false,
			Self::Eq { values } => values.iter().any(|v| v.to_array()[0] == byte),
			Self::Nibbles { low, high } => {
				let lo: u8 = low.to_array()[usize::from(byte & 0xF)];
				let hi: u8 = high.to_array()[usize::from(byte >> 4)];
				(lo & hi) != 0
			},
		}
	}

	/// Whether the representation matches the construction-time set exactly
	/// (no merged-row over-approximation for `Nibbles`).
	pub fn is_exact(&self) -> bool {
		match self {
			Self::Empty | Self::Eq { .. } => true,
			Self::Nibbles { low, high } => {
				let lows: [u8; 16] = low.to_array();
				let highs: [u8; 16] = high.to_array();
				// Over-approximation happens iff two high-nibble rows share a bit.
				// So: every nonzero high entry must be a distinct power of two
				// (unique bits never co-occur falsely).
				let used: u8 = highs
					.iter()
					.filter(|&&h| h != 0)
					.fold(0u8, |acc, &h| acc | h);
				let distinct: u32 = highs
					.iter()
					.filter(|&&h| h != 0)
					.map(|&h| h.count_ones())
					.sum();
				used.count_ones() == distinct && lows.iter().all(|&l| used | l == used)
			},
		}
	}

	/// Offset of the first byte in the set (or its superset, for `Nibbles`), or `None`.
	/// Callers relying on exactness should check [`ByteSet::is_exact`],
	/// or re-test hits with `contains` on the original set.
	pub fn find_in(&self, bytes: &[u8]) -> Option<usize> {
		let mut chunks = bytes.chunks_exact(16);
		let mut index: usize = 0;
		for chunk in chunks.by_ref() {
			let block: u8x16 = u8x16::from(<[u8; 16]>::try_from(chunk).unwrap());
			let bits: u32 = match self {
				Self::Empty => 0,
				Self::Eq { values } => {
					let mut m: u32 = 0;
					for value in values.iter() {
						m |= value.simd_eq(block).to_bitmask();
					}
					m
				},
				Self::Nibbles { low, high } => {
					let lo: u8x16 = low.shuffle_zeroing(block & u8x16::splat(0x0F));
					let hi: u8x16 = high.shuffle_zeroing(block.unbounded_shr_scalar(4));
					(lo & hi).simd_ne(u8x16::ZERO).to_bitmask()
				},
			};
			if bits != 0 {
				return Some(index + (bits.trailing_zeros() as usize));
			}
			index += 16;
		}
		let tail: &[u8] = chunks.remainder();
		tail.iter()
			.position(|byte| self.contains(*byte))
			.map(|pos| index + pos)
	}
}
