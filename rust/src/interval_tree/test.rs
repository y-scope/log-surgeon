use super::*;

#[test]
fn basic_operations() {
	{
		let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
		tree.insert(Interval::new(0, 10), 1, PolicyAdd);
		tree.insert(Interval::new(5, 15), 2, PolicyAdd);
		tree.insert(Interval::new(15, 15), 3, PolicyAdd);
		assert_eq!(tree.len(), 4);
		assert_eq!(tree.intervals[0].0, Interval::new(0, 4));
		assert_eq!(tree.intervals[0].1, 1);
		assert_eq!(tree.intervals[1].0, Interval::new(5, 10));
		assert_eq!(tree.intervals[1].1, 3);
		assert_eq!(tree.intervals[2].0, Interval::new(11, 14));
		assert_eq!(tree.intervals[2].1, 2);
		assert_eq!(tree.intervals[3].0, Interval::new(15, 15));
		assert_eq!(tree.intervals[3].1, 5);
		assert_eq!(tree.lookup(3), Some(&1));
		assert_eq!(tree.lookup(4), Some(&1));
		assert_eq!(tree.lookup(5), Some(&3));
		assert_eq!(tree.lookup(10), Some(&3));
		assert_eq!(tree.lookup(12), Some(&2));
		assert_eq!(tree.lookup(15), Some(&5));
		assert_eq!(tree.lookup(16), None);
	}
}

#[test]
fn insert_across_multiple_intervals() {
	{
		let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
		tree.insert(Interval::new(119, 119), 1, PolicyAdd);
		tree.insert(Interval::new(120, 120), 1, PolicyAdd);
		tree.insert(Interval::new(117, u32::MAX), 1, PolicyAdd);
		assert_eq!(tree.len(), 4);
		assert_eq!(tree.intervals[0].0, Interval::new(117, 118));
		assert_eq!(tree.intervals[0].1, 1);
		assert_eq!(tree.intervals[1].0, Interval::new(119, 119));
		assert_eq!(tree.intervals[1].1, 2);
		assert_eq!(tree.intervals[2].0, Interval::new(120, 120));
		assert_eq!(tree.intervals[2].1, 2);
		assert_eq!(tree.intervals[3].0, Interval::new(121, u32::MAX));
		assert_eq!(tree.intervals[3].1, 1);
	}
}

#[test]
fn serde_roundtrip() {
	let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
	tree.insert(Interval::new(0, 10), 1, PolicyAdd);
	tree.insert(Interval::new(20, 30), 2, PolicyAdd);

	let json: String = serde_json::to_string(&tree).unwrap();
	assert_eq!(
		json,
		r#"[{"start":0,"end":10,"value":1},{"start":20,"end":30,"value":2}]"#
	);

	let roundtripped: IntervalTree<u32, u64> = serde_json::from_str(&json).unwrap();
	assert_eq!(tree, roundtripped);
}

#[test]
fn complement_multiple_intervals() {
	let intervals: &mut [Interval<u32>] = &mut [
		Interval { start: 10, end: 15 },
		Interval { start: 20, end: 30 },
		Interval { start: 25, end: 40 },
	];
	let complement: Vec<Interval<u32>> = Interval::complement(intervals);
	assert_eq!(complement[0], Interval { start: 0, end: 9 });
	assert_eq!(complement[1], Interval { start: 16, end: 19 });
	assert_eq!(
		complement[2],
		Interval {
			start: 41,
			end: u32::MAX,
		}
	);
	assert_eq!(complement.len(), 3);
}

#[test]
fn complement_lower_is_min() {
	let intervals: &mut [Interval<u32>] = &mut [Interval { start: 0, end: 10 }];
	let complement: Vec<Interval<u32>> = Interval::complement(intervals);
	assert_eq!(complement.len(), 1);
	assert_eq!(
		complement[0],
		Interval {
			start: 11,
			end: u32::MAX,
		}
	);
}

#[test]
fn complement_lower_is_min_plus_one() {
	let intervals: &mut [Interval<u32>] = &mut [Interval { start: 1, end: 10 }];
	let complement: Vec<Interval<u32>> = Interval::complement(intervals);
	assert_eq!(complement.len(), 2);
	assert_eq!(complement[0], Interval { start: 0, end: 0 });
	assert_eq!(
		complement[1],
		Interval {
			start: 11,
			end: u32::MAX,
		}
	);
}

#[test]
fn complement_upper_is_max_minus_one() {
	let intervals: &mut [Interval<u32>] = &mut [Interval {
		start: 10,
		end: u32::MAX - 1,
	}];
	let complement: Vec<Interval<u32>> = Interval::complement(intervals);
	assert_eq!(complement.len(), 2);
	assert_eq!(complement[0], Interval { start: 0, end: 9 });
	assert_eq!(
		complement[1],
		Interval {
			start: u32::MAX,
			end: u32::MAX,
		}
	);
}
#[test]
fn complement_upper_is_max() {
	let intervals: &mut [Interval<u32>] = &mut [Interval {
		start: 10,
		end: u32::MAX,
	}];
	let complement: Vec<Interval<u32>> = Interval::complement(intervals);
	assert_eq!(complement.len(), 1);
	assert_eq!(complement[0], Interval { start: 0, end: 9 });
}
