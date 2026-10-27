use std::cell::Cell;

use crate::search::decompose::memo::Memo;

#[test]
fn computes_once_per_key() {
	let memo: Memo<String, usize> = Memo::new();
	let calls: Cell<usize> = Cell::new(0);
	let compute = |value: usize| {
		calls.set(calls.get() + 1);
		value
	};

	assert!(memo.is_empty());
	assert_eq!(1, memo.get_or_insert_with("a", || compute(1)));
	assert_eq!(1, memo.get_or_insert_with("a", || compute(2)));
	assert_eq!(2, memo.get_or_insert_with("b", || compute(2)));
	assert_eq!(2, calls.get());
	assert_eq!(2, memo.len());

	memo.clear();
	assert!(memo.is_empty());
	assert_eq!(3, memo.get_or_insert_with("a", || compute(3)));
}

#[test]
fn clones_are_independent() {
	let memo: Memo<String, usize> = Memo::new();
	memo.get_or_insert_with("a", || 1);

	let cloned: Memo<String, usize> = memo.clone();
	cloned.get_or_insert_with("b", || 2);

	assert_eq!(1, memo.len());
	assert_eq!(2, cloned.len());
	assert_eq!(1, cloned.get_or_insert_with("a", || 0));
}
