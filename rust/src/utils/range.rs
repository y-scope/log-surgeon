// TODO: replace with `std::range::Range` when stable.
/// Rust's `std::ops::Range` is not `Copy` for... reasons.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(C)]
pub struct Range<Idx> {
	pub start: Idx,
	pub end: Idx,
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
