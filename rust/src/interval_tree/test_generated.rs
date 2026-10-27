use super::*;

#[test]
fn complement_of_nothing_is_everything() {
	let complement: Vec<Interval<u32>> = Interval::complement(&mut []);
	assert_eq!(complement, vec![Interval::new(0, u32::MAX)]);
}

/// A nested interval must not move the cursor backwards.
#[test]
fn complement_nested_intervals() {
	let intervals: &mut [Interval<u32>] = &mut [Interval::new(20, 40), Interval::new(25, 30)];
	let complement: Vec<Interval<u32>> = Interval::complement(intervals);
	assert_eq!(
		complement,
		vec![Interval::new(0, 19), Interval::new(41, u32::MAX)]
	);
}

#[test]
fn complement_duplicate_full_range() {
	let intervals: &mut [Interval<u8>] =
		&mut [Interval::new(0, u8::MAX), Interval::new(0, u8::MAX)];
	let complement: Vec<Interval<u8>> = Interval::complement(intervals);
	assert!(complement.is_empty());
}

#[test]
fn complement_signed() {
	let intervals: &mut [Interval<i8>] = &mut [Interval::new(0, 0)];
	let complement: Vec<Interval<i8>> = Interval::complement(intervals);
	assert_eq!(
		complement,
		vec![Interval::new(i8::MIN, -1), Interval::new(1, i8::MAX)]
	);
}

#[test]
fn overlap_basic() {
	assert_eq!(
		Interval::new(0, 10).overlap(&Interval::new(5, 20)),
		Some(Interval::new(5, 10))
	);
	assert_eq!(
		Interval::new(5, 20).overlap(&Interval::new(0, 10)),
		Some(Interval::new(5, 10))
	);
	assert_eq!(
		Interval::new(0, 10).overlap(&Interval::new(3, 4)),
		Some(Interval::new(3, 4))
	);
	assert_eq!(
		Interval::new(0, 10).overlap(&Interval::new(10, 10)),
		Some(Interval::new(10, 10))
	);
	// Adjacent but disjoint.
	assert_eq!(
		Interval::new(0u32, 10).overlap(&Interval::new(11, 20)),
		None
	);
	assert_eq!(
		Interval::new(11u32, 20).overlap(&Interval::new(0, 10)),
		None
	);
}

/// `overlap` must not overflow on intervals touching the representable bounds.
#[test]
fn overlap_at_bounds_does_not_overflow() {
	assert_eq!(
		Interval::new(0u8, u8::MAX).overlap(&Interval::new(0, u8::MAX)),
		Some(Interval::new(0, u8::MAX))
	);
	assert_eq!(
		Interval::new(0u8, 0).overlap(&Interval::new(0, u8::MAX)),
		Some(Interval::new(0, 0))
	);
	assert_eq!(
		Interval::new(0u8, 0).overlap(&Interval::new(u8::MAX, u8::MAX)),
		None
	);
}

#[test]
fn contains() {
	assert!(Interval::new(0u32, 10).contains(&Interval::new(2, 8)));
	assert!(Interval::new(0u32, 10).contains(&Interval::new(0, 10)));
	assert!(!Interval::new(0u32, 10).contains(&Interval::new(0, 11)));
	assert!(!Interval::new(2u32, 10).contains(&Interval::new(1, 5)));
}

#[test]
fn empty_tree_lookups() {
	let tree: IntervalTree<u32, u64> = IntervalTree::new();
	assert!(tree.is_empty());
	assert_eq!(tree.len(), 0);
	assert_eq!(tree.lookup(0), None);
	assert_eq!(tree.lookup(u32::MAX), None);
	assert_eq!(tree.lookup_interval(7), None);
}

#[test]
fn lookup_interval_returns_containing_interval() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(10, 20), 1, PolicyAdd);
	assert_eq!(tree.lookup_interval(10), Some(Interval::new(10, 20)));
	assert_eq!(tree.lookup_interval(20), Some(Interval::new(10, 20)));
	assert_eq!(tree.lookup_interval(9), None);
	assert_eq!(tree.lookup_interval(21), None);
}

/// A new interval strictly inside an existing one splits it into three.
#[test]
fn insert_strictly_contained() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 100), 1, PolicyAdd);
	tree.insert(Interval::new(40, 50), 2, PolicyAdd);
	assert_eq!(tree.len(), 3);
	assert_eq!(tree.intervals[0], (Interval::new(0, 39), 1));
	assert_eq!(tree.intervals[1], (Interval::new(40, 50), 3));
	assert_eq!(tree.intervals[2], (Interval::new(51, 100), 1));
}

/// An existing interval strictly inside the new one.
#[test]
fn insert_strictly_containing() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(40, 50), 1, PolicyAdd);
	tree.insert(Interval::new(0, 100), 2, PolicyAdd);
	assert_eq!(tree.len(), 3);
	assert_eq!(tree.intervals[0], (Interval::new(0, 39), 2));
	assert_eq!(tree.intervals[1], (Interval::new(40, 50), 3));
	assert_eq!(tree.intervals[2], (Interval::new(51, 100), 2));
}

#[test]
fn insert_exactly_equal_interval() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(5, 10), 1, PolicyAdd);
	tree.insert(Interval::new(5, 10), 2, PolicyAdd);
	assert_eq!(tree.len(), 1);
	assert_eq!(tree.intervals[0], (Interval::new(5, 10), 3));
}

/// Adjacent (touching but non-overlapping) intervals must not be merged into one another.
#[test]
fn insert_adjacent_intervals_stay_separate() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 5), 1, PolicyAdd);
	tree.insert(Interval::new(6, 10), 2, PolicyAdd);
	assert_eq!(tree.len(), 2);
	assert_eq!(tree.lookup(5), Some(&1));
	assert_eq!(tree.lookup(6), Some(&2));
}

/// Inserting in descending order exercises the `first_overlap_before > 0` splice path.
#[test]
fn insert_descending_order() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	for i in (0..10u32).rev() {
		tree.insert(Interval::new(i * 10, (i * 10) + 5), u64::from(i), PolicyAdd);
	}
	assert_eq!(tree.len(), 10);
	for i in 0..10u32 {
		assert_eq!(tree.lookup(i * 10), Some(&u64::from(i)));
		assert_eq!(tree.lookup((i * 10) + 6), None);
	}
}

/// The new interval spans several existing intervals *and* the gaps between them.
#[test]
fn insert_spanning_intervals_and_gaps() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(10, 19), 1, PolicyAdd);
	tree.insert(Interval::new(30, 39), 2, PolicyAdd);
	tree.insert(Interval::new(0, 49), 10, PolicyAdd);
	assert_eq!(
		tree.intervals,
		vec![
			(Interval::new(0, 9), 10),
			(Interval::new(10, 19), 11),
			(Interval::new(20, 29), 10),
			(Interval::new(30, 39), 12),
			(Interval::new(40, 49), 10),
		]
	);
}

#[test]
fn insert_at_min_and_max_bounds() {
	let mut tree: IntervalTree<u8, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, u8::MAX), 1, PolicyAdd);
	tree.insert(Interval::new(0, 0), 2, PolicyAdd);
	tree.insert(Interval::new(u8::MAX, u8::MAX), 4, PolicyAdd);
	assert_eq!(tree.lookup(0), Some(&3));
	assert_eq!(tree.lookup(1), Some(&1));
	assert_eq!(tree.lookup(u8::MAX - 1), Some(&1));
	assert_eq!(tree.lookup(u8::MAX), Some(&5));
}

#[test]
fn insert_full_range_over_full_range() {
	let mut tree: IntervalTree<u8, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, u8::MAX), 1, PolicyAdd);
	tree.insert(Interval::new(0, u8::MAX), 2, PolicyAdd);
	assert_eq!(tree.len(), 1);
	assert_eq!(tree.intervals[0], (Interval::new(0, u8::MAX), 3));
}

#[test]
fn insert_single_points_covering_whole_range() {
	let mut tree: IntervalTree<u8, u64> = IntervalTree::new();
	for pos in 0..=u8::MAX {
		tree.insert(Interval::new(pos, pos), u64::from(pos), PolicyAdd);
	}
	assert_eq!(tree.len(), 256);
	for pos in 0..=u8::MAX {
		assert_eq!(tree.lookup(pos), Some(&u64::from(pos)));
	}
}

#[test]
fn insert_signed_across_zero() {
	let mut tree: IntervalTree<i8, u64> = IntervalTree::new();
	tree.insert(Interval::new(i8::MIN, i8::MAX), 1, PolicyAdd);
	tree.insert(Interval::new(-1, 0), 2, PolicyAdd);
	assert_eq!(tree.lookup(i8::MIN), Some(&1));
	assert_eq!(tree.lookup(-2), Some(&1));
	assert_eq!(tree.lookup(-1), Some(&3));
	assert_eq!(tree.lookup(0), Some(&3));
	assert_eq!(tree.lookup(1), Some(&1));
	assert_eq!(tree.lookup(i8::MAX), Some(&1));
}

/// The policy must only be invoked where intervals actually overlap.
#[test]
fn policy_not_invoked_for_disjoint_inserts() {
	let mut combines: usize = 0;
	{
		let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
		tree.insert(Interval::new(0, 5), 1, PolicyAdd);
		tree.insert(
			Interval::new(10, 15),
			2,
			PolicyFunction::new(|_existing: &mut u64, _new: u64| combines += 1),
		);
	}
	assert_eq!(combines, 0);
}

#[test]
fn policy_unique_accepts_equal_values() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(1, 5), 7, PolicyUnique);
	tree.insert(Interval::new(1, 5), 7, PolicyUnique);
	assert_eq!(tree.intervals, vec![(Interval::new(1, 5), 7)]);
}

#[test]
#[should_panic]
fn policy_unique_rejects_overlapping_values() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(1, 5), 7, PolicyUnique);
	tree.insert(Interval::new(3, 9), 8, PolicyUnique);
}

#[test]
fn policy_extend_merges_containers() {
	let mut tree: IntervalTree<u32, Vec<u8>> = IntervalTree::new();
	tree.insert(Interval::new(0, 10), vec![1], PolicyExtend);
	tree.insert(Interval::new(5, 15), vec![2], PolicyExtend);
	assert_eq!(tree.lookup(0), Some(&vec![1]));
	assert_eq!(tree.lookup(5), Some(&vec![1, 2]));
	assert_eq!(tree.lookup(15), Some(&vec![2]));
}

#[test]
fn from_iter_matches_repeated_insert() {
	let entries: [(Interval<u32>, u64); 3] = [
		(Interval::new(0, 10), 1),
		(Interval::new(5, 15), 2),
		(Interval::new(15, 15), 3),
	];

	let collected: IntervalTree<u32, u64> = entries
		.iter()
		.map(|&(interval, value)| (interval, value, PolicyAdd))
		.collect();

	let mut inserted: IntervalTree<u32, u64> = IntervalTree::new();
	for (interval, value) in entries {
		inserted.insert(interval, value, PolicyAdd);
	}

	assert_eq!(collected, inserted);
}

#[test]
fn retain_removes_entries() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 5), 1, PolicyAdd);
	tree.insert(Interval::new(10, 15), 2, PolicyAdd);
	tree.retain(|(_, value)| *value != 1);
	assert_eq!(tree.len(), 1);
	assert_eq!(tree.lookup(3), None);
	assert_eq!(tree.lookup(12), Some(&2));
}

#[test]
fn iter_and_iter_mut() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 5), 1, PolicyAdd);
	tree.insert(Interval::new(10, 15), 2, PolicyAdd);

	assert_eq!(
		tree.iter()
			.map(|(interval, _)| interval)
			.collect::<Vec<_>>(),
		vec![Interval::new(0, 5), Interval::new(10, 15)]
	);

	for (_, value) in tree.iter_mut() {
		*value *= 10;
	}
	assert_eq!(tree.lookup(0), Some(&10));
	assert_eq!(tree.lookup(10), Some(&20));
}

#[test]
fn coalesce_empty_and_singleton() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.coalesce();
	assert!(tree.is_empty());

	tree.insert(Interval::new(3, 7), 1, PolicyAdd);
	tree.coalesce();
	assert_eq!(tree.intervals, vec![(Interval::new(3, 7), 1)]);
}

#[test]
fn coalesce_merges_adjacent_equal_values() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 5), 1, PolicyAdd);
	tree.insert(Interval::new(6, 10), 1, PolicyAdd);
	tree.insert(Interval::new(11, 20), 1, PolicyAdd);
	assert_eq!(tree.len(), 3);
	tree.coalesce();
	assert_eq!(tree.intervals, vec![(Interval::new(0, 20), 1)]);
}

#[test]
fn coalesce_keeps_adjacent_different_values() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 5), 1, PolicyAdd);
	tree.insert(Interval::new(6, 10), 2, PolicyAdd);
	tree.coalesce();
	assert_eq!(
		tree.intervals,
		vec![(Interval::new(0, 5), 1), (Interval::new(6, 10), 2)]
	);
}

/// Equal values that are *not* adjacent must not be merged.
#[test]
fn coalesce_keeps_non_adjacent_equal_values() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 5), 1, PolicyAdd);
	tree.insert(Interval::new(7, 10), 1, PolicyAdd);
	tree.coalesce();
	assert_eq!(
		tree.intervals,
		vec![(Interval::new(0, 5), 1), (Interval::new(7, 10), 1)]
	);
	assert_eq!(tree.lookup(6), None);
}

#[test]
fn coalesce_merges_runs_and_preserves_boundaries() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	for (interval, value) in [
		(Interval::new(0, 1), 1),
		(Interval::new(2, 3), 1),
		(Interval::new(4, 5), 2),
		(Interval::new(6, 7), 2),
		(Interval::new(8, 9), 2),
		(Interval::new(11, 12), 2),
		(Interval::new(13, 14), 3),
	] {
		tree.insert(interval, value, PolicyAdd);
	}
	tree.coalesce();
	assert_eq!(
		tree.intervals,
		vec![
			(Interval::new(0, 3), 1),
			(Interval::new(4, 9), 2),
			(Interval::new(11, 12), 2),
			(Interval::new(13, 14), 3),
		]
	);
}

/// Coalescing an interval ending at `T::MAX` must not overflow in `up()`.
#[test]
fn coalesce_at_max_does_not_overflow() {
	let mut tree: IntervalTree<u8, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 127), 1, PolicyAdd);
	tree.insert(Interval::new(128, u8::MAX), 1, PolicyAdd);
	tree.coalesce();
	assert_eq!(tree.intervals, vec![(Interval::new(0, u8::MAX), 1)]);

	// A tree that is a single full-range interval is already coalesced.
	tree.coalesce();
	assert_eq!(tree.intervals, vec![(Interval::new(0, u8::MAX), 1)]);
}

#[test]
fn coalesce_is_idempotent() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	for i in 0..20u32 {
		tree.insert(
			Interval::new(i * 2, (i * 2) + 1),
			u64::from(i % 3),
			PolicyAdd,
		);
	}
	tree.coalesce();
	let once: Vec<(Interval<u32>, u64)> = tree.intervals.clone();
	tree.coalesce();
	assert_eq!(tree.intervals, once);
}

/// Coalescing must never change which value a position maps to.
#[test]
fn randomized_coalesce_preserves_lookups() {
	let mut rng: Rng = Rng(0x9E37_79B9_7F4A_7C15);

	for _ in 0..2000 {
		let mut tree: IntervalTree<u8, u64> = IntervalTree::new();
		for _ in 0..(1 + rng.below(8)) {
			let (a, b): (u8, u8) = (
				u8::try_from(rng.below(64)).unwrap(),
				u8::try_from(rng.below(64)).unwrap(),
			);
			// Few distinct values, so adjacent-equal runs are common.
			let value: u64 = u64::from(rng.below(3));
			tree.insert(Interval::new(a.min(b), a.max(b)), value, PolicyOverwrite);
		}

		let before: Vec<Option<u64>> = (0..=u8::MAX).map(|pos| tree.lookup(pos).copied()).collect();
		let len_before: usize = tree.len();
		tree.coalesce();
		let after: Vec<Option<u64>> = (0..=u8::MAX).map(|pos| tree.lookup(pos).copied()).collect();

		assert_eq!(before, after, "coalesce changed lookups");
		assert!(tree.len() <= len_before);

		// The result must be fully coalesced: no adjacent-and-equal pair remains.
		for window in tree.intervals.windows(2) {
			let (left, right): (&(Interval<u8>, u64), &(Interval<u8>, u64)) =
				(&window[0], &window[1]);
			assert!(
				!((left.0.end() != u8::MAX)
					&& (left.0.end().up() == right.0.start())
					&& (left.1 == right.1)),
				"not fully coalesced: {:?}",
				tree.intervals
			);
		}
	}
}

/// The new value simply replaces the existing one; used to produce adjacent equal values.
struct PolicyOverwrite;

impl<T> Policy<T> for PolicyOverwrite {
	fn combine(&mut self, existing: &mut T, new: T) {
		*existing = new;
	}
}

/// A trivial deterministic xorshift generator, so the randomized tests below
/// reproduce exactly without pulling in a dependency.
struct Rng(u64);

impl Rng {
	fn next(&mut self) -> u64 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		self.0
	}

	fn below(&mut self, bound: u32) -> u32 {
		u32::try_from(self.next() % u64::from(bound)).unwrap()
	}
}

/// The reference model: every position tracked independently.
fn reference_insert(cells: &mut [Option<u64>], interval: Interval<u8>, value: u64) {
	for cell in cells
		.iter_mut()
		.take(usize::from(interval.end()) + 1)
		.skip(usize::from(interval.start()))
	{
		*cell = Some(cell.map_or(value, |existing| existing + value));
	}
}

/// Compare `insert` against the per-position reference model over random inputs,
/// biased towards the representable bounds so that `up`/`down` are exercised at the edges.
#[test]
fn randomized_insert_matches_reference() {
	let mut rng: Rng = Rng(0x243F_6A88_85A3_08D3);

	for _ in 0..2000 {
		let mut tree: IntervalTree<u8, u64> = IntervalTree::new();
		let mut cells: [Option<u64>; 256] = [None; 256];

		for _ in 0..(1 + rng.below(6)) {
			let endpoint = |rng: &mut Rng| -> u8 {
				match rng.below(4) {
					0 => 0,
					1 => u8::MAX,
					_ => u8::try_from(rng.below(256)).unwrap(),
				}
			};
			let (a, b): (u8, u8) = (endpoint(&mut rng), endpoint(&mut rng));
			let interval: Interval<u8> = Interval::new(a.min(b), a.max(b));
			let value: u64 = u64::from(1 + rng.below(4));

			tree.insert(interval, value, PolicyAdd);
			reference_insert(&mut cells, interval, value);

			for pos in 0..=u8::MAX {
				assert_eq!(
					tree.lookup(pos),
					cells[usize::from(pos)].as_ref(),
					"mismatch at {pos} after inserting {interval:?} => {value}"
				);
			}
		}
	}
}

/// `complement` must return exactly the positions no input interval covers.
#[test]
fn randomized_complement_matches_reference() {
	let mut rng: Rng = Rng(0x4528_21E6_38D0_1377);

	for _ in 0..2000 {
		let mut intervals: Vec<Interval<u8>> = Vec::new();
		for _ in 0..rng.below(6) {
			let a: u8 = u8::try_from(rng.below(256)).unwrap();
			let b: u8 = u8::try_from(rng.below(256)).unwrap();
			intervals.push(Interval::new(a.min(b), a.max(b)));
		}

		let mut covered: [bool; 256] = [false; 256];
		for interval in intervals.iter() {
			for cell in covered
				.iter_mut()
				.take(usize::from(interval.end()) + 1)
				.skip(usize::from(interval.start()))
			{
				*cell = true;
			}
		}

		let complement: Vec<Interval<u8>> = Interval::complement(&mut intervals);

		let mut uncovered: [bool; 256] = [false; 256];
		let mut maybe_previous: Option<u8> = None;
		for interval in complement.iter() {
			if let Some(previous) = maybe_previous {
				assert!(
					interval.start() > previous,
					"complement is not disjoint: {complement:?}"
				);
			}
			maybe_previous = Some(interval.end());
			for cell in uncovered
				.iter_mut()
				.take(usize::from(interval.end()) + 1)
				.skip(usize::from(interval.start()))
			{
				*cell = true;
			}
		}

		for pos in 0..256 {
			assert_eq!(
				uncovered[pos], !covered[pos],
				"mismatch at {pos}: {complement:?}"
			);
		}
	}
}

/// `overlap` must agree with the set-intersection of the two interval's positions.
#[test]
fn randomized_overlap_matches_reference() {
	let mut rng: Rng = Rng(0xB7E1_5162_8AED_2A6A);

	for _ in 0..20000 {
		let interval = |rng: &mut Rng| -> Interval<u8> {
			let a: u8 = u8::try_from(rng.below(32)).unwrap();
			let b: u8 = u8::try_from(rng.below(32)).unwrap();
			Interval::new(a.min(b), a.max(b))
		};
		let (left, right): (Interval<u8>, Interval<u8>) = (interval(&mut rng), interval(&mut rng));

		let expected: Vec<u8> = (0..32u8)
			.filter(|&pos| {
				(left.start() <= pos)
					&& (pos <= left.end())
					&& (right.start() <= pos)
					&& (pos <= right.end())
			})
			.collect();

		match left.overlap(&right) {
			None => assert!(expected.is_empty(), "{left:?} and {right:?} do overlap"),
			Some(overlap) => {
				assert_eq!(
					Some(&overlap.start()),
					expected.first(),
					"{left:?} {right:?}"
				);
				assert_eq!(Some(&overlap.end()), expected.last(), "{left:?} {right:?}");
			},
		}

		// `overlap` is symmetric.
		assert_eq!(left.overlap(&right), right.overlap(&left));
	}
}
