use std::cmp::Ordering;

/// A naive interval "tree" implementation;
/// only constructed when building NFAs,
/// so we optimize for simplicity (correctness) and lookups.
///
/// Internally, just an ordered list of non-overlapping intervals;
/// lookups are `O(log(n))` with binary search.
#[derive(Debug, Clone)]
pub struct IntervalTree<T: Number, V: Clone> {
	intervals: Vec<(Interval<T>, V)>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Interval<T: Number> {
	start: T,
	end: T,
}

pub trait Number: Ord + std::fmt::Debug {
	const MIN: Self;
	const MAX: Self;

	fn up(&self) -> Self;
	fn down(&self) -> Self;
}

/// When inserting a value with an overlapping interval into an [`IntervalTree`],
/// a `Policy` determines how the existing and new values are combined.
pub trait Policy<T> {
	fn combine(&mut self, existing: &mut T, new: T);
}

/// Combine values using the [`Extend`] trait; e.g. for containers.
#[derive(Debug)]
pub struct PolicyExtend;

/// Combine values using the [`Add`] trait; e.g. for numbers.
#[derive(Debug)]
pub struct PolicyAdd;

/// Do nothing; for the unit type/object `()`.
#[derive(Debug)]
pub struct PolicyNoop;

/// Panics if overlapping intervals are inserted.
pub struct PolicyUnique;

/// Use an arbitrary function to combine values.
#[derive(Debug)]
pub struct PolicyFunction<T>(T);

enum Intersection<T: Number> {
	Same,
	DisjointLeftLower,
	DisjointRightLower,
	SameStartLeftExtendsRight {
		overlap: Interval<T>,
		remaining: Interval<T>,
	},
	SameStartRightExtendsLeft {
		overlap: Interval<T>,
		remaining: Interval<T>,
	},
	LeftFirst {
		before: Interval<T>,
		overlap_start: T,
	},
	RightFirst {
		before: Interval<T>,
		overlap_start: T,
	},
}

impl<T: Number, V: Clone> IntervalTree<T, V> {
	pub const fn new() -> Self {
		Self { intervals: Vec::new() }
	}

	pub fn len(&self) -> usize {
		self.intervals.len()
	}

	pub fn is_empty(&self) -> bool {
		self.intervals.is_empty()
	}
}

impl<T: Number, V: Clone> IntervalTree<T, V>
where
	T: Copy,
{
	pub fn iter(&self) -> impl Iterator<Item = (Interval<T>, &V)> {
		self.intervals.iter().map(|(interval, value)| (*interval, value))
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (Interval<T>, &mut V)> {
		self.intervals.iter_mut().map(|(interval, value)| (*interval, value))
	}
}

impl<T: Number, V: Clone, P> FromIterator<(Interval<T>, V, P)> for IntervalTree<T, V>
where
	T: Copy,
	P: Policy<V>,
{
	fn from_iter<I>(iter: I) -> Self
	where
		I: IntoIterator<Item = (Interval<T>, V, P)>,
	{
		let mut this: Self = Self::new();
		for (interval, value, policy) in iter.into_iter() {
			this.insert(interval, value, policy);
		}
		this
	}
}

impl<T: Number, V: Clone> IntervalTree<T, V>
where
	T: Copy,
{
	pub fn lookup(&self, pos: T) -> Option<&V> {
		self.lookup_entry(pos).map(|(_, value)| value)
	}

	pub fn lookup_interval(&self, pos: T) -> Option<Interval<T>> {
		self.lookup_entry(pos).map(|(interval, _)| interval)
	}

	pub fn insert<P>(&mut self, mut new: Interval<T>, new_value: V, mut policy: P)
	where
		P: Policy<V>,
	{
		// Existing intervals in the tree are disjoint,
		// but the new interval being inserted may intersect with multiple existing intervals,
		// so we process it iteratively.
		// Ideally, this loop could be written in a way such that one can explicitly see that it terminates,
		// e.g. `while new.start <= new.end` and `new.start` advances every iteration;
		// unfortunately, for the case that `existing.start < new.start`,
		// we split `existing` into `[existing.start, new.start)` and `[new.start, existing.end]` and loop again;
		// the next iteration should be one of the `SameStart*` cases, which do advance or terminate,
		// but can't think of a strictly "cleaner" way to model this invariant in code.
		loop {
			let index: usize = self.partition_point(new.start);
			if let Some((existing, existing_value)) = self.intervals.get_mut(index) {
				assert!(new.start <= existing.end);

				// The explicit `break`s and `continue`s are intentional for readability.
				match new.intersection(existing) {
					Intersection::Same => {
						policy.combine(existing_value, new_value.clone());
						break;
					},
					Intersection::DisjointLeftLower => {
						self.intervals.insert(index, (new, new_value.clone()));
						break;
					},
					Intersection::DisjointRightLower => {
						self.intervals.insert(index + 1, (new, new_value.clone()));
						break;
					},
					Intersection::SameStartLeftExtendsRight { overlap, remaining } => {
						// The new interval extends longer.
						*existing = overlap;
						policy.combine(existing_value, new_value.clone());
						assert_eq!(remaining.end, new.end);
						new = remaining;
						continue;
					},
					Intersection::SameStartRightExtendsLeft { overlap, remaining } => {
						// The existing interval extends longer.
						*existing = remaining;
						let mut merged: V = existing_value.clone();
						policy.combine(&mut merged, new_value.clone());
						self.intervals.insert(index, (overlap, merged));
						break;
					},
					Intersection::LeftFirst { before, overlap_start } => {
						// The new interval comes before.
						self.intervals.insert(index, (before, new_value.clone()));
						new.start = overlap_start;
						continue;
					},
					Intersection::RightFirst { before, overlap_start } => {
						// The new interval comes after; split the existing interval.
						existing.start = overlap_start;
						let tmp: V = existing_value.clone();
						self.intervals.insert(index, (before, tmp));
						assert_eq!(overlap_start, new.start);
						continue;
					},
				}
			} else {
				assert_eq!(index, self.intervals.len());
				self.intervals.push((new, new_value));
				break;
			}
		}
		self.check_invariants();
	}

	pub fn retain<P>(&mut self, predicate: P)
	where
		P: FnMut(&(Interval<T>, V)) -> bool,
	{
		self.intervals.retain(predicate);
	}
}

impl<T: Number, V: Clone> IntervalTree<T, V>
where
	T: Copy,
{
	fn lookup_entry(&self, pos: T) -> Option<(Interval<T>, &V)> {
		self.check_invariants();
		let index: usize = self.partition_point(pos);
		if let Some((interval, value)) = self.intervals.get(index) {
			assert!(pos <= interval.end);
			if interval.start <= pos {
				Some((*interval, value))
			} else {
				None
			}
		} else {
			None
		}
	}

	/// Informally, returns the location of the first interval that "goes past" `pos`.
	/// If `pos` is "past" every interval, returns `self.intervals.len()`.
	/// Otherwise, `self.intervals[pos].end >= pos`.
	fn partition_point(&self, pos: T) -> usize {
		self.check_invariants();
		// `partition_point` assumes partitioning as `[true, ..., false]` and returns the index of the first `false`.
		self.intervals.partition_point(|(interval, _)| interval.end < pos)
	}

	/// Checks that intervals are non-overlapping.
	fn check_invariants(&self) {
		let mut maybe_previous: Option<T> = None;
		for (interval, _) in self.intervals.iter() {
			if let Some(previous) = maybe_previous {
				assert!(interval.start() > previous);
				maybe_previous = Some(interval.end());
			}
		}
	}
}

impl<T: Number> Interval<T> {
	pub fn new(start: T, end: T) -> Self {
		assert!(start <= end);
		Self { start, end }
	}
}

impl<T: Number> Interval<T>
where
	T: Copy,
{
	pub fn start(&self) -> T {
		self.start
	}

	pub fn end(&self) -> T {
		self.end
	}

	pub fn contains(&self, other: &Interval<T>) -> bool {
		(self.start <= other.start) && (other.end <= self.end)
	}

	pub fn complement(intervals: &mut [Interval<T>]) -> Vec<Interval<T>> {
		intervals.sort_unstable();

		let mut complement: Vec<Interval<T>> = Vec::new();

		let mut cursor: T = T::MIN;
		for &Interval { start, end } in intervals.iter() {
			if cursor < start {
				complement.push(Interval::new(cursor, start.down()));
			}
			if end < T::MAX {
				cursor = end.up();
			} else {
				return complement;
			}
		}

		complement.push(Interval::new(cursor, T::MAX));

		complement
	}

	pub fn overlap(&self, other: &Self) -> Option<Self> {
		match self.intersection(other) {
			Intersection::Same => Some(*self),
			Intersection::DisjointLeftLower | Intersection::DisjointRightLower => None,
			Intersection::SameStartLeftExtendsRight { overlap, .. }
			| Intersection::SameStartRightExtendsLeft { overlap, .. } => Some(overlap),
			Intersection::LeftFirst { overlap_start, .. } | Intersection::RightFirst { overlap_start, .. } => {
				Some(Interval::new(overlap_start, std::cmp::min(self.end, other.end)))
			},
		}
	}

	fn intersection(&self, other: &Self) -> Intersection<T> {
		if self.end < other.start {
			return Intersection::DisjointLeftLower;
		} else if other.end < self.start {
			return Intersection::DisjointRightLower;
		}

		match (self.start.cmp(&other.start), self.end.cmp(&other.end)) {
			(Ordering::Equal, Ordering::Equal) => Intersection::Same,
			(Ordering::Equal, Ordering::Less) => Intersection::SameStartRightExtendsLeft {
				overlap: Interval::new(self.start, self.end),
				remaining: Interval::new(self.end.up(), other.end),
			},
			(Ordering::Equal, Ordering::Greater) => Intersection::SameStartLeftExtendsRight {
				overlap: Interval::new(other.start, other.end),
				remaining: Interval::new(other.end.up(), self.end),
			},
			(Ordering::Less, _) => Intersection::LeftFirst {
				before: Interval::new(self.start, other.start.down()),
				overlap_start: other.start,
			},
			(Ordering::Greater, _) => Intersection::RightFirst {
				before: Interval::new(other.start, self.start.down()),
				overlap_start: self.start,
			},
		}
	}
}

macro_rules! number_impl {
	($ty:ty, $($tt:tt)*) => {
		number_impl!($ty);
		number_impl!($($tt)*);
	};
	($ty:ty) => {
		impl Number for $ty {
			const MIN: Self = <$ty>::MIN;
			const MAX: Self = <$ty>::MAX;

			fn up(&self) -> Self {
				self.checked_add(1).unwrap()
			}

			fn down(&self) -> Self {
				self.checked_sub(1).unwrap()
			}
		}
	};
}

number_impl!(u8, u16, u32, u64, usize);
number_impl!(i8, i16, i32, i64, isize);

impl<T> Policy<T> for PolicyExtend
where
	T: IntoIterator + Extend<<T as IntoIterator>::Item>,
{
	fn combine(&mut self, existing: &mut T, new: T) {
		existing.extend(new);
	}
}

impl<T> Policy<T> for PolicyAdd
where
	T: std::ops::Add<Output = T> + Copy,
{
	fn combine(&mut self, existing: &mut T, new: T) {
		*existing = *existing + new;
	}
}

impl Policy<()> for PolicyNoop {
	fn combine(&mut self, _existing: &mut (), _new: ()) {}
}

impl<T> Policy<T> for PolicyUnique
where
	T: Eq + std::fmt::Debug,
{
	fn combine(&mut self, existing: &mut T, new: T) {
		assert_eq!(&new, existing);
	}
}

/// Ideally, we could implement `Policy<T>` on `F: FnMut(&mut T, T)` directly,
/// but we can't due to a limitation on Rust's trait solver.
/// Further, we need this helper function to make the trait bounds resolve properly.
impl<F> PolicyFunction<F> {
	pub fn new<T>(f: F) -> Self
	where
		F: for<'a> FnMut(&'a mut T, T),
	{
		Self(f)
	}
}

impl<T, F> Policy<T> for PolicyFunction<F>
where
	F: FnMut(&mut T, T),
{
	fn combine(&mut self, existing: &mut T, new: T) {
		self.0(existing, new);
	}
}

#[cfg(test)]
mod test {
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
}
