use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct IntervalTree<T: Number, V: Clone> {
	intervals: Vec<(Interval<T>, V)>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct Interval<T: Number> {
	start: T,
	end: T,
}

pub trait Number: Ord {
	const MIN: Self;
	const MAX: Self;

	fn up(&self) -> Self;
	fn down(&self) -> Self;
}

enum Intersection<T: Number> {
	Same,
	Disjoint {
		lower_is: Side,
	},
	ContainsNeighbourhood {
		lower: Interval<T>,
		overlap: Interval<T>,
		upper: Interval<T>,
		lower_is: Side,
	},
	SameStart {
		overlap: Interval<T>,
		upper: Interval<T>,
		lower_is: Side,
	},
	SameEnd {
		lower: Interval<T>,
		overlap: Interval<T>,
		lower_is: Side,
	},
	Partial {
		lower: Interval<T>,
		overlap: Interval<T>,
		upper: Interval<T>,
		lower_is: Side,
	},
}

enum Side {
	Left,
	Right,
}

impl<T: Number, V: Clone> IntervalTree<T, V> {
	pub fn new() -> Self {
		Self { intervals: Vec::new() }
	}

	pub fn len(&self) -> usize {
		self.intervals.len()
	}

	pub fn iter(&self) -> impl Iterator<Item = &(Interval<T>, V)> {
		self.intervals.iter()
	}
}

impl<T: Number, V: Clone> IntervalTree<T, V>
where
	T: Copy,
{
	pub fn lookup(&self, pos: T) -> Option<&V> {
		self.invariants();
		let index: usize = self.partition_point(pos);
		if let Some((interval, value)) = self.intervals.get(index) {
			assert!(pos <= interval.end);
			if interval.start <= pos { Some(value) } else { None }
		} else {
			None
		}
	}

	pub fn insert<Merge>(&mut self, interval: Interval<T>, value: V, merge: Merge)
	where
		Merge: Fn(&V, &V) -> V,
	{
		let index: usize = self.partition_point(interval.start);
		if let Some((other, other_value)) = self.intervals.get_mut(index) {
			assert!(interval.start <= other.end);
			match interval.intersection(other) {
				Intersection::Same => {
					*other_value = merge(other_value, &value);
				},
				Intersection::Disjoint { lower_is } => match lower_is {
					Side::Left => {
						self.intervals.insert(index, (interval, value));
					},
					Side::Right => {
						self.intervals.insert(index + 1, (interval, value));
					},
				},
				Intersection::ContainsNeighbourhood {
					lower,
					overlap,
					upper,
					lower_is,
				} => match lower_is {
					Side::Left => {
						*other_value = merge(other_value, &value);
						self.intervals.insert(index + 1, (upper, value.clone()));
						self.intervals.insert(index, (lower, value));
					},
					Side::Right => {
						let old_other_value: V = other_value.clone();
						*other = interval;
						*other_value = merge(other_value, &value);
						self.intervals.insert(index + 1, (upper, old_other_value.clone()));
						self.intervals.insert(index, (lower, old_other_value));
					},
				},
				Intersection::SameStart {
					overlap,
					upper,
					lower_is,
				} => match lower_is {
					Side::Left => {
						let merged: V = merge(other_value, &value);
						*other = upper;
						self.intervals.insert(index, (overlap, merged));
					},
					Side::Right => {
						*other_value = merge(other_value, &value);
						self.intervals.insert(index + 1, (upper, value));
					},
				},
				Intersection::SameEnd {
					lower,
					overlap,
					lower_is,
				} => match lower_is {
					Side::Left => {
						*other_value = merge(&other_value, &value);
						self.intervals.insert(index, (lower, value));
					},
					Side::Right => {
						let merged: V = merge(other_value, &value);
						*other = lower;
						self.intervals.insert(index + 1, (overlap, merged));
					},
				},
				Intersection::Partial {
					lower,
					overlap,
					upper,
					lower_is,
				} => {
					let merged: V = merge(other_value, &value);
					match lower_is {
						Side::Left => {
							*other = upper;
							self.intervals.insert(index, (overlap, merged));
							self.intervals.insert(index, (lower, value));
						},
						Side::Right => {
							*other = lower;
							self.intervals.insert(index + 1, (upper, value));
							self.intervals.insert(index + 1, (overlap, merged));
						},
					}
				},
			}
		} else {
			assert!(index == self.intervals.len());
			self.intervals.push((interval, value));
		}
		self.invariants();
	}
}

impl<T: Number, V: Clone> IntervalTree<T, V>
where
	T: Copy,
{
	fn partition_point(&self, pos: T) -> usize {
		self.invariants();
		// `partition_point` assumes partitioning as `[true, ..., false]` and returns the index of the first `false`.
		let index: usize = self.intervals.partition_point(|(interval, _)| interval.end < pos);
		// `index` is the position of first interval that "goes past" `pos`; `interval.end >= pos`.
		index
	}

	fn invariants(&self) {
		let mut maybe_previous: Option<T> = None;
		for (interval, _) in self.intervals.iter() {
			if let Some(previous) = maybe_previous {
				assert!(interval.start() > previous);
				maybe_previous = Some(interval.end());
			}
		}
	}
}

impl<T: Number, V: Clone> std::ops::Index<usize> for IntervalTree<T, V> {
	type Output = (Interval<T>, V);

	fn index(&self, i: usize) -> &Self::Output {
		&self.intervals[i]
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

	pub fn complement(intervals: &mut [Interval<T>]) -> Vec<Interval<T>> {
		intervals.sort_unstable();

		let mut complement: Vec<Interval<T>> = Vec::new();

		let mut pos: T = T::MIN;
		for &Interval { start, end } in intervals.iter() {
			if pos < start {
				complement.push(Interval::new(pos, start.down()));
			}
			if end < T::MAX {
				pos = end.up();
			} else {
				return complement;
			}
		}

		if pos < T::MAX {
			complement.push(Interval::new(pos, T::MAX));
		}

		complement
	}

	fn intersection(&self, other: &Self) -> Intersection<T> {
		let (first, second, lower_is): (&Self, &Self, Side) = if self <= other {
			(self, other, Side::Left)
		} else {
			(other, self, Side::Right)
		};
		if first.end < second.start {
			return Intersection::Disjoint { lower_is };
		}
		match (self.start.cmp(&other.start), self.end.cmp(&other.end)) {
			(Ordering::Equal, Ordering::Equal) => Intersection::Same,
			(Ordering::Less, Ordering::Greater) | (Ordering::Greater, Ordering::Less) => {
				Intersection::ContainsNeighbourhood {
					lower: Interval::new(first.start, second.start.down()),
					overlap: *second,
					upper: Interval::new(second.end.up(), first.end),
					lower_is,
				}
			},
			(Ordering::Equal, _) => Intersection::SameStart {
				overlap: *first,
				upper: Interval::new(first.end.up(), second.end),
				lower_is,
			},
			(_, Ordering::Equal) => Intersection::SameEnd {
				lower: Interval::new(first.start, second.start.down()),
				overlap: *second,
				lower_is,
			},
			(Ordering::Less, Ordering::Less) | (Ordering::Greater, Ordering::Greater) => Intersection::Partial {
				lower: Interval::new(first.start, second.start.down()),
				overlap: Interval::new(second.start, first.end),
				upper: Interval::new(first.end.up(), second.end()),
				lower_is,
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
				self + 1
			}

			fn down(&self) -> Self {
				self - 1
			}
		}
	};
}

number_impl!(u8, u16, u32, u64, usize);
number_impl!(i8, i16, i32, i64, isize);

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn stuff() {
		{
			let mut tree: IntervalTree<u32, u64> = IntervalTree::new();
			tree.insert(Interval::new(0, 10), 1, merge);
			tree.insert(Interval::new(5, 15), 2, merge);
			tree.insert(Interval::new(15, 15), 3, merge);
			assert_eq!(tree.len(), 4);
			assert_eq!(tree[0].0, Interval::new(0, 4));
			assert_eq!(tree[0].1, 1);
			assert_eq!(tree[1].0, Interval::new(5, 10));
			assert_eq!(tree[1].1, 3);
			assert_eq!(tree[2].0, Interval::new(11, 14));
			assert_eq!(tree[2].1, 2);
			assert_eq!(tree[3].0, Interval::new(15, 15));
			assert_eq!(tree[3].1, 5);
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
	fn complement() {
		{
			let intervals: &mut [Interval<u32>] = &mut [
				Interval { start: 10, end: 15 },
				Interval { start: 20, end: 30 },
				Interval { start: 25, end: 40 },
			];
			let complement: Vec<Interval<u32>> = Interval::complement(intervals);
			assert_eq!(complement.len(), 3);
			assert_eq!(complement[0], Interval { start: 0, end: 9 });
			assert_eq!(complement[1], Interval { start: 16, end: 19 });
			assert_eq!(
				complement[2],
				Interval {
					start: 41,
					end: u32::MAX,
				}
			);
		}
	}

	fn merge(x: &u64, y: &u64) -> u64 {
		x + y
	}
}
