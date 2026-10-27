//! A query is anchored at a boundary exactly when it has no wildcard there.
//!
//! The rule is uniform across both ends: `foo*` is anchored at the start,
//! `*foo` at the end, `foo` at both (an exact match), and `*foo*` at neither.
//! Both ends are expressed the same way --
//! as a wildcard the automaton actually consumes --
//! and the cost of the unanchored case is controlled by bounding how much of the shape is built,
//! not by stopping the intersection early.
//!
//! These tests pin the semantics through the public entry point,
//! so they cover `decompose`, the composition path, and the engine together.

use log_surgeon::parsing_spec::ParsingSpec;
use log_surgeon::parsing_spec::ParsingSpecBuilder;
use log_surgeon::search::Interpretation;
use log_surgeon::search::SearchString;

fn test_spec() -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for (name, pattern) in [
		("level", "TRACE|DEBUG|INFO|WARN|ERROR|FATAL"),
		("word", "[a-zA-Z]+"),
		("digits", "[0-9]+"),
		// The rule cannot match empty (the spec builder forbids that),
		// but the `pad` capture can,
		// so `%optional.pad%` is a variable that can stand aside entirely.
		("optional", r"<(?<pad>[!?]*)>"),
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

fn matches(spec: &ParsingSpec, query: &str, shape: &str) -> bool {
	!search(spec, query, shape).is_empty()
}

fn render(interpretation: &Interpretation) -> String {
	interpretation
		.leaf_queries
		.iter()
		.map(|sq| {
			if sq.is_static_text() {
				format!("'{}'", sq.string_value)
			} else {
				format!("<{}={}>", sq.fully_qualified_name, sq.string_value)
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}

/// `*foo` must require the message to *end* with `foo`,
/// the way `foo*` requires it to start with it.
#[test]
fn trailing_wildcard_decides_end_anchoring() {
	let spec: ParsingSpec = test_spec();
	let shape: &str = "%level% %word% done";

	// The shape always ends with `done`, so a query anchored to anything else cannot match.
	assert!(
		matches(&spec, "*done", shape),
		"the shape does end with `done`"
	);
	assert!(
		!matches(&spec, "*INFO", shape),
		"`*INFO` is anchored at the end, and the shape cannot end with INFO"
	);

	// The unanchored counterpart still matches: this is the behaviour `*INFO` used to have.
	assert!(matches(&spec, "*INFO*", shape), "`*INFO*` is not anchored");
}

/// `foo*` is start-anchored; the mirror of the above.
#[test]
fn leading_wildcard_decides_start_anchoring() {
	let spec: ParsingSpec = test_spec();
	let shape: &str = "start %word% end";

	assert!(
		matches(&spec, "start*", shape),
		"the shape does start with `start`"
	);
	assert!(
		!matches(&spec, "end*", shape),
		"`end*` is anchored at the start, and the shape cannot start with `end`"
	);
	assert!(matches(&spec, "*end*", shape), "`*end*` is not anchored");
}

/// A query with no wildcards at all is anchored at both ends: an exact match.
#[test]
fn bare_query_is_anchored_at_both_ends() {
	let spec: ParsingSpec = test_spec();

	// `%word%` is `[a-zA-Z]+`, so it cannot be empty:
	// `hello` alone is never a whole message.
	assert!(
		!matches(&spec, "hello", "hello %word%"),
		"`hello` cannot be the whole message: a space and a word must follow"
	);
	assert!(
		!matches(&spec, "hello", "hello%word%"),
		"`hello` cannot be the whole message: the rule must emit at least one character"
	);
	// But with the wildcard it can.
	assert!(
		matches(&spec, "hello*", "hello%word%"),
		"`hello*` only anchors the start"
	);

	// An exact literal shape matches its own text exactly.
	assert!(matches(&spec, "abc", "abc"), "an exact match");
	assert!(
		!matches(&spec, "ab", "abc"),
		"`ab` is not the whole of `abc`"
	);
	assert!(matches(&spec, "ab*", "abc"), "`ab*` is a prefix match");
}

/// An end-anchored query may finish before a trailing variable that can produce nothing.
#[test]
fn end_anchoring_allows_a_trailing_nullable_variable() {
	let spec: ParsingSpec = test_spec();
	let shape: &str = "msg=%word%%optional.pad%";

	// `pad` is `[!?]*`, so it can vanish, letting the word be the last thing in the message.
	let results: Vec<Interpretation> = search(&spec, "*hello", shape);
	assert!(
		!results.is_empty(),
		"the trailing variable can be empty, so the message can end with `hello`"
	);

	// And the engine reports that precisely, as an *empty* capture rather than a `*`.
	let rendered: Vec<String> = results.iter().map(render).collect::<Vec<_>>();
	assert!(
		rendered.iter().any(|r| r.contains("<optional.pad=>")),
		"expected an empty capture pinning the variable to nothing, got {rendered:?}"
	);
}

/// A start-anchored query may begin after a leading variable that can produce nothing.
#[test]
fn start_anchoring_allows_a_leading_nullable_variable() {
	let spec: ParsingSpec = test_spec();
	let shape: &str = "%optional.pad%%word% tail";

	assert!(
		matches(&spec, "hello*", shape),
		"the leading variable can be empty, so the message can start with `hello`"
	);
}

/// A non-nullable trailing variable must still emit something,
/// so the query cannot end before it.
#[test]
fn end_anchoring_rejects_a_trailing_non_nullable_variable() {
	let spec: ParsingSpec = test_spec();

	assert!(
		!matches(&spec, "*id=", "id=%digits%"),
		"`%digits%` must emit at least one character, so the message cannot end at `id=`"
	);
	assert!(
		matches(&spec, "*id=*", "id=%digits%"),
		"the unanchored form still matches"
	);
}

/// Anchoring at both ends pins a capture exactly, with no room for the rule to pad it.
#[test]
fn both_ends_anchored_pins_a_capture_exactly() {
	let spec: ParsingSpec = test_spec();
	let shape: &str = "%level%";

	// `INFO` is a whole level, so the shape can be exactly that message.
	let results: Vec<Interpretation> = search(&spec, "INFO", shape);
	assert!(!results.is_empty(), "`INFO` is a complete level");
	let rendered: Vec<String> = results.iter().map(render).collect::<Vec<_>>();
	assert!(
		rendered.iter().any(|r| r.contains("<level=INFO>")),
		"expected the capture pinned to exactly `INFO`, got {rendered:?}"
	);

	// `INF` is not a whole level, and nothing may follow it.
	assert!(
		!matches(&spec, "INF", shape),
		"no level is exactly `INF`, and both ends are anchored"
	);
	// Unanchoring the end admits it again.
	assert!(
		matches(&spec, "INF*", shape),
		"`INF*` only needs a level starting with INF"
	);
}

/// The engine always intersects to the end,
/// so an unanchored query must carry a *real* trailing wildcard rather than being allowed to
/// stop early.
///
/// This pins the contract that replaced the old `TO_END` / `WILDCARD_END` const parameters:
/// there is no longer a mode in which the intersection accepts
/// while the shape is mid-way through.
/// The observable consequence is that a trailing `*` and an anchored end give genuinely different
/// answers,
/// and that both are reported without any synthetic wildcard being appended afterwards.
#[test]
fn unanchored_queries_consume_the_rest_of_the_shape() {
	let spec: ParsingSpec = test_spec();

	// The shape continues past `id=` with a non-nullable rule,
	// so only the unanchored form matches;
	// the anchored one must not,
	// which it could not express if the engine stopped at the query's last run.
	assert!(matches(&spec, "*id=*", "id=%digits%"));
	assert!(!matches(&spec, "*id=", "id=%digits%"));

	// The wildcard has to traverse the whole tail, including several parts, and still agree with
	// `decompose` about the result.
	let shape: &str = "a%word%b%digits%c";
	for query in ["*a*", "a*", "*b*", "*c", "a*c", "*a*b*c"] {
		let parsed: SearchString = SearchString::parse(query).unwrap();
		let actual: Vec<Interpretation> = parsed
			.search_by_log_shapes(&spec, &[shape])
			.pop()
			.expect("one result per shape");
		let expected: Vec<Interpretation> =
			parsed.interpretations_for_log_shape_via_engine(&spec, shape);
		assert_eq!(
			expected.iter().map(render).collect::<Vec<_>>(),
			actual.iter().map(render).collect::<Vec<_>>(),
			"query={query:?}"
		);
	}
}

/// An unanchored query reports nothing past its last literal character.
///
/// The engine now runs to the end of the shape,
/// so it *sees* the trailing parts;
/// they must still be dropped from the result,
/// because the trailing wildcard leaves them unconstrained
/// and reporting each as a bare `*` would carry no information.
/// Positional identity is read left to right,
/// so trimming the tail cannot disturb it.
#[test]
fn trailing_unconstrained_parts_are_not_reported() {
	let spec: ParsingSpec = test_spec();

	// `%word%` is constrained by `hello`; the `%digits%` after it is not, and must not appear.
	let results: Vec<Interpretation> = search(&spec, "*hello*", "%word% %digits%");
	assert!(!results.is_empty(), "the word can be `hello`");
	for rendered in results.iter().map(render) {
		assert!(
			!rendered.contains("digits"),
			"the trailing unconstrained rule must be dropped, got {rendered:?}"
		);
	}

	// But a *leading* vacuous capture is kept: its position is what identifies the later reference.
	let results: Vec<Interpretation> = search(&spec, "*5*", "%word% %digits%");
	assert!(!results.is_empty(), "the digits can be `5`");
	assert!(
		results.iter().map(render).any(|r| r.contains("word")),
		"the leading reference must still be reported to keep positions meaningful: {:?}",
		results.iter().map(render).collect::<Vec<_>>(),
	);
}

/// `decompose` and the engine must agree about anchoring, on every path.
#[test]
fn decompose_and_engine_agree_on_anchoring() {
	let spec: ParsingSpec = test_spec();

	let shapes: &[&str] = &[
		"%level% %word% done",
		"id=%digits%",
		"msg=%word%%optional.pad%",
		"%optional.pad%%word% tail",
		"%level%",
		"a%word%b%digits%c",
		"start %word% end",
		"%word%",
	];

	let queries: &[&str] = &[
		"*done", "*done*", "done*", "done", "*INFO", "*INFO*", "INFO", "INFO*", "*id=", "*id=*",
		"id=1", "id=1*", "*hello", "*hello*", "hello*", "hello", "*end", "*end*", "*c", "*c*",
		"a*c", "a*c*", "*b*c", "*1", "*1*", "*>", "*tail", "*tail*", "*", "",
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

			assert_eq!(
				expected.is_empty(),
				actual.is_empty(),
				"decompose and engine disagree on whether it matches: \
				 shape={shape:?} query={query_text:?}\n  \
				 engine={:?}\n  actual={:?}",
				expected.iter().map(render).collect::<Vec<_>>(),
				actual.iter().map(render).collect::<Vec<_>>(),
			);
		}
	}
}

/// `**` says exactly what `*` does, so it is collapsed at parse:
/// anchoring is read off the first and last symbol, and nothing downstream sees a doubled
/// wildcard.
#[test]
fn adjacent_wildcards_collapse_at_parse() {
	let spec: ParsingSpec = test_spec();

	for (doubled, single) in [
		("**hello**", "*hello*"),
		("hello**", "hello*"),
		("**hello", "*hello"),
		("a***c", "a*c"),
		("***", "*"),
	] {
		let parsed: SearchString = SearchString::parse(doubled).unwrap();
		assert_eq!(
			single,
			parsed.to_string(),
			"{doubled:?} should parse to {single:?}"
		);
		assert_eq!(
			SearchString::parse(single).unwrap().anchored_start(),
			parsed.anchored_start()
		);
		assert_eq!(
			SearchString::parse(single).unwrap().anchored_end(),
			parsed.anchored_end()
		);

		for shape in ["hello %word%", "a%word%b%digits%c", "%word%"] {
			assert_eq!(
				search(&spec, single, shape),
				search(&spec, doubled, shape),
				"shape={shape:?}: {doubled:?} must search as {single:?}"
			);
		}
	}

	// An escaped star is a literal and is never collapsed.
	assert_eq!(r"*\**", SearchString::parse(r"**\***").unwrap().to_string());
}

/// The two degenerate queries: `*` constrains nothing, `` constrains everything.
///
/// Both have no literal runs, so they are told apart only by anchoring,
/// which is why it is derived from the query rather than from its runs.
#[test]
fn wildcard_only_and_empty_queries() {
	let spec: ParsingSpec = test_spec();

	// `*` matches every shape, with a single unconstrained interpretation.
	for shape in ["hello %word%", "%optional.pad%", "abc", "%word%"] {
		let results: Vec<Interpretation> = search(&spec, "*", shape);
		assert_eq!(
			vec!["'*'"],
			results.iter().map(render).collect::<Vec<_>>(),
			"shape={shape:?}"
		);
	}

	// `` is anchored at both ends, so it matches only a shape that can produce nothing,
	// and then pins every variable to the empty string.
	assert!(
		!matches(&spec, "", "hello %word%"),
		"static text cannot vanish"
	);
	assert!(!matches(&spec, "", "abc"));
	assert!(
		!matches(&spec, "", "%word%"),
		"`%word%` must emit a character"
	);
	let results: Vec<Interpretation> = search(&spec, "", "%optional.pad%");
	assert_eq!(
		vec!["<optional.pad=>"],
		results.iter().map(render).collect::<Vec<_>>(),
		"the nullable capture is pinned to nothing"
	);
}
