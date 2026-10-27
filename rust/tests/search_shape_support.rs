//! Which shapes `search_by_log_shapes` supports, and what it does with the rest.
//!
//! A shape is **supported** when every variable names a rule the spec defines and that rule is a
//! leaf.
//! Unsupported shapes are a programming error, not a condition to recover from,
//! so the search panics with a message naming the shape and rule
//! rather than silently answering differently from the engine.
//!
//! A shape with *no* variables (pure static text) is supported:
//! its text is exactly what a query must reproduce,
//! which placement and composition already handle.

use log_surgeon::parsing_spec::ParsingSpec;
use log_surgeon::parsing_spec::ParsingSpecBuilder;
use log_surgeon::search::Interpretation;
use log_surgeon::search::SearchString;

fn test_spec() -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for (name, pattern) in [
		("word", "[a-zA-Z]+"),
		("digits", "[0-9]+"),
		("blockID", r"blk_(?<num>[0-9]+)_(?<gen>[0-9]+)"),
	] {
		builder.add_rule(name, pattern).unwrap();
	}
	builder.build()
}

fn search(spec: &ParsingSpec, query: &str, shape: &str) -> Vec<Interpretation> {
	SearchString::parse(query)
		.unwrap()
		.search_by_log_shapes(spec, &[shape])
		.pop()
		.expect("one result per shape")
}

/// A pure-static shape must be answered, not deferred to the engine.
#[test]
fn pure_static_shapes_are_supported() {
	let spec: ParsingSpec = test_spec();

	// The whole message is exactly the shape.
	assert!(!search(&spec, "abc", "abc").is_empty());
	assert!(
		search(&spec, "ab", "abc").is_empty(),
		"`ab` is not the whole of `abc`"
	);

	// Prefix / suffix / infix, with no rule to attribute anything to.
	assert!(!search(&spec, "ab*", "abc").is_empty());
	assert!(!search(&spec, "*bc", "abc").is_empty());
	assert!(!search(&spec, "*b*", "abc").is_empty());
	assert!(search(&spec, "*z*", "abc").is_empty());

	// A multi-part static shape.
	let shape: &str = "a static shape with no rules";
	assert!(!search(&spec, "*static*", shape).is_empty());
	assert!(
		!search(&spec, "*no rules", shape).is_empty(),
		"end-anchored"
	);
	assert!(
		!search(&spec, "a static*", shape).is_empty(),
		"start-anchored"
	);
	assert!(search(&spec, "*missing*", shape).is_empty());
}

/// Pure-static shapes must agree with the engine, like any other shape.
#[test]
fn pure_static_matches_the_engine() {
	let spec: ParsingSpec = test_spec();
	let shapes: &[&str] = &["abc", "plain literal text", "12345", "a b c d"];

	let queries: &[&str] = &[
		"*a*", "*abc", "abc", "abc*", "*text", "*literal*", "plain*", "*d", "*z*", "*1*3*",
		"*b c*", "a*c",
	];

	for shape in shapes.iter() {
		for query_text in queries.iter() {
			let query: SearchString = SearchString::parse(query_text).unwrap();

			let actual: Vec<Interpretation> = query
				.search_by_log_shapes(&spec, &[shape])
				.pop()
				.expect("one result per shape");
			let expected: Vec<Interpretation> =
				query.interpretations_for_log_shape_via_engine(&spec, shape);

			// Compared with wildcards stripped:
			// the engine pins some values more tightly (`a*`) than
			// composition leaves them (`*a`),
			// and both describe the same decomposition.
			// This mirrors the comparison in `decompose_differential.rs`.
			let rendered = |xs: &[Interpretation]| {
				xs.iter()
					.map(|x| {
						x.leaf_queries
							.iter()
							.map(|sq| sq.string_value.replace('*', ""))
							.collect::<Vec<_>>()
							.join(" ")
					})
					.collect::<Vec<_>>()
			};
			assert_eq!(
				rendered(&expected)
					.iter()
					.collect::<std::collections::BTreeSet<_>>(),
				rendered(&actual)
					.iter()
					.collect::<std::collections::BTreeSet<_>>(),
				"pure-static shape disagrees with the engine: shape={shape:?} query={query_text:?}"
			);
		}
	}
}

/// A leaf capture of a rule that has nested captures is supported.
#[test]
fn leaf_capture_reference_is_supported() {
	let spec: ParsingSpec = test_spec();
	let results: Vec<Interpretation> = search(&spec, "*123*", "%blockID.num%");
	assert!(!results.is_empty(), "the leaf capture can hold `123`");
	assert!(
		results.iter().any(|r| r
			.leaf_queries
			.iter()
			.any(|sq| &*sq.fully_qualified_name == "blockID.num")),
		"expected a `blockID.num` capture, got {results:?}"
	);
}

/// A variable naming a rule with nested captures is unsupported:
/// it would have to report the inner captures,
/// which the decomposition does not carry.
#[test]
#[should_panic(expected = "references non-leaf rule")]
fn non_leaf_variable_panics() {
	let spec: ParsingSpec = test_spec();
	let _ = search(&spec, "*123*", "%blockID%");
}

/// `search_by_name` is a *different* entry point and has no such restriction:
/// it searches a rule by name directly
/// and reports the nested captures the engine finds.
/// Only the log-shape decomposition needs leaf references.
#[test]
fn non_leaf_name_is_still_supported_by_search_by_name() {
	let spec: ParsingSpec = test_spec();
	let query: SearchString = SearchString::parse("*123*").unwrap();

	let by_name: Vec<Interpretation> = query.search_by_name(&spec, "blockID");
	assert!(
		!by_name.is_empty(),
		"the whole `blockID` rule can be searched by name"
	);

	let names: std::collections::BTreeSet<String> = by_name
		.iter()
		.flat_map(|interpretation| interpretation.leaf_queries.iter())
		.map(|leaf_query| leaf_query.fully_qualified_name.to_string())
		.collect();
	assert!(
		names.contains("blockID.num") || names.contains("blockID.gen"),
		"expected the rule's nested captures to be reported, got {names:?}"
	);

	// The leaf capture can also be searched by name, but that is not required here.
	assert!(!query.search_by_name(&spec, "blockID.num").is_empty());
}

/// A variable naming a rule the spec does not define is unsupported.
#[test]
#[should_panic(expected = "references undefined rule")]
fn undefined_variable_panics() {
	let spec: ParsingSpec = test_spec();
	let _ = search(&spec, "*x*", "%nonexistent%");
}

/// An unsupported shape must fail the whole call even when other shapes are valid,
/// and must do so before any results are produced -- not part-way through the list.
#[test]
#[should_panic(expected = "references non-leaf rule")]
fn unsupported_shape_fails_the_whole_call_early() {
	let spec: ParsingSpec = test_spec();
	let query: SearchString = SearchString::parse("*a*").unwrap();
	// The first two shapes are valid; the third is not.
	let _ = query.search_by_log_shapes(&spec, &["%word%", "%digits%", "%blockID%"]);
}
