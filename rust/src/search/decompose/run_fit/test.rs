use super::*;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::search::SymbolicChar;

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
		("path", r"/[a-z]+"),
	])
}

/// The split points at which the rule can supply a leading part of the run:
/// `k` such that the rule can *end* with `run[..k]`, with the end pinned.
fn suffix_splits(cache: &RunFitCache, spec: &ParsingSpec, name: &str, run: &str) -> Vec<usize> {
	let characters: Vec<char> = run.chars().collect::<Vec<_>>();
	Vec::from_iter((0..=characters.len()).filter(|&k| {
		let piece: String = characters[..k].iter().collect::<String>();
		cache.matches_piece(
			spec,
			name,
			&piece,
			Pinned {
				start: false,
				end: true,
			},
		)
	}))
}

/// The split points at which the rule can supply a trailing part of the run:
/// `k` such that the rule can *begin* with `run[k..]`, with the start pinned.
fn prefix_splits(cache: &RunFitCache, spec: &ParsingSpec, name: &str, run: &str) -> Vec<usize> {
	let characters: Vec<char> = run.chars().collect::<Vec<_>>();
	Vec::from_iter((0..=characters.len()).filter(|&k| {
		let piece: String = characters[k..].iter().collect::<String>();
		cache.matches_piece(
			spec,
			name,
			&piece,
			Pinned {
				start: true,
				end: false,
			},
		)
	}))
}

#[test]
fn whole_run_inside_a_rule() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();
	let unpinned: Pinned = Pinned {
		start: false,
		end: false,
	};

	assert!(
		cache.matches_piece(&spec, "digits", "123", unpinned),
		"a digit run sits inside a digit rule"
	);
	assert!(
		!cache.matches_piece(&spec, "digits", "abc", unpinned),
		"a letter run cannot sit inside a digits-only rule"
	);
}

#[test]
fn empty_contributions_are_always_available() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();

	// Supplying *none* of the run is always possible --
	// it wraps to the query `*`, which matches any rule --
	// and is how a rule irrelevant to the run is represented.
	for pinned in [
		Pinned {
			start: false,
			end: false,
		},
		Pinned {
			start: false,
			end: true,
		},
		Pinned {
			start: true,
			end: false,
		},
	] {
		assert!(
			cache.matches_piece(&spec, "digits", "", pinned),
			"{pinned:?}"
		);
	}
}

#[test]
fn straddle_splits_for_a_partially_matching_run() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();

	// Run `12x`: the digits rule can end with `1` or `12`, but not `12x`,
	// and cannot contain it whole either.
	assert_eq!(vec![0, 1, 2], suffix_splits(&cache, &spec, "digits", "12x"));
	assert!(
		!cache.matches_piece(
			&spec,
			"digits",
			"12x",
			Pinned {
				start: false,
				end: false,
			},
		),
		"`12x` cannot sit wholly inside `[0-9]+`"
	);

	// Run `x12`: the digits rule can begin with `12` (split at 1) or `2` (split at 2).
	assert_eq!(vec![1, 2, 3], prefix_splits(&cache, &spec, "digits", "x12"));
}

#[test]
fn straddle_is_bounded_by_the_rules_language() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();

	// `level` is a fixed alternation, so only genuine suffixes of a branch qualify:
	// the rule can *end* with a piece of `INFOx` only for the empty piece and `INFO`.
	assert_eq!(vec![0, 4], suffix_splits(&cache, &spec, "level", "INFOx"));
}

#[test]
fn run_with_no_relationship_to_the_rule_matches_nothing() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();

	assert!(
		!cache.matches_piece(
			&spec,
			"level",
			"zzz",
			Pinned {
				start: false,
				end: false,
			},
		),
		"`zzz` shares nothing with INFO|WARN|ERROR"
	);
}

#[test]
fn special_characters_in_a_run_are_escaped() {
	let spec: ParsingSpec = spec_with_rules(&[("star", r"a\*b"), ("slash", r"a\\b")]);
	let cache: RunFitCache = RunFitCache::new();
	let unpinned: Pinned = Pinned {
		start: false,
		end: false,
	};

	// A literal `*` in the run must not be read as a wildcard.
	assert!(cache.matches_piece(&spec, "star", "a*b", unpinned));

	// A literal backslash likewise.
	assert!(cache.matches_piece(&spec, "slash", r"a\b", unpinned));

	// And a `*` run must not match a rule that has no literal star.
	assert!(!cache.matches_piece(&spec, "slash", "a*b", unpinned));
}

#[test]
fn unknown_rule_matches_nothing() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();

	// Callers must treat this as "cannot conclude", not as a proof of no match.
	assert!(!cache.matches_piece(
		&spec,
		"nonexistent",
		"abc",
		Pinned {
			start: false,
			end: false,
		},
	));
}

#[test]
fn cache_reuses_computations() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();
	assert!(cache.is_empty());

	let unpinned: Pinned = Pinned {
		start: false,
		end: false,
	};
	assert!(cache.matches_piece(&spec, "digits", "123", unpinned));
	assert_eq!(1, cache.len());

	// A hit does not add an entry. Note the four pinnings are four queries.
	assert!(cache.matches_piece(&spec, "digits", "123", unpinned));
	assert_eq!(1, cache.len());

	// Distinct keys are cached separately.
	let _ = cache.matches_piece(
		&spec,
		"digits",
		"123",
		Pinned {
			start: false,
			end: true,
		},
	);
	let _ = cache.matches_piece(&spec, "word", "123", unpinned);
	assert_eq!(3, cache.len());

	cache.clear();
	assert!(cache.is_empty());
}

#[test]
fn cache_clone_shares_entries() {
	let spec: ParsingSpec = test_spec();
	let cache: RunFitCache = RunFitCache::new();
	let unpinned: Pinned = Pinned {
		start: false,
		end: false,
	};
	assert!(cache.matches_piece(&spec, "digits", "123", unpinned));

	let cloned: RunFitCache = cache.clone();
	assert_eq!(1, cloned.len());
	// The clone holds the same answer rather than recomputing it.
	assert!(cloned.matches_piece(&spec, "digits", "123", unpinned));
}

fn literal(text: &str) -> Vec<SymbolicChar> {
	Vec::from_iter(text.chars().map(|c| match c {
		'*' => SymbolicChar::GlobStar,
		_ => SymbolicChar::Literal(c),
	}))
}

/// The rule's own answer to "can it emit text beside the value?", per side.
#[test]
fn slack_is_what_the_rule_can_emit_beside_the_value() {
	let spec: ParsingSpec = spec_with_rules(&[
		("level", "INFO|WARN|ERROR"),
		("digits", "[0-9]+"),
		("hex", "0x[0-9a-f]+"),
		("alt", "a|ba"),
	]);
	let cache: RunFitCache = RunFitCache::new();

	// An alternation of whole words has no slack around one of them.
	assert!(!cache.has_slack(&spec, "level", &literal("INFO"), Side::Before));
	assert!(!cache.has_slack(&spec, "level", &literal("INFO"), Side::After));
	// Nor between pieces of one word, e.g. `IN*O`: no level is `?IN*O` or `IN*O?`.
	assert!(!cache.has_slack(&spec, "level", &literal("IN*O"), Side::Before));
	assert!(!cache.has_slack(&spec, "level", &literal("IN*O"), Side::After));

	// A repetition has slack on both sides.
	assert!(cache.has_slack(&spec, "digits", &literal("12"), Side::Before));
	assert!(cache.has_slack(&spec, "digits", &literal("12"), Side::After));

	// A fixed prefix removes slack on one side only.
	assert!(!cache.has_slack(&spec, "hex", &literal("0xff"), Side::Before));
	assert!(cache.has_slack(&spec, "hex", &literal("0xff"), Side::After));

	// The case a glob query cannot decide: `a*` is satisfiable (by `a`),
	// yet `ba` is a match with text before `a`, so the leading `*` of `*a` is used.
	assert!(cache.has_slack(&spec, "alt", &literal("a"), Side::Before));
	assert!(!cache.has_slack(&spec, "alt", &literal("a"), Side::After));

	// An unknown rule keeps its padding.
	assert!(cache.has_slack(&spec, "missing", &literal("a"), Side::Before));
}
