use super::*;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::search::SearchString;
use crate::search::SymbolicChar;
use crate::search::decompose::RunFitCache;

fn spec_with_rules(rules: &[(&str, &str)]) -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for &(name, pattern) in rules.iter() {
		builder.add_rule(name, pattern).unwrap();
	}
	builder.build()
}

fn test_spec() -> ParsingSpec {
	spec_with_rules(&[("digits", "[0-9]+"), ("word", "[a-z]+")])
}

/// The texts of `pieces`, comma separated.
fn texts(pieces: &[PieceRef]) -> String {
	pieces
		.iter()
		.map(|piece| piece.text.clone())
		.collect::<Vec<_>>()
		.join(",")
}

fn composed_for(spec: &ParsingSpec, shape: &str, query: &str) -> (ShapeModel, Composed) {
	composed_for_parts(spec, shape, query).0
}

/// The model and the composition for `query` against `shape`,
/// plus the full [`Query`] for rendering.
fn composed_for_parts(
	spec: &ParsingSpec,
	shape: &str,
	query: &str,
) -> ((ShapeModel, Composed), SearchString) {
	let model: ShapeModel = ShapeModel::new(spec, shape);
	let parsed: SearchString = SearchString::parse(query).unwrap();
	let decomposed: Query<'_> = Query::new(&parsed);
	let fits: RunFitCache = RunFitCache::new();
	let table: PlacementTable = PlacementTable::compute(spec, &model, &decomposed, &fits).unwrap();
	let composed: Composed = compose(
		spec,
		&model,
		&table,
		&decomposed,
		&fits,
		ComposeBudget::default(),
	);
	((model, composed), parsed)
}

/// Renders a composition as sorted `part=text` entries, marking captures with `<>`.
fn rendered(composed: &Composed) -> Vec<String> {
	let Composed::Compositions(compositions) = composed else {
		panic!("expected compositions, got {composed:?}");
	};
	let mut out: Vec<String> = compositions
		.iter()
		.map(|composition| {
			let mut entries: Vec<String> = Vec::new();
			for capture in composition.captures.iter() {
				entries.push(format!(
					"{}:<{}={}>",
					capture.part,
					capture.name,
					texts(&capture.pieces)
				));
			}
			for literal in composition.literals.iter() {
				entries.push(format!("{}:'{}'", literal.part, texts(&literal.pieces)));
			}
			entries.sort();
			entries.join(" ")
		})
		.collect::<Vec<_>>();
	out.sort();
	out.dedup();
	out
}

#[test]
fn single_run_inside_a_rule() {
	let spec: ParsingSpec = test_spec();
	let (_, composed) = composed_for(&spec, "id=%digits%", "*123*");
	assert_eq!(vec!["1:<digits=123>"], rendered(&composed));
}

#[test]
fn single_run_in_static_text() {
	let spec: ParsingSpec = test_spec();
	let (_, composed) = composed_for(&spec, "id=%digits%", "*id=*");
	assert_eq!(vec!["0:'id='"], rendered(&composed));
}

#[test]
fn run_straddling_rule_and_text() {
	let spec: ParsingSpec = test_spec();
	// `12y`: `12` from the rule at part 1, `y` from static text at part 2.
	let (_, composed) = composed_for(&spec, "x%digits%y", "*12y*");
	assert!(
		rendered(&composed).contains(&"1:<digits=12> 2:'y'".to_owned()),
		"got {:?}",
		rendered(&composed)
	);
}

#[test]
fn positional_identity_is_recorded() {
	let spec: ParsingSpec = test_spec();
	// The requirement: two `word` instances must be distinguishable.
	// The composition names the part index,
	// so the two placements yield visibly different results.
	let (_, composed) = composed_for(&spec, "A%word%B%word%C", "*qq*");
	assert_eq!(
		vec!["1:<word=qq>", "3:<word=qq>"],
		rendered(&composed),
		"each instance of `word` must be identified by its part index"
	);
}

#[test]
fn two_runs_in_distinct_rules() {
	let spec: ParsingSpec = test_spec();
	let (_, composed) = composed_for(&spec, "A%word%B%digits%C", "*qq*77*");
	assert!(
		rendered(&composed).contains(&"1:<word=qq> 3:<digits=77>".to_owned()),
		"got {:?}",
		rendered(&composed)
	);
}

#[test]
fn two_runs_sharing_one_rule_are_merged() {
	let spec: ParsingSpec = test_spec();
	// Both runs can sit inside the same rule, separated by text the rule also produces.
	let (_, composed) = composed_for(&spec, "id=%digits%", "*1*2*");
	assert!(
		rendered(&composed).contains(&"1:<digits=1,2>".to_owned()),
		"pieces landing in one part must merge in order; got {:?}",
		rendered(&composed)
	);
}

#[test]
fn ordering_is_enforced() {
	let spec: ParsingSpec = test_spec();
	// In-order composes.
	let (_, composed) = composed_for(&spec, "A%digits%B%digits%C", "*A*B*");
	assert!(matches!(composed, Composed::Compositions(_)));

	// Reversed does not: each run is placeable alone, but not left to right.
	let (_, composed) = composed_for(&spec, "A%digits%B%digits%C", "*B*A*");
	assert!(matches!(composed, Composed::Impossible), "got {composed:?}");
}

#[test]
fn run_that_fits_nowhere_is_impossible() {
	let spec: ParsingSpec = test_spec();
	let (_, composed) = composed_for(&spec, "id=%digits%", "*#*");
	assert!(matches!(composed, Composed::Impossible));
}

#[test]
fn wildcard_only_query_has_one_empty_composition() {
	let spec: ParsingSpec = test_spec();
	// `*` constrains nothing, so there is exactly one trivial decomposition.
	let (_, composed) = composed_for(&spec, "id=%digits%", "*");
	let Composed::Compositions(compositions) = &composed else {
		panic!("expected compositions, got {composed:?}");
	};
	assert_eq!(1, compositions.len());
	assert!(compositions[0].captures.is_empty());
	assert!(compositions[0].literals.is_empty());
}

/// An end-anchored query pins every part past the last constrained one to the
/// empty string; composition renders that as an empty capture, like the engine.
#[test]
fn end_anchored_tail_is_empty_captures() {
	let spec: ParsingSpec = spec_with_rules(&[
		("digits", "[0-9]+"),
		("optional", r"<(?<pad>[!?]*)>"),
		("word", "[a-z]+"),
	]);

	// `*hello` against `msg=%word%%optional.pad%`: `%optional.pad%` can vanish,
	// and placement admits it as the message's end, so it is part of the tail.
	assert_eq!(
		vec!["'*' <word=*hello> <optional.pad=>"],
		interpretations_of(&spec, "msg=%word%%optional.pad%", "*hello")
	);
	// A *non*-nullable tail cannot vanish; the shape still does not match.
	assert!(!composes(&spec, "id=%digits%", "*id="));
}

/// The empty query, anchored at both ends, pins *every* part to producing nothing.
#[test]
fn empty_query_pins_every_variable_to_empty() {
	let spec: ParsingSpec = spec_with_rules(&[("optional", r"<(?<pad>[!?]*)>")]);

	// A shape of only nullable variables matches the empty query,
	// each as an empty capture.
	assert_eq!(
		vec!["<optional.pad=> <optional.pad=>"],
		interpretations_of(&spec, "%optional.pad%%optional.pad%", "")
	);
	// Static text cannot vanish.
	assert!(!composes(&spec, "abc", ""));
	assert!(!composes(&spec, "x%optional.pad%", ""));
}

#[test]
fn exhausted_budget_is_unknown_not_impossible() {
	let spec: ParsingSpec = test_spec();
	let model: ShapeModel = ShapeModel::new(&spec, "A%word%B%word%C%word%D");
	let parsed: SearchString = SearchString::parse("*q*q*q*").unwrap();
	let decomposed: Query<'_> = Query::new(&parsed);
	let fits: RunFitCache = RunFitCache::new();
	let table: PlacementTable = PlacementTable::compute(&spec, &model, &decomposed, &fits).unwrap();
	let composed: Composed = compose(
		&spec,
		&model,
		&table,
		&decomposed,
		&fits,
		ComposeBudget {
			max_compositions: 1,
		},
	);
	// Degrading must never look like a proof of impossibility.
	assert!(matches!(composed, Composed::Unknown), "got {composed:?}");
}

/// Renders an interpretation the way the engine's `Debug` does, but compactly.
fn render_interpretation(interpretation: &crate::search::Interpretation) -> String {
	interpretation
		.leaf_queries
		.iter()
		.map(|leaf_query| {
			if !leaf_query.is_static_text() {
				format!(
					"<{}={}>",
					leaf_query.fully_qualified_name, leaf_query.string_value
				)
			} else {
				format!("'{}'", leaf_query.string_value)
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}

fn interpretations_of(spec: &ParsingSpec, shape: &str, query: &str) -> Vec<String> {
	let ((model, composed), parsed) = composed_for_parts(spec, shape, query);
	let decomposed: Query<'_> = Query::new(&parsed);
	let Composed::Compositions(compositions) = &composed else {
		panic!("expected compositions, got {composed:?}");
	};
	let mut out: Vec<String> = compositions
		.iter()
		.map(|composition| {
			render_interpretation(&composition.to_interpretation(
				spec,
				&model,
				&decomposed,
				&RunFitCache::new(),
			))
		})
		.collect::<Vec<_>>();
	out.sort();
	out.dedup();
	out
}

/// The query text an interpretation accounts for, as runs of literal characters.
///
/// Concatenating sub-query values and splitting on wildcards recovers
/// what the query's runs must have been,
/// which is what the invariant compares against.
fn runs_accounted_for(interpretation: &crate::search::Interpretation) -> Vec<String> {
	let mut runs: Vec<String> = vec![String::new()];
	for leaf_query in interpretation.leaf_queries.iter() {
		for symbol in leaf_query.symbolic_value.iter() {
			match symbol {
				SymbolicChar::Literal(character) => {
					runs.last_mut().expect("non-empty").push(*character)
				},
				SymbolicChar::GlobStar => {
					if !runs.last().expect("non-empty").is_empty() {
						runs.push(String::new());
					}
				},
			}
		}
	}
	runs.retain(|run| !run.is_empty());
	runs
}

#[test]
fn every_literal_character_survives_rendering() {
	let spec: ParsingSpec = test_spec();
	// Shapes that exercise static text, single and repeated rules, and adjacent rules.
	const SHAPES: &[&str] = &[
		"id=%digits%",
		"A%word%B%word%C",
		"A%word%B%digits%C",
		"%word%%digits%",
		"x%digits%y%word%z%digits%w",
	];
	// Queries whose runs must reappear intact, including ones that straddle part boundaries.
	const QUERIES: &[&str] = &[
		"*qq*", "*77*", "*id=*", "*id=7*", "*1*2*", "*qq*77*", "*x*y*", "*ab7*", "*7cd*",
	];

	let mut checked: usize = 0;
	for shape in SHAPES.iter() {
		for query in QUERIES.iter() {
			let ((model, composed), parsed) = composed_for_parts(&spec, shape, query);
			let decomposed: Query<'_> = Query::new(&parsed);
			let Composed::Compositions(compositions) = &composed else {
				continue;
			};
			// What the query itself asks for.
			let expected: Vec<String> = decomposed
				.runs
				.iter()
				.map(|run| run.text.clone())
				.collect::<Vec<_>>();

			for composition in compositions.iter() {
				let interpretation =
					composition.to_interpretation(&spec, &model, &decomposed, &RunFitCache::new());
				assert_eq!(
					expected,
					runs_accounted_for(&interpretation),
					"query {query:?} on shape {shape:?} rendered as {:?}",
					render_interpretation(&interpretation)
				);
				checked += 1;
			}
		}
	}
	// Guard against the assertions being vacuous.
	assert!(checked > 25, "only {checked} interpretations checked");
}

#[test]
fn vacuous_captures_encode_position() {
	let spec: ParsingSpec = test_spec();
	// Two `word` references: the first is identified by having no vacuous capture before it,
	// the second by having exactly one.
	// The surrounding `'*'`s are the shape's static text (`A`, `B`, `C`),
	// reported unconstrained because the query's wildcards merely pass over them.
	assert_eq!(
		vec!["'*' <word=*> '*' <word=*qq*> '*'", "'*' <word=*qq*> '*'"],
		interpretations_of(&spec, "A%word%B%word%C", "*qq*")
	);
}

#[test]
fn trailing_unconstrained_rules_are_omitted() {
	let spec: ParsingSpec = test_spec();
	// Three references, only the first constrained: the trailing two are unconstrained and omitted.
	// The shape's static text is still reported,
	// so each rendering ends with a `'*'`, never a vacuous capture.
	let rendered: Vec<String> = interpretations_of(&spec, "A%word%B%word%C%word%D", "*qq*");
	assert!(
		rendered.contains(&"'*' <word=*qq*> '*'".to_owned()),
		"got {rendered:?}"
	);
	for one in rendered.iter() {
		assert!(!one.ends_with("=*>"), "trailing vacuous capture in {one:?}");
	}
}

#[test]
fn text_matched_by_static_text_is_preserved() {
	let spec: ParsingSpec = test_spec();
	// The run is satisfied by the shape's static text: no rule is constrained,
	// but the query's characters must still appear.
	// The run covers the whole of `id=`, so there is no wildcard *before* it --
	// a static value must glob-match its part's text exactly, and `'*id='` would not match `id=`.
	// The trailing `*` is not padding for `id=` but for the omitted `%digits%` after it;
	// see `omitted_trailing_variables_leave_a_trailing_wildcard`.
	assert_eq!(
		vec!["'id=*'"],
		interpretations_of(&spec, "id=%digits%", "*id=*")
	);
}

/// Rule references after the last constrained part are omitted,
/// but they still emit text: if the rendering ends in static text,
/// nothing else stands for it, so that value must end with `*`.
/// Without it `'id='` would claim the message ends after `id=`.
#[test]
fn omitted_trailing_variables_leave_a_trailing_wildcard() {
	let spec: ParsingSpec = test_spec();
	// The shape ends in the omitted variable.
	assert_eq!(
		vec!["'id=*'"],
		interpretations_of(&spec, "id=%digits%", "*id=*")
	);
	assert_eq!(
		vec!["'id=*'"],
		interpretations_of(&spec, "id=%digits%", "id=*")
	);
	// Several omitted variables, still a single wildcard.
	assert_eq!(
		vec!["'id=*'"],
		interpretations_of(&spec, "id=%digits%%word%", "*id=*")
	);
	// Static text after the omitted variable already carries the wildcard; nothing changes.
	assert_eq!(
		vec!["'id=*'"],
		interpretations_of(&spec, "id=%digits% end", "*id=*")
	);
	// Nothing is omitted, so nothing is added: the run covers the whole shape.
	assert_eq!(vec!["'abc'"], interpretations_of(&spec, "abc", "*abc*"));
	// A trailing *capture* is padded by its own rule, not by this.
	assert_eq!(
		vec!["'id=' <digits=7*>"],
		interpretations_of(&spec, "id=%digits%", "*id=7*")
	);
}

/// Compositions that differ only in *where* text sits inside a static part
/// are one decomposition, and must be reported once.
#[test]
fn compositions_differing_only_in_static_offsets_are_merged() {
	let spec: ParsingSpec = test_spec();
	// `aa` at offset 0 (`'aa*'`) or 1 (`'*aa'`): one decomposition.
	assert_eq!(vec!["'*aa*'"], interpretations_of(&spec, "aaa", "*aa*"));
	// Three ways to place `a` then `a` in `aaa`.
	assert_eq!(vec!["'*a*a*'"], interpretations_of(&spec, "aaa", "*a*a*"));
	// Anchoring pins the offset, so there is nothing to merge.
	assert_eq!(vec!["'*aa'"], interpretations_of(&spec, "aaa", "*aa"));
	assert_eq!(vec!["'aa*'"], interpretations_of(&spec, "aaa", "aa*"));
	// Different *parts* are different decompositions, and are kept apart:
	// the first static part (with `%word%` omitted), the rule, or the last static part.
	assert_eq!(
		vec!["'*' <word=*> '*x*'", "'*' <word=*x*> '*'", "'*x*'"],
		interpretations_of(&spec, "xx%word%xx", "*x*")
	);
}

/// A static sub-query's value must glob-match its shape part's text *exactly*,
/// so a run landing strictly inside a longer static part has to be padded on **both** sides.
///
/// Padding used to be allowed only for rules,
/// on the grounds that static text "must match verbatim".
/// That reasoning is backwards: precisely because the text is reproduced verbatim,
/// a value covering only part of it must be free to skip the rest.
/// Reporting `'*ab'` for a run `ab` inside `abcdef` claims the text *ends* in `ab`,
/// which is false and unsatisfiable.
#[test]
fn a_run_inside_a_longer_static_part_is_padded_on_both_sides() {
	let spec: ParsingSpec = test_spec();
	assert_eq!(
		vec!["'*bcd*'"],
		interpretations_of(&spec, "abcdef", "*bcd*")
	);
	// Flush against the start: no padding before, but the tail must still be skippable.
	assert_eq!(vec!["'abc*'"], interpretations_of(&spec, "abcdef", "*abc*"));
	// Flush against the end: the mirror image.
	assert_eq!(vec!["'*def'"], interpretations_of(&spec, "abcdef", "*def*"));
	// The whole part: no padding at all.
	assert_eq!(
		vec!["'abcdef'"],
		interpretations_of(&spec, "abcdef", "*abcdef*")
	);
}

#[test]
fn a_run_straddling_a_boundary_stays_contiguous() {
	let spec: ParsingSpec = test_spec();
	// `id=7` is one run, split across static text and the rule.
	// The query has no wildcard inside it,
	// so the rendering must not introduce one between `id=` and `7`.
	assert_eq!(
		vec!["'id=' <digits=7*>"],
		interpretations_of(&spec, "id=%digits%", "*id=7*")
	);
}

#[test]
fn multiple_pieces_in_one_capture_are_wildcard_separated() {
	let spec: ParsingSpec = test_spec();
	// Both runs land in the one rule; the rule may emit text between them.
	// The shape's trailing static text is reported as `'*'`.
	assert_eq!(
		vec!["'*' <digits=*1*2*>"],
		interpretations_of(&spec, "id=%digits%", "*1*2*")
	);
}

#[test]
fn captures_in_distinct_rules_keep_their_positions() {
	let spec: ParsingSpec = test_spec();
	let rendered: Vec<String> = interpretations_of(&spec, "A%word%B%digits%C", "*qq*77*");
	assert!(
		rendered.contains(&"'*' <word=*qq*> '*' <digits=*77*> '*'".to_owned()),
		"got {rendered:?}"
	);
}

/// Whether the composed path finds any decomposition at all.
fn composes(spec: &ParsingSpec, shape: &str, query: &str) -> bool {
	let (_, composed) = composed_for(spec, shape, query);
	matches!(&composed, Composed::Compositions(compositions) if !compositions.is_empty())
}

/// A spec whose rules are alternations, so that "the rule contains X" and "the rule *is* X" differ.
fn level_spec() -> ParsingSpec {
	spec_with_rules(&[
		("level", "TRACE|DEBUG|INFO|WARN|ERROR|FATAL"),
		("word", "[a-zA-Z]+"),
	])
}

#[test]
fn several_runs_can_share_one_static_part() {
	let spec: ParsingSpec = test_spec();
	// Regression: placements recorded only *which* part a run landed in, not where within it.
	// All three runs belong in the single static part `abc `, at increasing offsets,
	// which the part-only DP state could not express -- so this decomposition was missed entirely.
	assert!(composes(&spec, "abc %word%", "a*b*c"));
	// The offsets must be respected, not merely recorded: `c*b` is not in ascending order.
	assert!(!composes(&spec, "abc %word%", "c*b*a"));
}

#[test]
fn one_rule_must_produce_all_of_its_runs_together() {
	let spec: ParsingSpec = level_spec();
	// Regression: each run was checked against the rule *individually*,
	// so both `INFO` and `WARN` were placed in one `%level%`.
	// The rule is an alternation: it matches either alone and neither pair.
	let rendered: Vec<String> = interpretations_of(&spec, "%level% %word%", "*INFO*WARN*");
	assert!(
		!rendered
			.iter()
			.any(|one| one.contains("<level=*INFO*WARN*>")),
		"a single `level` cannot produce both runs: {rendered:?}"
	);
	// The decomposition that splits them across the two rules is still found.
	assert!(
		rendered
			.iter()
			.any(|one| one.contains("<word=*INFO*WARN*>")),
		"got {rendered:?}"
	);
}

#[test]
fn an_anchored_run_must_start_the_rule_not_merely_occur_in_it() {
	let spec: ParsingSpec = level_spec();
	// Regression: containment (`fits_wholly`) was used where anchoring demands the rule *begin*
	// with the run. `WARN` contains `N`, but no level starts with it,
	// so `N*` must not be placed in `%level%`.
	assert!(!composes(&spec, "%level% %word%", "N*"));
	// A level that really does start with `I` is still placed.
	assert!(composes(&spec, "%level% %word%", "I*"));
}

#[test]
fn an_anchored_straddle_must_match_the_rule_exactly() {
	let spec: ParsingSpec = level_spec();
	// Regression: the straddle path used `suffixes[split]` ("the rule can *end* with this"),
	// but an anchored run pins the rule's start too,
	// so the rule must match the piece exactly.
	// `WARN` ends with `N`, which allowed `NIn*` to be split as `level=N` + `word=In`.
	assert!(!composes(&spec, "%level%%word%", "NIn*"));
	// The same shape still admits a split where the first piece really is a whole level.
	assert!(composes(&spec, "%level%%word%", "INFOxy*"));
}

#[test]
fn captures_and_literals_are_in_shape_order() {
	let spec: ParsingSpec = test_spec();
	let (_, composed) = composed_for(&spec, "a%word%b%digits%c", "*xx*99*");
	let Composed::Compositions(compositions) = &composed else {
		panic!("expected compositions");
	};
	for composition in compositions.iter() {
		let parts: Vec<usize> = composition
			.captures
			.iter()
			.map(|c| c.part)
			.collect::<Vec<_>>();
		let mut sorted: Vec<usize> = parts.clone();
		sorted.sort_unstable();
		assert_eq!(sorted, parts, "captures must be in shape order");
	}
}

#[test]
fn unconstrained_static_text_is_reported_as_a_wildcard() {
	let spec: ParsingSpec = test_spec();
	// The query constrains only the rule; the shape's static text is reported as `'*'`,
	// matching the engine and `search_by_name`'s output shape, rather than being dropped.
	assert_eq!(
		vec!["'*' <word=*qq*> '*'"],
		interpretations_of(&spec, "A%word%B", "*qq*")
	);
}

#[test]
fn a_run_split_between_text_and_a_rule_yields_two_leaf_queries() {
	let spec: ParsingSpec = test_spec();
	// A run of `foobar` against `foo%word%`:
	// `foo` is the shape's static text and `bar` is the rule's.
	// Both must appear, as two sub-queries.
	assert_eq!(
		vec!["'foo' <word=bar>"],
		interpretations_of(&spec, "foo%word%", "foobar")
	);
}

#[test]
fn a_static_gap_between_captures_is_one_wildcard() {
	let spec: ParsingSpec = test_spec();
	// Both captures are constrained, so the static text separating them is reported once as `'*'`.
	assert!(
		interpretations_of(&spec, "A%word%B%digits%C", "*qq*77*")
			.contains(&"'*' <word=*qq*> '*' <digits=*77*> '*'".to_owned()),
		"got {:?}",
		interpretations_of(&spec, "A%word%B%digits%C", "*qq*77*")
	);
}

#[test]
fn adjacent_static_parts_are_merged() {
	let spec: ParsingSpec = test_spec();
	// An escaped `%` tokenizes `a%%b` into two adjacent static parts.
	// They must render as one sub-query,
	// because `Interpretation::invariants` forbids two static sub-queries in a row.
	for rendered in interpretations_of(&spec, "a%%b%word%", "*a*") {
		let static_count: usize = rendered.matches("'").count() / 2;
		assert!(
			static_count <= 1,
			"adjacent static parts not merged: {rendered:?}"
		);
	}
}

/// A capture's padding is tightened to what the rule can emit, matching the engine.
#[test]
fn capture_padding_is_tightened_to_the_rule() {
	let spec: ParsingSpec = spec_with_rules(&[
		("level", "INFO|WARN|ERROR"),
		("digits", "[0-9]+"),
		("hex", "0x[0-9a-f]+"),
		("alt", "a|ba"),
	]);

	// No level has text around `INFO`, so neither `*` survives.
	assert_eq!(
		vec![String::from("<level=INFO>")],
		interpretations_of(&spec, "%level%", "*INFO*")
	);
	// Interior wildcards are run boundaries the query demands; only the ends go.
	assert_eq!(
		vec![String::from("<level=IN*O>")],
		interpretations_of(&spec, "%level%", "*IN*O*")
	);
	// One side only: a hex number starts with `0x` but can continue.
	assert_eq!(
		vec![String::from("<hex=0xff*>")],
		interpretations_of(&spec, "%hex%", "*0xff*")
	);
	// A repetition keeps both.
	assert_eq!(
		vec![String::from("<digits=*12*>")],
		interpretations_of(&spec, "%digits%", "*12*")
	);
	// `ba` uses the leading slack of `*a`, so it must stay, though `a*` alone is satisfiable.
	assert_eq!(
		vec![String::from("<alt=*a>")],
		interpretations_of(&spec, "%alt%", "*a*")
	);
}

/// A start-anchored query pins every rule before its first run to the empty string,
/// the mirror image of [`end_anchored_tail_is_empty_captures`].
#[test]
fn start_anchored_head_is_empty_captures() {
	let spec: ParsingSpec = spec_with_rules(&[("word", "[a-z]+"), ("optional", "<(?<pad>[!?]*)>")]);

	// `hello` must begin the message, so the nullable `pad` before it emits nothing.
	assert_eq!(
		vec![String::from("<optional.pad=> <word=hello*>")],
		interpretations_of(&spec, "%optional.pad%%word%", "hello*")
	);
	// Both ends at once: nothing may surround `hello`.
	assert_eq!(
		vec![String::from("<optional.pad=> <word=hello> <optional.pad=>")],
		interpretations_of(&spec, "%optional.pad%%word%%optional.pad%", "hello")
	);
	// Unanchored, the same rule really can be anything: it stays `*`.
	assert_eq!(
		vec![String::from("<optional.pad=*> <word=*hello*>")],
		interpretations_of(&spec, "%optional.pad%%word%", "*hello*")
	);
	// An *interior* vacuous rule is left alone even when the query is anchored at both ends:
	// a run on each side of it constrains the message, not the rule.
	// (Each `word` may still continue inward, toward the query's wildcard.)
	assert_eq!(
		vec![String::from("<word=a*> <optional.pad=*> <word=*b>")],
		interpretations_of(&spec, "%word%%optional.pad%%word%", "a*b")
	);
}
