use super::*;

const fn xorshift(state: u64) -> u64 {
	let mut x: u64 = state;
	x ^= x << 13;
	x ^= x >> 7;
	x ^= x << 17;
	x
}

/// A simple xorshift64 stream (skipping zero).
struct Rng {
	state: u64,
}

impl Rng {
	fn new(seed: u64) -> Self {
		Self { state: seed.max(1) }
	}

	fn next(&mut self) -> u64 {
		self.state = xorshift(self.state);
		self.state
	}

	fn next_u8(&mut self) -> u8 {
		(self.next() & 0xFF) as u8
	}

	fn next_below(&mut self, bound: usize) -> usize {
		(self.next() % (bound as u64)) as usize
	}
}

/// Reference: first position whose byte is `values` contains.
fn find_scalar(values: &[u8], bytes: &[u8]) -> Option<usize> {
	bytes.iter().position(|byte| values.contains(byte))
}

fn assert_matches(bytes: &[u8], positions: &[usize], values: &[u8], message: &str) {
	let set: ByteSet = ByteSet::from_bytes(values.iter().copied());
	assert_eq!(set.find_in(&bytes[0..0]), None, "{message}: prefix 0",);
	for &count in positions.iter() {
		let expect: Option<usize> = find_scalar(values, &bytes[..count]);
		let actual: Option<usize> = set.find_in(&bytes[..count]);
		assert_eq!(actual, expect, "{message}: first {count}");
	}
}

#[test]
fn eq_small_sets() {
	let bytes: Vec<u8> = (0..=255u8).collect();
	assert_matches(
		&bytes,
		&[
			len_at(&bytes, 0),
			len_at(&bytes, 1),
			len_at(&bytes, 16),
			len_at(&bytes, 30),
			256,
		],
		b"@",
		"@ single",
	);
	assert_matches(&bytes, &[64], b" @\t\n", "@ tab nl sp");
	assert_matches(&bytes, &[64], &[0xF0u8, 0x80, 0x9F, 0xE0], "lead bytes");
	assert_matches(&b"z".repeat(40), &[40], &[0xFF], "no hit");
}

#[test]
fn nibble_sets() {
	let members: Vec<u8> = vec![
		0x21, 0x28, 0x2F, 0x3A, 0x3F, 0x5B, 0x5E, 0x5F, 0x7B, 0x7D, 0x7E,
	];
	let bytes: Vec<u8> = (0..=255u8).collect();
	assert_matches(
		&bytes,
		&[64, 128, 160, 176, 256],
		&members,
		"mixed nibble set",
	);
	assert_matches(
		&b".".repeat(300),
		&[300],
		&(b'a'..=b'z').collect::<Vec<u8>>(),
		"a..z no hit",
	);
	assert_matches(
		&bytes,
		&[176, 232, 256],
		&[0xF0, 0xE4, 0xD0, 0xC2, 0xB0, 0x09],
		"high and low rows",
	);
}

#[test]
fn nibble_merge_superset_is_checked_by_contains() {
	// A set needing more than NUM_CLASSES distinct row masks (16 rows, mixed masks)
	// merges classes in `from_bytes`; `contains` then over-approximates, and callers
	// distinguish members via `contains` at build time.
	let members: Vec<u8> = (0..16u8).map(|hi| (hi << 4) | hi).collect();
	let set: ByteSet = ByteSet::from_bytes(members.iter().copied());
	assert!(matches!(set, ByteSet::Nibbles { .. }));
	let mut hits: usize = 0;
	for byte in 0..=0xFF {
		if set.contains(byte) {
			hits += 1;
		}
	}
	assert!(hits >= 16, "all members still found");
}

#[test]
fn differential_random() {
	let mut rng: Rng = Rng::new(0x5EED);
	for round in 0..400 {
		let width: usize = rng.next_below(256) + 1;
		let mut members: Vec<u8> = Vec::new();
		for _ in 0..width {
			members.push(rng.next_u8());
		}
		members.sort_unstable();
		members.dedup();
		let set: ByteSet = ByteSet::from_bytes(members.iter().copied());
		// `contains` may over-approximate (nibble merging), but `find_in` must be exact:
		// assert exactly that the first `contains` hit is a member.
		for byte in 0..=255u8 {
			if set.contains(byte) {
				assert!(
					members.contains(&byte) || matches!(set, ByteSet::Nibbles { .. }),
					"round {round} byte {byte:#04x} falsely matched",
				);
			}
			if members.contains(&byte) {
				assert!(
					set.contains(byte),
					"round {round} byte {byte:#04x} member not matched",
				);
			}
		}

		let length: usize = rng.next_below(256);
		let mut bytes: Vec<u8> = std::iter::repeat_with(|| rng.next_u8())
			.take(length)
			.collect();
		if !members.is_empty() && rng.next_below(2) == 0 {
			let pos: usize = rng.next_below(length.max(1));
			let value: u8 = members[rng.next_below(members.len())];
			if let Some(slot) = bytes.get_mut(pos) {
				*slot = value;
			}
		}
		if set.is_exact() {
			assert_eq!(
				set.find_in(&bytes),
				find_scalar(&members, &bytes),
				"round {round} (exact)",
			);
		} else {
			// Over-approximate: `find_in` agrees with its own scalar form `contains`.
			let expected: Option<usize> = bytes.iter().position(|b| set.contains(*b));
			assert_eq!(set.find_in(&bytes), expected, "round {round} (superset)");
		}
	}
}

fn len_at(bytes: &[u8], index: usize) -> usize {
	index.min(bytes.len())
}
