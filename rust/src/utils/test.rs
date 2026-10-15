/// A deterministic pseudo-random number generator (xorshift64).
pub(crate) fn rng(seed: u64) -> impl FnMut() -> u64 {
	let mut state: u64 = seed;
	move || {
		state ^= state << 13;
		state ^= state >> 7;
		state ^= state << 17;
		state
	}
}
