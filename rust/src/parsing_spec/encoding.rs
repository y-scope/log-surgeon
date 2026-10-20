use std::num::NonZero;

use crate::nfa::Tnfa;
use crate::regex::Regex;

/// Index in the parsing specification, offset by/starting at 1.
/// `Option<EncodingIdx>` is ABI equivalent to `u16` (for FFI);
/// `None`/`0` represents no encoding.
#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct EncodingIdx(NonZero<u16>);

#[derive(Debug, Clone)]
pub struct Encoding {
	pub idx: EncodingIdx,
	pub name: String,
	pub regex: Regex,
	/// Cached NFA for intersecting with.
	pub nfa: Tnfa,
}

impl Eq for Encoding {}

impl Ord for Encoding {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(&self.name, &self.regex).cmp(&(&other.name, &other.regex))
	}
}

impl PartialEq for Encoding {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other).is_eq()
	}
}

impl PartialOrd for Encoding {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl From<EncodingIdx> for NonZero<u16> {
	fn from(encoding_idx: EncodingIdx) -> Self {
		encoding_idx.0
	}
}

impl From<EncodingIdx> for u16 {
	fn from(encoding_idx: EncodingIdx) -> Self {
		encoding_idx.0.get()
	}
}

impl From<NonZero<u16>> for EncodingIdx {
	fn from(encoding_idx: NonZero<u16>) -> Self {
		Self(encoding_idx)
	}
}
