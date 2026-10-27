use super::*;
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
		("digits", "[0-9]+"),
		("word", "[a-z]+"),
		("level", "INFO|WARN|ERROR"),
	])
}

/// The query as the decomposer sees it: symbols, runs, and anchoring derived once.
fn decomposed(query: &str) -> (SearchString, Vec<Run>) {
	let parsed: SearchString = SearchString::parse(query).unwrap();
	let runs: Vec<Run> = Query::new(&parsed).runs;
	(parsed, runs)
}

fn table_for(
	spec: &ParsingSpec,
	shape: &str,
	query: &str,
) -> (ShapeModel, Vec<Run>, PlacementTable) {
	let model: ShapeModel = ShapeModel::new(spec, shape);
	let (parsed, _) = decomposed(query);
	let fits: RunFitCache = RunFitCache::new();
	let table: PlacementTable =
		PlacementTable::compute(spec, &model, &Query::new(&parsed), &fits).unwrap();
	(model, Query::new(&parsed).runs, table)
}

/// Renders a placement as `part:text` pieces, marking rules with `<>`.
fn render(placement: &Placement) -> String {
	placement
		.pieces
		.iter()
		.map(|piece| {
			if piece.is_rule {
				format!("{}:<{}>", piece.part, piece.text)
			} else {
				format!("{}:'{}'", piece.part, piece.text)
			}
		})
		.collect::<Vec<_>>()
		.join("+")
}

fn rendered(table: &PlacementTable, run: usize) -> Vec<String> {
	let mut out: Vec<String> = table.placements[run].iter().map(render).collect::<Vec<_>>();
	out.sort();
	out.dedup();
	out
}

#[test]
fn splits_a_query_into_runs() {
	let (_, runs) = decomposed("*abc*def*");
	assert_eq!(2, runs.len());
	assert_eq!("abc", runs[0].text);
	assert_eq!("def", runs[1].text);
	assert!(!runs[0].anchored_start);

	// A query with no leading wildcard anchors its first run.
	let (_, runs) = decomposed("abc*");
	assert_eq!(1, runs.len());
	assert!(runs[0].anchored_start);

	// A query with no trailing wildcard anchors its last run.
	let (_, runs) = decomposed("*abc");
	assert!(runs[0].anchored_end);

	// Adjacent wildcards collapse at parse, so they neither add runs nor change anchoring.
	let (_, runs) = decomposed("**abc**");
	assert_eq!(1, runs.len());
	assert!(!runs[0].anchored_start);
	assert!(!runs[0].anchored_end);

	// A wildcard-only query has no runs at all, so it constrains nothing;
	// so does the empty query, which constrains everything -- the runs cannot tell them apart,
	// which is why anchoring is stored on the [`Query`], not on its runs.
	assert!(decomposed("*").1.is_empty());
	assert!(decomposed("").1.is_empty());
}

#[test]
fn run_placed_in_static_text() {
	let spec: ParsingSpec = test_spec();
	// `id=` is static text in part 0.
	let (_, _, table) = table_for(&spec, "id=%digits%", "*id=*");
	assert_eq!(vec!["0:'id='"], rendered(&table, 0));
}

#[test]
fn run_placed_inside_a_rule() {
	let spec: ParsingSpec = test_spec();
	let (_, _, table) = table_for(&spec, "id=%digits%", "*123*");
	// Part 1 is the rule; the run sits wholly inside it.
	assert_eq!(vec!["1:<123>"], rendered(&table, 0));
}

#[test]
fn run_straddling_a_rule_and_following_text() {
	let spec: ParsingSpec = test_spec();
	// `12y`: `12` from the digits rule, `y` from the static text after it.
	let (_, _, table) = table_for(&spec, "x%digits%y", "*12y*");
	assert!(
		rendered(&table, 0).contains(&"1:<12>+2:'y'".to_owned()),
		"got {:?}",
		rendered(&table, 0)
	);
}

#[test]
fn run_straddling_text_then_rule_is_found_from_the_text_side() {
	let spec: ParsingSpec = test_spec();
	// `x12` has `x` in static text and `12` in the rule.
	// The straddle search starts from a rule,
	// so this direction is represented by the *rule* supplying a trailing piece;
	// check the run is placeable.
	let (model, _, table) = table_for(&spec, "x%digits%y", "*x12*");
	assert!(
		!table.is_impossible(),
		"`x12` must be placeable in `x%digits%y`"
	);
	assert!(can_compose(&table, model.parts.len()));
}

#[test]
fn positional_identity_distinguishes_two_instances_of_one_rule() {
	let spec: ParsingSpec = test_spec();
	// The key requirement: `A%foo%B%foo%C` has two `foo` instances,
	// and a placement must say *which*.
	let (_, _, table) = table_for(&spec, "A%word%B%word%C", "*qq*");
	let placements: Vec<String> = rendered(&table, 0);
	// Parts 1 and 3 are the two rule instances;
	// both are valid placements and are reported distinctly.
	assert!(
		placements.contains(&"1:<qq>".to_owned()),
		"got {placements:?}"
	);
	assert!(
		placements.contains(&"3:<qq>".to_owned()),
		"got {placements:?}"
	);
	assert_eq!(2, placements.len());
}

#[test]
fn run_that_fits_nowhere_is_impossible() {
	let spec: ParsingSpec = test_spec();
	// `#` appears in no static text and no rule's language.
	let (_, _, table) = table_for(&spec, "id=%digits%", "*#*");
	assert!(
		table.is_impossible(),
		"a run that fits nowhere proves no match"
	);
	assert!(!can_compose(&table, 2));
}

#[test]
fn runs_must_compose_in_order() {
	let spec: ParsingSpec = test_spec();

	// `A` then `B` is in order and composes.
	let (model, _, table) = table_for(&spec, "A%word%B%word%C", "*A*B*");
	assert!(!table.is_impossible());
	assert!(can_compose(&table, model.parts.len()));

	// `B` then `A` is not: each run is placeable alone, but not in that order.
	let (model, _, table) = table_for(&spec, "A%digits%B%digits%C", "*B*A*");
	assert!(!table.is_impossible(), "each run is individually placeable");
	assert!(
		!can_compose(&table, model.parts.len()),
		"but they cannot be placed left to right"
	);
}

#[test]
fn two_runs_may_share_one_rule() {
	let spec: ParsingSpec = test_spec();
	// `*1*2*` can both sit inside the same digits rule,
	// since a rule can emit text between the runs.
	let (model, _, table) = table_for(&spec, "id=%digits%", "*1*2*");
	assert!(!table.is_impossible());
	assert!(
		can_compose(&table, model.parts.len()),
		"a rule can hold both runs"
	);
}

#[test]
fn anchored_run_must_start_at_the_shape_start() {
	let spec: ParsingSpec = test_spec();

	// Anchored `id=` matches the shape's opening static text.
	let (_, _, table) = table_for(&spec, "id=%digits%", "id=*");
	assert!(!table.is_impossible());

	// Anchored `d=` does not, since it is not at offset 0.
	let (_, _, table) = table_for(&spec, "id=%digits%", "d=*");
	assert!(
		table.is_impossible(),
		"an anchored run cannot start mid-text"
	);
}

#[test]
fn static_text_occurrence_is_required_verbatim() {
	let spec: ParsingSpec = test_spec();
	// `hello` is both a substring of the static text *and* a word the rule can emit,
	// so both placements are reported:
	// the run does not "have" to be in the variable, but it may be.
	let (_, _, table) = table_for(&spec, "hello %word%", "*hello*");
	assert_eq!(vec!["0:'hello'", "1:<hello>"], rendered(&table, 0));

	// `hellp` is not in the static text, but `%word%` is `[a-z]+`,
	// so only the rule placement remains.
	let (_, _, table) = table_for(&spec, "hello %word%", "*hellp*");
	assert_eq!(vec!["1:<hellp>"], rendered(&table, 0));

	// A run in neither is impossible.
	let (_, _, table) = table_for(&spec, "hello %word%", "*HELLO*");
	assert!(table.is_impossible());
}

#[test]
fn overlapping_occurrences_in_static_text_are_all_placed() {
	let spec: ParsingSpec = test_spec();

	// `aa` occurs in `aaa` at offsets 0 *and* 1; `str::match_indices` would only find 0.
	let (_, _, table) = table_for(&spec, "aaa", "*aa*");
	let starts: Vec<Position> = Vec::from_iter(table.placements[0].iter().map(Placement::start));
	assert_eq!(vec![(0, 0), (0, 1)], starts);

	// An end-anchored run needs the later occurrence.
	let (model, _, table) = table_for(&spec, "aaa", "*aa");
	assert_eq!(1, table.placements[0].len());
	assert_eq!((0, 1), table.placements[0][0].start());
	assert!(can_compose(&table, model.parts.len()));

	// Multi-byte characters: offsets are in characters, not bytes.
	let (_, _, table) = table_for(&spec, "\u{e9}\u{e9}\u{e9}", "*\u{e9}\u{e9}*");
	let starts: Vec<Position> = Vec::from_iter(table.placements[0].iter().map(Placement::start));
	assert_eq!(vec![(0, 0), (0, 1)], starts);
}

#[test]
fn a_run_passes_through_a_nullable_variable() {
	let spec: ParsingSpec =
		spec_with_rules(&[("word", "[a-z]+"), ("optional", r"<(?<pad>[!?]*)>")]);

	// `ab` must cross `optional.pad`, which contributes nothing.
	let (model, _, table) = table_for(&spec, "a%optional.pad%b", "ab");
	assert_eq!(vec!["0:'a'+1:<>+2:'b'"], rendered(&table, 0));
	assert!(can_compose(&table, model.parts.len()));

	// The same from a rule: `word` supplies `x`, the nullable variable nothing, `b` the rest.
	let (_, _, table) = table_for(&spec, "%word%%optional.pad%b", "*xb*");
	assert!(
		rendered(&table, 0).contains(&"0:<x>+1:<>+2:'b'".to_owned()),
		"got {:?}",
		rendered(&table, 0)
	);

	// A non-nullable variable cannot be skipped.
	let (_, _, table) = table_for(&spec, "a%word%b", "ab");
	assert!(table.is_impossible());
}

#[test]
fn repeated_characters_do_not_blow_the_placement_cap() {
	let spec: ParsingSpec = test_spec();
	// Every offset of the banner is an occurrence of `==`,
	// but interior occurrences are interchangeable, so only a few are kept.
	let shape: String = format!("%word% {} end", "=".repeat(4 * MAX_PLACEMENTS_PER_RUN));

	let (_, _, table) = table_for(&spec, &shape, "*==*");
	assert_eq!(1, table.placements[0].len());

	// A second run in the same part needs an occurrence after the first one ends.
	let (model, _, table) = table_for(&spec, &shape, "*==*==*");
	assert!(table.placements[1].len() <= 2);
	assert!(can_compose(&table, model.parts.len()));

	// The occurrence ending the text is a distinct rendering class, so it is kept.
	let (_, _, table) = table_for(&spec, "a===", "*==");
	assert_eq!(
		vec![(0, 2)],
		Vec::from_iter(table.placements[0].iter().map(Placement::start))
	);
}

/// Rows are computed left to right; a run's placements in a static part depend on where
/// the previous run can leave off there. `*a*aa*` against `aaaa` must place the second
/// run past the first, so that an infeasible composition is not suggested
/// and a feasible one is still found.
#[test]
fn placement_rows_are_relative_to_the_previous_run() {
	let spec: ParsingSpec = test_spec();
	let (model, _, table) = table_for(&spec, "aaaa", "*a*aa*");

	// The first run (`a`) has placements anywhere; the second (`aa`):
	// every placement is reachable from some placement of the first run,
	// *or* it is the boundary occurrence (`0`, the end) that a later run may still
	// start at when entered from an earlier part -- entry `0` is always offered.
	// Here only offsets that are reachable matter for composition; `Reachability`
	// prunes the rest.
	for placement in table.placements[1].iter() {
		let reachable: bool = 0 == placement.start_offset
			|| table.placements[0]
				.iter()
				.any(|first| first.next_available() <= placement.start());
		assert!(
			reachable || (placement.start_offset + 2 == 4),
			"placement {:?} is neither reachable from an earlier placement nor a boundary",
			placement,
		);
	}
	assert!(can_compose(&table, model.parts.len()));

	// And when nothing fits *after* the earlier run, the later run has no placement
	// in the part at all.
	let (_, _, table) = table_for(&spec, "aa", "*a*aaa*");
	assert!(
		table.placements[1].is_empty(),
		"the run `aaa` cannot follow `a` inside `aa`"
	);
}
