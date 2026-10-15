//! [`log_surgeon::search::decompose`] must never contradict the engine.
//!
//! [`log_surgeon::search::SearchString::search_by_log_shapes`] answers from the cheap
//! [`log_surgeon::search::decompose`] model,
//! instead of building and intersecting automata.
//! These tests pin the two against each other over a sweep of (shape, query) pairs:
//!
//! - **Soundness**: a shape the rejection tier discards must produce no engine match,
//!   so filtering can never drop a real result.
//!   This is the property the search path depends on.
//! - **Transparency**: the public entry point must return exactly what the engine alone would.
//! - **No tighter than the engine**: where both report the same decomposition,
//!   every capture value composition reports must glob-contain the engine's,
//!   so tightening a capture's padding never drops a match.
//! - **Satisfiability**: a static sub-query's value must actually be matchable by the shape's
//!   static text.
//!   This is checked separately because the structural comparison strips wildcards,
//!   and so is blind to a value that names the right characters in an impossible arrangement.

use std::collections::BTreeSet;

use log_surgeon::parsing_spec::ParsingSpec;
use log_surgeon::parsing_spec::ParsingSpecBuilder;
use log_surgeon::search::Interpretation;
use log_surgeon::search::SearchString;
use log_surgeon::search::decompose::ShapeModel;
use log_surgeon::search::decompose::can_match;

fn test_spec() -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for (name, pattern) in [
		("timestamp", r"[0-9]{4}-[0-9]{2}-[0-9]{2}"),
		("level", "TRACE|DEBUG|INFO|WARN|ERROR|FATAL"),
		(
			"ipv4Addr",
			r"[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}",
		),
		("path", r"/[a-zA-Z0-9_./\-]+"),
		("word", "[a-zA-Z]+"),
		("digits", "[0-9]+"),
		("blockID", r"blk_(?<num>[0-9]+)_(?<gen>[0-9]+)"),
		// A *nullable capture*: the rule itself cannot match empty,
		// (the spec builder rejects that),
		// but `optional.pad` can.
		// Referenced as `%optional.pad%` it is a variable that can stand aside,
		// which is what makes anchoring interesting --
		// a run can be last in the message without being in the last shape part.
		("optional", r"<(?<pad>[!?]*)>"),
	] {
		builder.add_rule(name, pattern).unwrap();
	}
	builder.build()
}

const SHAPES: &[&str] = &[
	"%timestamp% %level% %word%",
	"%level% %ipv4Addr%:%digits%",
	"%word% %path%",
	"abc %level% xyz",
	"hello %word% world",
	"%word%",
	"plain literal text",
	// Pure-static shapes: no variables at all,
	// so decomposition must come entirely from the static text.
	// These previously went to the engine.
	"a static shape with no rules",
	"12345",
	"%digits%-%digits%",
	"id=%digits%",
	// A leaf capture of a rule that itself has nested captures;
	// the leaf reference is supported, the whole rule would not be.
	"%blockID.num%",
	"a%word%b%digits%c",
	"%level%%word%",
	"100%% of %digits%",
	// Trailing nullable variable: an end-anchored query can finish before it.
	"%word%%optional.pad%",
	"id=%digits%%optional.pad%",
	// Leading nullable variable: a start-anchored query can begin after it.
	"%optional.pad%%word%",
	// Repeated characters: a run can occur at *overlapping* offsets in one static part,
	// each a distinct placement.
	"aaa",
	"---",
	// Nullable variable in the *middle* of a run: the run can pass through it,
	// with the variable pinned to the empty string.
	"a%optional.pad%b",
	"%word%%optional.pad%b",
	"%word%%optional.pad% tail",
];

/// Queries chosen to exercise literal/wildcard interleavings,
/// plus a deterministic pseudo-random sweep.
fn queries() -> Vec<String> {
	let mut queries: Vec<String> = [
		"*run*",
		"*INFO*",
		"*ERROR*",
		"*123*",
		"*abc*",
		"*zzz*",
		"*.*",
		"*/*",
		"*_*",
		"*%*",
		"*a*e*r*",
		"*blk*123*",
		"INFO*",
		"*xyz",
		"hello*",
		"*world",
		"a*b*c",
		"*-*",
		"id=1*",
		"*:*",
		"plain*",
		"*literal*",
		"abc*xyz",
		"*2024-01-01*",
		"*INFO*WARN*",
		"plain literal text",
		"plain literal",
		"plain*text",
		"plain",
		// End-anchored forms, and their unanchored counterparts,
		// so the two are exercised side by side.
		"*text",
		"*text*",
		"*abc",
		"*abc*",
		"*1",
		"*1*",
		"*INFO",
		"*INFO*",
		"*>",
		"*>*",
		"*!",
		"*?>",
		"a*c",
		"a*c*",
		"*a*e",
		"*a*e*",
		"id=1",
		"*_*b",
		// Overlapping occurrences in static text.
		"*aa",
		"*aa*",
		"aa*",
		"*a*aa*",
		"*--",
		// Runs crossing a nullable variable.
		"ab",
		"*ab*",
		"*xb*",
		"*hello tail",
	]
	.iter()
	.map(|q| (*q).to_owned())
	.collect::<Vec<_>>();

	let alphabet: [char; 18] = [
		'a', 'e', 'r', 'u', 'n', 'z', 'I', 'N', 'O', 'F', '1', '5', '_', '.', '/', '-', ' ', 'b',
	];
	let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
	let mut rand = |bound: usize| -> usize {
		state = state
			.wrapping_mul(6_364_136_223_846_793_005)
			.wrapping_add(1_442_695_040_888_963_407);
		((state >> 33) as usize) % bound
	};

	for _ in 0..400 {
		let mut query: String = String::new();
		if 0 != rand(4) {
			query.push('*');
		}
		for segment in 0..(1 + rand(3)) {
			if 0 != segment {
				query.push('*');
			}
			for _ in 0..(1 + rand(4)) {
				query.push(alphabet[rand(alphabet.len())]);
			}
		}
		if 0 != rand(4) {
			query.push('*');
		}
		queries.push(query);
	}

	queries
}

/// Strips wildcards, giving the fixed text a value constrains.
fn fixed_text(value: &str) -> String {
	value.replace('*', "")
}

/// Renders an interpretation as its *structure*: which rules are constrained, by which characters.
///
/// Wildcards are stripped from every value,
/// so a value the engine reports more tightly than composition does
/// (e.g. a nullable rule the engine pins to empty, which composition leaves as `*`)
/// compares equal.
/// Both describe the same decomposition --
/// the same rule instance holding the same characters --
/// and differ only in how precisely the value's interior is pinned down.
fn structure_of(interpretation: &Interpretation) -> Vec<String> {
	Vec::from_iter(
		interpretation
			.leaf_queries
			.iter()
			.filter(|leaf_query| {
				!leaf_query.is_static_text() || !fixed_text(&leaf_query.string_value).is_empty()
			})
			.map(|leaf_query| {
				if leaf_query.is_static_text() {
					format!("'{}'", fixed_text(&leaf_query.string_value))
				} else {
					format!(
						"<{}={}>",
						leaf_query.fully_qualified_name,
						fixed_text(&leaf_query.string_value)
					)
				}
			}),
	)
}

/// Renders an interpretation without its information-free static sub-queries.
///
/// A static sub-query of only wildcards constrains nothing:
/// it marks a separator the query did not mention.
/// Captures are kept whatever their value,
/// since a *vacuous capture* does carry information --
/// its position is what identifies which rule reference a later capture refers to.
fn without_vacuous_text(interpretation: &Interpretation) -> Vec<String> {
	Vec::from_iter(
		interpretation
			.leaf_queries
			.iter()
			.filter(|leaf_query| {
				!leaf_query.is_static_text() || !fixed_text(&leaf_query.string_value).is_empty()
			})
			.map(|leaf_query| {
				if !leaf_query.is_static_text() {
					format!(
						"<{}={}>",
						leaf_query.fully_qualified_name, leaf_query.string_value
					)
				} else {
					format!("'{}'", leaf_query.string_value)
				}
			}),
	)
}

#[test]
fn decompose_never_drops_a_match() {
	let spec: ParsingSpec = test_spec();

	let mut differing: usize = 0;
	let mut rejected: usize = 0;
	let mut kept: usize = 0;
	let mut checked: usize = 0;
	let mut engine_matched: usize = 0;

	for query_text in queries().iter() {
		let query: SearchString = SearchString::parse(query_text).unwrap();

		for shape in SHAPES.iter() {
			checked += 1;

			// What the public entry point returns (having consulted `decompose`).
			let actual: Vec<Interpretation> = query
				.search_by_log_shapes(&spec, &[shape])
				.pop()
				.expect("one result per shape");

			// The engine's own answer, bypassing `decompose` entirely.
			let expected: Vec<Interpretation> =
				query.interpretations_for_log_shape_via_engine(&spec, shape);
			if !expected.is_empty() {
				engine_matched += 1;
			}

			// Transparency: consulting `decompose` must not change *what the result says*.
			// Matching at all must agree exactly:
			// the composed path decides yes/no,
			// so disagreeing here would either drop a real match or invent one.
			assert_eq!(
				expected.is_empty(),
				actual.is_empty(),
				"decompose changed whether the shape matches: \
				 shape={shape:?} query={query_text:?}\n  \
				 engine={expected:?}\n  actual={actual:?}"
			);

			// The decompositions must agree on *structure*:
			// which rule instances are constrained, and by which characters.
			// Wildcard placement within a value is compared separately below,
			// since the engine tightens some values that composition leaves loose.
			let expected_set: BTreeSet<Vec<String>> =
				BTreeSet::from_iter(expected.iter().map(structure_of));
			let actual_set: BTreeSet<Vec<String>> =
				BTreeSet::from_iter(actual.iter().map(structure_of));
			assert_eq!(
				expected_set, actual_set,
				"decompose changed the decompositions: shape={shape:?} query={query_text:?}\n  \
				 engine={expected:?}\n  actual={actual:?}"
			);

			// Record where the engine is strictly tighter, so the gap stays visible and measured.
			let expected_exact: BTreeSet<Vec<String>> =
				BTreeSet::from_iter(expected.iter().map(without_vacuous_text));
			let actual_exact: BTreeSet<Vec<String>> =
				BTreeSet::from_iter(actual.iter().map(without_vacuous_text));
			if expected_exact != actual_exact {
				differing += 1;
			}

			// Never *tighter* than the engine: composition tightens a capture's padding
			// to what the rule can use, and over-tightening would silently drop matches.
			// So wherever the two agree on structure, each capture value composition reports
			// must cover (glob-contain) the engine's.
			for engine in expected.iter() {
				for composed in actual.iter() {
					if structure_of(engine) != structure_of(composed) {
						continue;
					}
					let engine_captures = engine
						.leaf_queries
						.iter()
						.filter(|leaf_query| !leaf_query.is_static_text());
					let composed_captures = composed
						.leaf_queries
						.iter()
						.filter(|leaf_query| !leaf_query.is_static_text());
					for (theirs, ours) in std::iter::zip(engine_captures, composed_captures) {
						assert!(
							glob_contains(&ours.string_value, &theirs.string_value),
							"composition is tighter than the engine: shape={shape:?} \
							 query={query_text:?} capture {} engine={:?} composed={:?}",
							ours.fully_qualified_name,
							theirs.string_value,
							ours.string_value
						);
					}
				}
			}

			// Soundness of the prefilter: rejecting must never discard a real match.
			let model: ShapeModel = ShapeModel::new(&spec, shape);
			if can_match(&model, query.as_slice()) {
				kept += 1;
			} else {
				rejected += 1;
				assert!(
					expected.is_empty(),
					"decompose rejected a matching shape: \
					 shape={shape:?} query={query_text:?}, \
					 engine found {expected:?}"
				);
			}
		}
	}

	println!(
		"checked {checked} (query, shape) pairs: {engine_matched} matched the engine, \
		 {rejected} rejected by the prefilter ({kept} kept), \
		 {differing} where the engine's values are strictly tighter"
	);
	assert!(
		0 < rejected,
		"expected the rejection tier to reject something"
	);
	assert!(
		0 < engine_matched,
		"expected some (query, shape) pair to match"
	);
}

/// Whether every string the glob `specific` matches is also matched by `general`.
///
/// Containment for `*`-only globs: `general` must match `specific` with each `*` of
/// `specific` read as one fresh symbol that only a `*` of `general` can consume.
/// Values here contain no escapes (the test alphabet has no `*` or `\\` literals).
fn glob_contains(general: &str, specific: &str) -> bool {
	const FRESH: char = '\u{0}';
	let pattern: Vec<char> = general.chars().collect::<Vec<_>>();
	let text: Vec<char> = specific
		.chars()
		.map(|c| if '*' == c { FRESH } else { c })
		.collect::<Vec<_>>();
	glob_matches(&pattern, &text)
}

/// Whether the glob `pattern` (in which `*` matches any run of characters) matches `text` entirely.
fn glob_matches(pattern: &[char], text: &[char]) -> bool {
	match pattern.first() {
		None => text.is_empty(),
		Some('*') => (0..=text.len()).any(|skip| glob_matches(&pattern[1..], &text[skip..])),
		Some(&expected) => {
			!text.is_empty() && (text[0] == expected) && glob_matches(&pattern[1..], &text[1..])
		},
	}
}

/// A static sub-query's value must be satisfiable by the text it describes.
///
/// The structural comparison in [`decompose_never_drops_a_match`] strips wildcards,
/// so it cannot see *where* they sit --
/// and a value like `'*ab'` for the text `abcdef` names exactly the right
/// characters while asserting something false, namely that the text ends in `ab`.
/// A consumer that takes the reported value at face value would find it matches nothing.
///
/// Pure-static shapes make the check unambiguous:
/// the shape has exactly one part,
/// so an interpretation's single static sub-query must glob-match the whole shape text.
#[test]
fn static_leaf_query_values_are_satisfiable() {
	let spec: ParsingSpec = test_spec();

	let shapes: &[&str] = &[
		"abcdef",
		"abc",
		"plain literal text",
		"aaa",
		"abab",
		"12345",
		"a static shape with no rules",
	];

	let mut checked: usize = 0;

	for query_text in queries().iter() {
		let query: SearchString = SearchString::parse(query_text).unwrap();

		for shape in shapes.iter() {
			let text: Vec<char> = shape.chars().collect::<Vec<_>>();

			for interpretation in query
				.search_by_log_shapes(&spec, &[shape])
				.pop()
				.expect("one result")
			{
				// A pure-static shape decomposes into exactly one static sub-query.
				assert_eq!(
					1,
					interpretation.leaf_queries.len(),
					"a pure-static shape should yield one sub-query: \
					 shape={shape:?} query={query_text:?}"
				);
				let leaf_query: &log_surgeon::search::LeafQuery = &interpretation.leaf_queries[0];
				assert!(leaf_query.is_static_text(), "expected a static sub-query");

				let pattern: Vec<char> = leaf_query.string_value.chars().collect::<Vec<_>>();
				assert!(
					glob_matches(&pattern, &text),
					"static value {:?} cannot match the shape's text {shape:?}: \
					 query={query_text:?}",
					leaf_query.string_value
				);
				checked += 1;
			}
		}
	}

	println!("checked {checked} static sub-query values for satisfiability");
	assert!(
		0 < checked,
		"expected some interpretation, or the test proves nothing"
	);
}

/// A run crossing a rule immediately followed by *another rule* has no fixed boundary to pin its
/// split, so every split must be tried.
/// A previous implementation capped those candidates at
/// `MAX_UNPINNED_SPLITS` and silently dropped the rest,
/// rejecting shapes it merely could not place.
///
/// Here `r2` must contribute exactly 9 characters as a *middle* piece --
/// more than the cap of 8 --
/// so the old code found no placement and reported the shape impossible,
/// while the engine matched.
/// The table must instead be treated as incomplete,
/// so the caller falls back to the engine.
#[test]
fn middle_split_longer_than_the_candidate_cap_still_matches() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for (name, pattern) in [("r1", "[0-9]+"), ("r2", "[a-z]{9}"), ("r3", "[0-9]+")] {
		builder.add_rule(name, pattern).unwrap();
	}
	let spec: ParsingSpec = builder.build();

	let shape: &str = "%r1%%r2%%r3%";
	let query_text: &str = "1aaaaaaaaa2";

	let query: SearchString = SearchString::parse(query_text).unwrap();
	let engine: Vec<Interpretation> = query.interpretations_for_log_shape_via_engine(&spec, shape);
	let actual: Vec<Interpretation> = query
		.search_by_log_shapes(&spec, &[shape])
		.pop()
		.expect("one result per shape");

	// The engine finds the match; `r2` takes all 9 letters.
	assert!(!engine.is_empty(), "the engine should match this shape");
	// `decompose` must not drop it.
	// An incomplete table makes it defer to the engine,
	// so the two agree exactly.
	assert_eq!(
		engine, actual,
		"decompose disagrees with the engine: shape={shape:?} query={query_text:?}"
	);
}

/// The same hazard, exercised through several shapes and query lengths around the cap,
/// so a fix that merely shifts the boundary cannot pass.
#[test]
fn adjacent_rule_chains_are_never_falsely_rejected() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for (name, pattern) in [
		("digits", "[0-9]+"),
		("letters", "[a-z]+"),
		("fixed9", "[a-z]{9}"),
		("fixed12", "[a-z]{12}"),
	] {
		builder.add_rule(name, pattern).unwrap();
	}
	let spec: ParsingSpec = builder.build();

	let shapes: &[&str] = &[
		"%digits%%letters%%digits%",
		"%digits%%fixed9%%digits%",
		"%digits%%fixed12%%digits%",
		"%letters%%digits%%letters%",
		"%digits%%letters%%digits%%letters%",
	];

	let mut checked: usize = 0;
	let mut matched: usize = 0;

	for shape in shapes.iter() {
		for letters in 1..=16 {
			let query_text: String = format!("1{}2", "a".repeat(letters));
			let query: SearchString = SearchString::parse(&query_text).unwrap();

			let engine: Vec<Interpretation> =
				query.interpretations_for_log_shape_via_engine(&spec, shape);
			let actual: Vec<Interpretation> = query
				.search_by_log_shapes(&spec, &[shape])
				.pop()
				.expect("one result per shape");

			assert_eq!(
				engine, actual,
				"decompose disagrees with the engine: shape={shape:?} query={query_text:?}"
			);

			checked += 1;
			if !engine.is_empty() {
				matched += 1;
			}
		}
	}

	println!("checked {checked} adjacent-chain queries, {matched} matched");
	assert!(
		0 < matched,
		"expected some query to match, or the test proves nothing"
	);
}

/// The engine path must agree with composition on **unanchored** queries, including the tail.
///
/// This exercises the contract that replaced the `TO_END` / `WILDCARD_END` const parameters.
/// The intersection now always runs to the end of the shape it is given,
/// so an unanchored query carries a real trailing `.*`;
/// the shape is instead cut down beforehand,
/// and whatever that cut removed has to be reported the same way the full shape would have
/// reported it.
///
/// `%r2%` is `[a-z]{9}`,
/// which makes `PlacementTable::compute` return `None` for these queries (the unpinned-split cap),
/// forcing the engine path --
/// so the comparison below is genuinely engine vs. engine-free,
/// rather than two spellings of the same code.
#[test]
fn engine_fallback_agrees_on_unanchored_queries() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for (name, pattern) in [("r1", "[0-9]+"), ("r2", "[a-z]{9}"), ("r3", "[0-9]+")] {
		builder.add_rule(name, pattern).unwrap();
	}
	let spec: ParsingSpec = builder.build();

	// Static text both before and after the rules, so a truncated tail has something to report.
	let shapes: &[&str] = &["%r1%%r2%%r3%", "pre %r1%%r2%%r3% post", "%r1%%r2%%r3% tail"];
	let queries: &[&str] = &[
		"*1aaaaaaaaa2*", "1aaaaaaaaa2*", "*1aaaaaaaaa2", "1aaaaaaaaa2", "*aaaaaaaaa*", "pre*",
		"*post", "*tail",
	];

	let mut checked: usize = 0;
	let mut matched: usize = 0;

	for shape in shapes.iter() {
		for query_text in queries.iter() {
			let query: SearchString = SearchString::parse(query_text).unwrap();

			let engine: Vec<Interpretation> =
				query.interpretations_for_log_shape_via_engine(&spec, shape);
			let actual: Vec<Interpretation> = query
				.search_by_log_shapes(&spec, &[shape])
				.pop()
				.expect("one result per shape");

			assert_eq!(
				engine.is_empty(),
				actual.is_empty(),
				"engine and decompose disagree on whether it matches: \
				 shape={shape:?} query={query_text:?}\n  \
				 engine={engine:?}\n  actual={actual:?}"
			);

			// Compared structurally, for the same reason as `decompose_never_drops_a_match`:
			// the two may pad a capture differently
			// (`*aaa*` vs `aaa`, equivalent when the rule is fixed-width)
			// while describing the same decomposition.
			// What must agree is which parts are constrained and by which characters --
			// in particular that the *number* of sub-queries matches,
			// which is what the trailing-tail handling decides.
			let expected_set: BTreeSet<Vec<String>> =
				BTreeSet::from_iter(engine.iter().map(structure_of));
			let actual_set: BTreeSet<Vec<String>> =
				BTreeSet::from_iter(actual.iter().map(structure_of));
			assert_eq!(
				expected_set, actual_set,
				"engine and decompose disagree: shape={shape:?} query={query_text:?}\n  \
				 engine={engine:?}\n  actual={actual:?}"
			);

			checked += 1;
			if !engine.is_empty() {
				matched += 1;
			}
		}
	}

	println!("checked {checked} engine-fallback queries, {matched} matched");
	assert!(
		0 < matched,
		"expected some query to match, or the test proves nothing"
	);
}
