//! Truncating a shape's automaton must not change what the engine reports.
//!
//! Real log shapes carry very long tails of static text while their variables cluster near the
//! front,
//! and [`log_surgeon::parsing_spec::ParsingSpec::automata_for_fragments`] emits a state per
//! literal character.
//! The intersection runs to the end of whatever shape it is given,
//! so an unanchored query's trailing wildcard would traverse every one of those states --
//! which is why the shape is cut down first,
//! to the last part
//! [`log_surgeon::search::decompose::PlacementTable::last_reachable_part`] says the query's text
//! can reach.
//!
//! This is the division of labour the design rests on:
//! *how much shape matters* is decided by placement,
//! which knows where the query's runs can sit,
//! rather than by letting the automaton stop early and guess.
//!
//! These tests pin the *optimization* rather than the semantics:
//! the truncated automaton and the full one must produce the same interpretations.

use log_surgeon::nfa::Tnfa;
use log_surgeon::parsing_spec::LogShapeFragment;
use log_surgeon::parsing_spec::ParsingSpec;
use log_surgeon::parsing_spec::ParsingSpecBuilder;
use log_surgeon::search::Interpretation;
use log_surgeon::search::SearchString;
use log_surgeon::search::decompose::PlacementTable;
use log_surgeon::search::decompose::Query;
use log_surgeon::search::decompose::RunFitCache;
use log_surgeon::search::decompose::ShapeModel;

fn test_spec() -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for (name, pattern) in [
		("level", "TRACE|DEBUG|INFO|WARN|ERROR|FATAL"),
		("word", "[a-zA-Z]+"),
		("digits", "[0-9]+"),
		("blockID", r"blk_(?<num>[0-9]+)_(?<gen>[0-9]+)"),
	] {
		builder.add_rule(name, pattern).unwrap();
	}
	builder.build()
}

fn render(interpretation: &Interpretation) -> String {
	interpretation
		.leaf_queries
		.iter()
		.map(|sq| {
			if !sq.is_static_text() {
				format!("<{}={}>", sq.fully_qualified_name, sq.string_value)
			} else {
				format!("'{}'", sq.string_value)
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}

/// The last shape part `query`'s literal text can reach in `model`.
fn last_reachable_for(
	spec: &ParsingSpec,
	model: &ShapeModel,
	query: &SearchString,
) -> Option<usize> {
	let fits: RunFitCache = RunFitCache::new();
	let table: PlacementTable = PlacementTable::compute(spec, model, &Query::new(query), &fits)?;
	table.last_reachable_part()
}

/// A shape whose variables sit at the front, followed by a long static tail --
/// the corpus's shape.
fn long_tailed_shape(tail_length: usize) -> String {
	format!("%level% %word%: {}", "x".repeat(tail_length))
}

#[test]
fn unreachable_tail_is_excluded() {
	let spec: ParsingSpec = test_spec();
	let shape: String = long_tailed_shape(4_096);
	let model: ShapeModel = ShapeModel::new(&spec, &shape);

	// `INFO` can only be produced by the leading `%level%`, so the reach ends well before the tail.
	let query: SearchString = SearchString::parse("*INFO*").unwrap();
	let end: usize = last_reachable_for(&spec, &model, &query).expect("a placement exists");
	assert!(
		end < (model.parts.len() - 1),
		"the tail must be unreachable: reached {end} of {} parts",
		model.parts.len(),
	);

	// A run that genuinely reaches into the tail must keep it.
	let query: SearchString = SearchString::parse("*xxx*").unwrap();
	let end: usize = last_reachable_for(&spec, &model, &query).expect("a placement exists");
	assert_eq!(
		model.parts.len() - 1,
		end,
		"a run matching the tail must include it"
	);
}

/// Only the *last* run bounds the reach, since a composition lays its runs down left to right.
///
/// An earlier run may well be placeable late in the shape,
/// but never in a composition that also places the runs following it,
/// so taking the union over all runs would truncate far less than is sound.
#[test]
fn reach_is_bounded_by_the_last_run_alone() {
	let spec: ParsingSpec = test_spec();
	// `x` appears in the tail, so on its own the first run could sit arbitrarily late.
	let shape: String = long_tailed_shape(256);
	let model: ShapeModel = ShapeModel::new(&spec, &shape);

	// `*x*INFO*`: the last run is `INFO`, which only the leading `%level%` can produce.
	let query: SearchString = SearchString::parse("*x*INFO*").unwrap();
	let Some(end) = last_reachable_for(&spec, &model, &query) else {
		panic!("expected a placement table");
	};
	assert!(
		end < (model.parts.len() - 1),
		"the last run pins the reach to the front: reached {end} of {} parts",
		model.parts.len(),
	);
}

/// The truncated automaton must agree with the full one, interpretation for interpretation.
#[test]
fn truncation_is_transparent() {
	let spec: ParsingSpec = test_spec();

	let shapes: &[&str] = &[
		"%level% %word%: hello world",
		"%blockID.num% received from %word%",
		"a%word%b%digits%c trailing static text here",
		"%level%%word% xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
		"prefix %digits% middle %digits% suffix",
		"%level% %word%: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
	];

	let queries: &[&str] = &[
		"*INFO*", "*hello*", "*blk_1_2*", "*world", "a*b*c", "*received*", "prefix*", "*middle*",
		"*xxx*", "*zzz*", "*1*2*", "*from*", "*text*", "*: *",
	];

	let mut truncated_count: usize = 0;

	for shape in shapes.iter() {
		let model: ShapeModel = ShapeModel::new(&spec, shape);
		let last: usize = model.parts.len() - 1;

		for query_text in queries.iter() {
			let query: SearchString = SearchString::parse(query_text).unwrap();

			let Some(end) = last_reachable_for(&spec, &model, &query) else {
				continue;
			};
			if end >= last {
				continue;
			}
			truncated_count += 1;

			// The full automaton's answer.
			let expected: Vec<Interpretation> =
				query.interpretations_for_log_shape_via_engine(&spec, shape);

			// The truncated automaton's answer, built exactly as the search path builds it.
			let fragments: Vec<LogShapeFragment> = model.fragments_in(0, end);
			let truncated: Tnfa = spec.automata_for_fragments(&fragments).unwrap();
			let actual: Vec<Interpretation> =
				query.interpretations_for_automata(&spec, &truncated, &model.parts[(end + 1)..]);

			assert_eq!(
				expected.iter().map(render).collect::<Vec<_>>(),
				actual.iter().map(render).collect::<Vec<_>>(),
				"truncation changed the result: \
			 shape={shape:?} query={query_text:?} kept 0..={end} of {last}"
			);
		}
	}

	println!("checked {truncated_count} truncated (shape, query) pairs");
	assert!(
		0 < truncated_count,
		"expected some pair to actually truncate"
	);
}

/// Truncation must make the long-tail case cheap, not merely correct.
#[test]
fn long_tail_is_truncated_away() {
	let spec: ParsingSpec = test_spec();
	let shape: String = long_tailed_shape(20_000);
	let model: ShapeModel = ShapeModel::new(&spec, &shape);

	let query: SearchString = SearchString::parse("*INFO*").unwrap();
	let end: usize = last_reachable_for(&spec, &model, &query).expect("a placement exists");

	let full: Tnfa = spec.automata_for_shape(&shape).unwrap();
	let fragments: Vec<LogShapeFragment> = model.fragments_in(0, end);
	let truncated: Tnfa = spec.automata_for_fragments(&fragments).unwrap();

	println!(
		"full shape automaton: {} states, truncated: {} states",
		full.states.len(),
		truncated.states.len()
	);

	assert!(
		full.states.len() > 20_000,
		"expected the full shape to be large"
	);
	assert!(
		truncated.states.len() < 100,
		"expected truncation to drop the tail, got {} states",
		truncated.states.len()
	);

	// And the answer must still be the same.
	let expected: Vec<Interpretation> =
		query.interpretations_for_log_shape_via_engine(&spec, &shape);
	let actual: Vec<Interpretation> =
		query.interpretations_for_automata(&spec, &truncated, &model.parts[(end + 1)..]);
	assert_eq!(
		expected.iter().map(render).collect::<Vec<_>>(),
		actual.iter().map(render).collect::<Vec<_>>(),
	);
}
