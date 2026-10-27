//! A thread-safe get-or-compute memo, shared by the decomposer's caches.
//!
//! Uses interior mutability so a memo can sit behind a shared reference on a long-lived owner
//! such as [`crate::parsing_spec::ParsingSpec`], and so that filling it does not require `&mut`
//! on the search path.

#[cfg(test)]
mod test;

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// A map from `K` to computed `V`, filled on demand.
///
/// The lock is held only to look up and to insert, never while computing,
/// so a slow computation does not block other keys.
/// Two threads racing on the same missing key may both compute it;
/// that is wasted work but not a correctness problem,
/// and is cheaper than holding the lock across the computation.
/// So `compute` must be a pure function of the key.
#[derive(Debug)]
pub struct Memo<K, V> {
	entries: Mutex<BTreeMap<K, V>>,
}

impl<K, V> Memo<K, V> {
	/// An empty memo, as a `const` so it can initialise a `static`.
	#[must_use]
	pub const fn new() -> Self {
		Self {
			entries: Mutex::new(BTreeMap::new()),
		}
	}
}

impl<K: Ord, V: Clone> Memo<K, V> {
	/// The value for `key`, computing and storing it on a miss.
	///
	/// Looked up by reference, so a hit allocates nothing;
	/// the key is copied into a `K` only on a miss.
	pub fn get_or_insert_with<Q>(&self, key: &Q, compute: impl FnOnce() -> V) -> V
	where
		K: Borrow<Q> + From<Q::Owned>,
		Q: Ord + ToOwned + ?Sized,
	{
		if let Some(cached) = self.entries.lock().unwrap().get(key) {
			return cached.clone();
		}

		let value: V = compute();

		self.entries
			.lock()
			.unwrap()
			.insert(K::from(key.to_owned()), value.clone());

		value
	}

	/// The number of stored entries.
	#[must_use]
	pub fn len(&self) -> usize {
		self.entries.lock().unwrap().len()
	}

	/// Whether nothing is stored yet.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.entries.lock().unwrap().is_empty()
	}

	/// Drops every stored entry.
	pub fn clear(&self) {
		self.entries.lock().unwrap().clear();
	}
}

impl<K, V> Default for Memo<K, V> {
	fn default() -> Self {
		Self::new()
	}
}

impl<K: Clone, V: Clone> Clone for Memo<K, V> {
	/// Clones the stored entries; the clone is independent of the original from then on.
	fn clone(&self) -> Self {
		Self {
			entries: Mutex::new(self.entries.lock().unwrap().clone()),
		}
	}
}
