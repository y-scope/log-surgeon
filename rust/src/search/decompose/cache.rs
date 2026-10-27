//! A cache of [`ShapeModel`]s, keyed by log shape.
//!
//! Building a model walks the whole shape and resolves every variable against the spec.
//! Shapes are long (thousands of characters is normal),
//! and a caller typically searches the same set of shapes repeatedly,
//! so rebuilding per query dominates the cost of [`crate::search::decompose`].
//! The cache makes it a once-per-shape cost instead.
//!
//! Backed by a [`Memo`], so it can sit behind a shared reference on a long-lived owner such as
//! [`crate::parsing_spec::ParsingSpec`].

#[cfg(test)]
mod test;

use std::sync::Arc;

use crate::parsing_spec::ParsingSpec;
use crate::search::decompose::ShapeModel;
use crate::search::decompose::memo::Memo;

/// A shape-keyed cache of [`ShapeModel`]s.
///
/// [`Clone`] clones the entries, and the models are behind [`Arc`],
/// so a clone (as of a [`crate::parser::Parser`]) shares the built models
/// rather than rebuilding them.
#[derive(Clone, Debug, Default)]
pub struct ShapeModelCache {
	models: Memo<Box<str>, Arc<ShapeModel>>,
}

impl ShapeModelCache {
	/// An empty cache, as a `const` so [`crate::parsing_spec::BLANK`] can use it.
	#[must_use]
	pub const fn new() -> Self {
		Self {
			models: Memo::new(),
		}
	}

	/// The model for `shape`, building and caching it on a miss.
	///
	/// # Panics
	///
	/// Panics if the shape is unsupported; see [`ShapeModel::new`].
	#[must_use]
	pub fn get(&self, spec: &ParsingSpec, shape: &str) -> Arc<ShapeModel> {
		self.models
			.get_or_insert_with(shape, || Arc::new(ShapeModel::new(spec, shape)))
	}

	/// The number of cached shapes.
	#[must_use]
	pub fn len(&self) -> usize {
		self.models.len()
	}

	/// Whether nothing is cached yet.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.models.is_empty()
	}

	/// Drops every cached model.
	pub fn clear(&self) {
		self.models.clear();
	}
}
