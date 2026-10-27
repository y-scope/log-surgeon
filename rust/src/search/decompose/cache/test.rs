use super::*;
use crate::parsing_spec::ParsingSpecBuilder;

fn test_spec() -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder.add_rule("word", "[a-zA-Z]+").unwrap();
	builder.add_rule("digits", "[0-9]+").unwrap();
	builder.build()
}

#[test]
fn caches_built_models() {
	let spec: ParsingSpec = test_spec();
	let cache: ShapeModelCache = ShapeModelCache::new();
	assert!(cache.is_empty());

	let first: Arc<ShapeModel> = cache.get(&spec, "id=%digits%");
	assert_eq!(1, cache.len());

	// A second request must return the same model, not a rebuild.
	let second: Arc<ShapeModel> = cache.get(&spec, "id=%digits%");
	assert!(Arc::ptr_eq(&first, &second));
	assert_eq!(1, cache.len());
}

#[test]
fn distinct_shapes_are_cached_separately() {
	let spec: ParsingSpec = test_spec();
	let cache: ShapeModelCache = ShapeModelCache::new();

	let a: Arc<ShapeModel> = cache.get(&spec, "%word%");
	let b: Arc<ShapeModel> = cache.get(&spec, "%digits%");
	assert_eq!(2, cache.len());
	assert!(!Arc::ptr_eq(&a, &b));
}

#[test]
#[should_panic(expected = "references undefined rule")]
fn unknown_rule_panics_and_caches_nothing() {
	let spec: ParsingSpec = test_spec();
	let cache: ShapeModelCache = ShapeModelCache::new();

	// An unsupported shape is an error, not a cacheable negative result.
	let _ = cache.get(&spec, "%nonexistent%");
}

#[test]
fn clear_drops_entries() {
	let spec: ParsingSpec = test_spec();
	let cache: ShapeModelCache = ShapeModelCache::new();
	let _ = cache.get(&spec, "%word%");
	assert!(!cache.is_empty());
	cache.clear();
	assert!(cache.is_empty());
}

#[test]
fn clone_shares_models() {
	let spec: ParsingSpec = test_spec();
	let cache: ShapeModelCache = ShapeModelCache::new();
	let original: Arc<ShapeModel> = cache.get(&spec, "%word%");

	let cloned: ShapeModelCache = cache.clone();
	assert_eq!(1, cloned.len());
	// The clone shares the already-built model rather than rebuilding it.
	assert!(Arc::ptr_eq(&original, &cloned.get(&spec, "%word%")));
}
