use super::*;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::search::SearchString;

fn spec_with_rules(rules: &[(&str, &str)]) -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for &(name, pattern) in rules.iter() {
		builder.add_rule(name, pattern).unwrap();
	}
	builder.build()
}

fn test_spec() -> ParsingSpec {
	spec_with_rules(&[
		("level", "TRACE|DEBUG|INFO|WARN|ERROR"),
		("digits", "[0-9]+"),
		("word", "[a-zA-Z]+"),
	])
}

/// The query's symbols, exactly as written: a trailing `*` is what makes it unanchored.
fn symbols_of(query: &str) -> Vec<SymbolicChar> {
	SearchString::parse(query).unwrap().as_slice().to_vec()
}

fn is_rejected(spec: &ParsingSpec, shape: &str, query: &str) -> bool {
	let model: ShapeModel = ShapeModel::new(spec, shape);
	!can_match(&model, &symbols_of(query))
}

#[test]
fn static_text_must_match_verbatim() {
	let spec: ParsingSpec = test_spec();
	assert!(!is_rejected(&spec, "hello %word%", "*hello*"));
	// `%word%` can emit `hxllo`'s letters,
	// so the prefilter can only reject once the run has to involve the static text.
	// Use a character no part can emit to isolate the static-text check.
	assert!(is_rejected(&spec, "hello %word%", "*h#llo*"));
}

#[test]
fn variable_charset_rejects_foreign_characters() {
	let spec: ParsingSpec = test_spec();
	// A digit-only variable cannot produce letters, and no static text supplies them either.
	assert!(is_rejected(&spec, "id=%digits%", "*id=abc*"));
	assert!(!is_rejected(&spec, "id=%digits%", "*id=123*"));
}

#[test]
fn contiguity_is_enforced_across_a_variable() {
	let spec: ParsingSpec = test_spec();
	// `a` and `b` are both producible, but `axb` requires the digit-only variable to emit `x`.
	assert!(is_rejected(&spec, "a%digits%b", "*axb*"));
	// With a digit between, it can match.
	assert!(!is_rejected(&spec, "a%digits%b", "*a1b*"));
}

#[test]
fn fixed_text_is_accepted_when_some_part_can_produce_every_piece() {
	let spec: ParsingSpec = test_spec();
	// `id=` is the shape's static text, `123` a possible capture of `digits`.
	assert!(!is_rejected(&spec, "id=%digits%", "id=123*"));
	assert!(!is_rejected(&spec, "%level% ok", "INFO ok"));
	assert!(!is_rejected(&spec, "a%word%", "abc*"));
}

#[test]
fn runs_must_match_in_order() {
	let spec: ParsingSpec = test_spec();
	// Both runs exist in the shape, but in the opposite order,
	// so no walk consumes them in order.
	// This is the key improvement over checking each run independently.
	assert!(!is_rejected(&spec, "aaa %word% zzz", "*aaa*zzz*"));
	assert!(is_rejected(&spec, "aaa %digits% zzz", "*zzz*aaa*"));
}

#[test]
fn start_anchoring_is_honoured() {
	let spec: ParsingSpec = test_spec();
	// Anchored at the start: the query must begin where the shape begins.
	assert!(!is_rejected(&spec, "hello %word%", "hello*"));
	// `ello` cannot start the shape, and `h` blocks it from starting mid-text.
	assert!(is_rejected(&spec, "hello %word%", "ello*"));
	// Unanchored, the same text is fine.
	assert!(!is_rejected(&spec, "hello %word%", "*ello*"));
}

#[test]
fn end_anchoring_requires_consuming_the_shape() {
	let spec: ParsingSpec = test_spec();
	// Anchored: trailing static text remains unconsumed, so the query cannot reach the end.
	assert!(is_rejected(&spec, "%word% tail", "*xyz"));
	// Consuming through to the end is possible.
	assert!(!is_rejected(&spec, "%word% tail", "* tail"));
	// Unanchored, the trailing wildcard consumes the remaining text.
	assert!(!is_rejected(&spec, "%word% tail", "*xyz*"));
}

#[test]
fn wildcard_only_query_is_never_rejected() {
	let spec: ParsingSpec = test_spec();
	// `*` constrains nothing.
	assert!(!is_rejected(&spec, "%word%", "*"));
	assert!(!is_rejected(&spec, "a%word%b", "*"));
}

#[test]
fn adjacent_wildcards_collapse_at_parse() {
	// Runs of wildcards collapse at parse,
	// so they cannot change what the prefilter decides.
	assert_eq!(symbols_of("*ab*"), symbols_of("**ab**"));
	assert_eq!(symbols_of("*a*b*"), symbols_of("*a***b*"));
}

#[test]
fn empty_query_requires_an_empty_message() {
	let spec: ParsingSpec = test_spec();
	// The empty query is anchored at both ends, so it matches only a message of nothing.
	// Static text cannot vanish; a variable is conservatively assumed able to.
	assert!(is_rejected(&spec, "a%word%", ""));
	assert!(!is_rejected(&spec, "%word%", ""));
}
