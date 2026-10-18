use super::*;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::regex::AnchoredRegex;

#[test]
fn big_pattern() {
	let dfa: Tdfa = for_pattern("0((?<foobar>1(2[a-zA-Z])*)*|(?<baz>xyz))*world");
	let b: bool = dfa.execute("012a2b2c12z12zxyzxyzxyzworld");
	assert!(b);
}

#[test]
fn bracketed_expression_with_overlapping_range() {
	let dfa: Tdfa = for_pattern("[aa]");
	let b: bool = dfa.execute("a");
	assert!(b);
}

#[test]
fn submatch_precedence() {
	let rule: RootRule = for_rule("(?<foo>a|aa)(?<bar>a|aa)");
	let dfa: &Tdfa = &rule.dfa;
	let mut data: TdfaExecution = dfa.execution_data();

	let b: bool = dfa.execute_with_captures("aaa", &mut data, &rule);
	assert!(b);

	assert_eq!(data.captures.len(), 2);
	assert_eq!(data.captures[0].range.start, 0);
	assert_eq!(data.captures[0].range.end, 1);
	assert_eq!(data.captures[1].range.start, 1);
	assert_eq!(data.captures[1].range.end, 3);
}

/// [`Tdfa::execute`] only checks that the whole input can be consumed;
/// it does not require the DFA to land on an accepting state.
#[test]
fn execute_accepts_viable_prefixes() {
	let dfa: Tdfa = for_pattern("abcdef");

	assert!(dfa.execute("abcdef"));
	assert!(dfa.execute("abc"));
	assert!(dfa.execute(""));

	assert!(!dfa.execute("abcdefg"));
	assert!(!dfa.execute("xyz"));
	assert!(!dfa.execute("abd"));
}

#[test]
fn literal_sequence() {
	let dfa: Tdfa = for_pattern("abcdef");

	assert!(full_match(&dfa, "abcdef"));

	assert!(!full_match(&dfa, ""));
	assert!(!full_match(&dfa, "abc"));
	assert!(!full_match(&dfa, "abcdefg"));
	assert!(!full_match(&dfa, "Abcdef"));
}

#[test]
fn alternation() {
	let dfa: Tdfa = for_pattern("abc|def|gh");

	assert!(full_match(&dfa, "abc"));
	assert!(full_match(&dfa, "def"));
	assert!(full_match(&dfa, "gh"));

	assert!(!full_match(&dfa, "abd"));
	assert!(!full_match(&dfa, "g"));
	assert!(!full_match(&dfa, "abcdef"));
}

#[test]
fn repetition_operators() {
	let kleene_plus: Tdfa = for_pattern("ab+c");
	assert!(full_match(&kleene_plus, "abc"));
	assert!(full_match(&kleene_plus, "abbbbc"));
	assert!(!full_match(&kleene_plus, "ac"));

	let kleene_closure: Tdfa = for_pattern("ab*c");
	assert!(full_match(&kleene_closure, "ac"));
	assert!(full_match(&kleene_closure, "abc"));
	assert!(full_match(&kleene_closure, "abbbbc"));
	assert!(!full_match(&kleene_closure, "abbbb"));

	let optional: Tdfa = for_pattern("ab?c");
	assert!(full_match(&optional, "ac"));
	assert!(full_match(&optional, "abc"));
	assert!(!full_match(&optional, "abbc"));

	let bounded: Tdfa = for_pattern("ab{2,3}c");
	assert!(!full_match(&bounded, "abc"));
	assert!(full_match(&bounded, "abbc"));
	assert!(full_match(&bounded, "abbbc"));
	assert!(!full_match(&bounded, "abbbbc"));

	let exact: Tdfa = for_pattern("ab{2}c");
	assert!(full_match(&exact, "abbc"));
	assert!(!full_match(&exact, "abc"));
	assert!(!full_match(&exact, "abbbc"));
}

#[test]
fn character_classes() {
	let ranges: Tdfa = for_pattern("[a-c1-3]+");
	assert!(full_match(&ranges, "abc123"));
	assert!(!full_match(&ranges, "abcd"));

	let negated: Tdfa = for_pattern("[^a-c]+");
	assert!(full_match(&negated, "xyz"));
	assert!(!full_match(&negated, "xbz"));

	let escape_classes: Tdfa = for_pattern(r"\w+\s\d+");
	assert!(full_match(&escape_classes, "abc 123"));
	assert!(full_match(&escape_classes, "aB9\t0"));
	assert!(!full_match(&escape_classes, "abc 12a"));
	assert!(!full_match(&escape_classes, "abc-123"));

	let any_char: Tdfa = for_pattern("a.c");
	assert!(full_match(&any_char, "abc"));
	assert!(full_match(&any_char, "a\nc"));
	// A multi-byte code point counts as a single character.
	assert!(full_match(&any_char, "a\u{e9}c"));
	assert!(!full_match(&any_char, "ac"));
}

#[test]
fn ip_address_like_pattern() {
	let dfa: Tdfa = for_pattern(r"\d+\.\d+\.\d+\.\d+");

	assert!(full_match(&dfa, "1.2.3.4"));
	assert!(full_match(&dfa, "127.0.0.1"));

	assert!(!full_match(&dfa, "1.2.3"));
	assert!(!full_match(&dfa, "1.2.3."));
	assert!(!full_match(&dfa, "1.2.3.4.5"));
	assert!(!full_match(&dfa, "1x2.3.4.5"));
}

#[test]
fn captures_in_sequence() {
	let rule: RootRule = for_rule("(?<a>x)(?<b>y)");
	let captures: Vec<(u16, usize, usize)> = captures_of(&rule, "xy");

	assert_eq!(captures, vec![(1, 0, 1), (2, 1, 2)]);
}

#[test]
fn captures_of_leaf_values() {
	let rule: RootRule = for_rule(r"(?<num>\d+)\.(?<frac>\d+)");
	let captures: Vec<(u16, usize, usize)> = captures_of(&rule, "12.345");

	assert_eq!(captures, vec![(1, 0, 2), (2, 3, 6)]);
}

/// Repeated captures are reported once per iteration, left to right.
#[test]
fn repeated_capture_reports_each_iteration() {
	let rule: RootRule = for_rule("(?<a>x)+");
	let captures: Vec<(u16, usize, usize)> = captures_of(&rule, "xxx");

	assert_eq!(captures, vec![(1, 0, 1), (1, 1, 2), (1, 2, 3)]);

	let bounded: RootRule = for_rule("(?<a>b){2,3}");
	assert_eq!(captures_of(&bounded, "bb"), vec![(1, 0, 1), (1, 1, 2)]);
	assert_eq!(
		captures_of(&bounded, "bbb"),
		vec![(1, 0, 1), (1, 1, 2), (1, 2, 3)]
	);
}

/// Captures are sorted left to right, then parent before child.
#[test]
fn nested_captures() {
	let rule: RootRule = for_rule("(?<outer>a(?<inner>b)c)");
	let dfa: &Tdfa = &rule.dfa;
	let mut data: TdfaExecution = dfa.execution_data();

	assert!(dfa.execute_with_captures("abc", &mut data, &rule));
	assert_eq!(data.captures.len(), 2);

	let outer: &MatchedCapture = &data.captures[0];
	assert_eq!(outer.capture_id.get(), 1);
	assert_eq!(outer.parent_id, None);
	assert_eq!(outer.parent_index, 0);
	assert!(!outer.is_leaf);
	assert_eq!((outer.range.start, outer.range.end), (0, 3));

	let inner: &MatchedCapture = &data.captures[1];
	assert_eq!(inner.capture_id.get(), 2);
	assert_eq!(inner.parent_id.map(NonZero::get), Some(1));
	// `parent_index` is 1-based; `0` means "the root rule".
	assert_eq!(inner.parent_index, 1);
	assert!(inner.is_leaf);
	assert_eq!((inner.range.start, inner.range.end), (1, 2));
}

#[test]
fn nested_repeated_captures() {
	let rule: RootRule = for_rule("(?<a>(?<b>x)+)");
	let captures: Vec<(u16, usize, usize)> = captures_of(&rule, "xxx");

	assert_eq!(captures, vec![(1, 0, 3), (2, 0, 1), (2, 1, 2), (2, 2, 3)]);

	let mut data: TdfaExecution = rule.dfa.execution_data();
	assert!(rule.dfa.execute_with_captures("xxx", &mut data, &rule));
	// All inner captures point at the single outer capture.
	assert_eq!(
		Vec::from_iter(data.captures.iter().map(|cap| cap.parent_index)),
		vec![0, 1, 1, 1],
	);
}

/// Each alternative gets a distinct capture ID, even with the same name.
#[test]
fn alternation_captures_have_distinct_ids() {
	let rule: RootRule = for_rule("(?<a>(?<b>x)|(?<c>y))+");
	let captures: Vec<(u16, usize, usize)> = captures_of(&rule, "xyx");

	assert_eq!(
		captures,
		vec![
			(1, 0, 1),
			(2, 0, 1),
			(1, 1, 2),
			(3, 1, 2),
			(1, 2, 3),
			(2, 2, 3)
		],
	);
}

/// Repetition operators are greedy; the first capture takes as much as it can.
#[test]
fn greedy_repetition_captures() {
	let rule: RootRule = for_rule("(?<a>a+)(?<b>a+)");
	let captures: Vec<(u16, usize, usize)> = captures_of(&rule, "aaaa");

	assert_eq!(captures, vec![(1, 0, 3), (2, 3, 4)]);
}

/// A capture that doesn't participate in the match isn't reported.
#[test]
fn skipped_capture_is_not_reported() {
	let optional: RootRule = for_rule("a(?<a>b)?c");
	assert_eq!(captures_of(&optional, "ac"), vec![]);
	assert_eq!(captures_of(&optional, "abc"), vec![(1, 1, 2)]);

	let kleene: RootRule = for_rule("x(?<a>y)*z");
	assert_eq!(captures_of(&kleene, "xz"), vec![]);
	assert_eq!(captures_of(&kleene, "xyyz"), vec![(1, 1, 2), (1, 2, 3)]);
}

/// Capture ranges are byte offsets into the input.
#[test]
fn capture_ranges_are_byte_offsets() {
	let rule: RootRule = for_rule("(?<a>.)(?<b>.)");
	let input: &str = "\u{e9}b";
	let captures: Vec<(u16, usize, usize)> = captures_of(&rule, input);

	assert_eq!(captures, vec![(1, 0, 2), (2, 2, 3)]);
	assert_eq!(&input[0..2], "\u{e9}");
}

#[test]
fn execution_data_is_reusable() {
	let rule: RootRule = for_rule(r"(?<a>\d+)-(?<b>\d+)");
	let dfa: &Tdfa = &rule.dfa;
	let mut data: TdfaExecution = dfa.execution_data();

	assert!(dfa.execute_with_captures("12-34", &mut data, &rule));
	let first: Vec<(usize, usize)> =
		Vec::from_iter(data.captures.iter().map(|c| (c.range.start, c.range.end)));

	// A failed execution shouldn't leave stale captures behind.
	assert!(!dfa.execute_with_captures("12+34", &mut data, &rule));
	assert_eq!(data.captures, vec![]);

	assert!(dfa.execute_with_captures("12-34", &mut data, &rule));
	let second: Vec<(usize, usize)> =
		Vec::from_iter(data.captures.iter().map(|c| (c.range.start, c.range.end)));

	assert_eq!(first, second);
}

#[test]
fn captures_carry_rule_idx() {
	let rule_idx: RuleIdx = RuleIdx::new(NonZero::new(7).unwrap());
	let rule: RootRule = for_rule_with_idx(rule_idx, "(?<a>x)");
	let dfa: &Tdfa = &rule.dfa;
	let mut data: TdfaExecution = dfa.execution_data();

	assert!(dfa.execute_with_captures("x", &mut data, &rule));
	assert_eq!(data.captures.len(), 1);
	assert_eq!(data.captures[0].rule_idx, rule_idx);
}

/// Run the DFA to completion and check it lands on an accepting state,
/// as opposed to [`Tdfa::execute`],
/// which only checks the input can be consumed.
fn full_match(dfa: &Tdfa, input: &str) -> bool {
	let mut current_state: usize = 0;
	for ch in input.chars() {
		let Some(transition): Option<&Transition> =
			dfa.lookup_transition(current_state, u32::from(ch))
		else {
			return false;
		};
		current_state = transition.target;
	}
	dfa.states[current_state].accepting_rule.is_some()
}

/// We need to go through [`AnchoredRegex::from_pattern_with_placeholders`]
/// which assigns capture IDs.
#[track_caller]
fn for_rule(pattern: &str) -> RootRule {
	for_rule_with_idx(RuleIdx::NIL, pattern)
}

#[track_caller]
fn for_rule_with_idx(rule_idx: RuleIdx, pattern: &str) -> RootRule {
	let anchored: AnchoredRegex =
		AnchoredRegex::from_pattern_with_placeholders(pattern, "test".into(), &mut ()).unwrap();
	RootRule::new(rule_idx, "test".into(), 0, anchored, &[])
}

#[track_caller]
fn for_pattern(pattern: &str) -> Tdfa {
	for_rule(pattern).dfa
}

/// Returns captures as `(capture ID, start, end)` triples.
///
/// Panics if no match.
#[track_caller]
fn captures_of(rule: &RootRule, input: &str) -> Vec<(u16, usize, usize)> {
	let mut data: TdfaExecution = rule.dfa.execution_data();
	assert!(rule.dfa.execute_with_captures(input, &mut data, rule));
	Vec::from_iter(
		data.captures
			.iter()
			.map(|cap| (cap.capture_id.get(), cap.range.start, cap.range.end)),
	)
}

/// The compressed parsing DFA for a spec with a single rule.
#[track_caller]
fn compressed_for_pattern(pattern: &str) -> FlattenedDfa {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder.add_rule("rule", pattern).unwrap();
	builder.build().combined_dfa
}

/// Build a spec from its file-definition text, without round-tripping.
#[track_caller]
fn spec_of(definition: &str) -> crate::parsing_spec::ParsingSpec {
	crate::parsing_spec::ParsingSpecBuilder::from_parsing_spec_definition(definition)
		.unwrap()
		.build()
}

/// Brute-force leftmost-longest reference for [`FlattenedDfa::execute_leftmost`]:
/// try every viable start position in order with the (unflattened) parsing TDFA,
/// which itself implements longest match and priority per attempt.
///
/// Viable starts are the character boundaries in `[pos..]` up to and including
/// the position just after the first `'\n'` at or after `pos` (excluding end of input:
/// no rule may match the empty string).
fn brute_force_leftmost(
	tdfa: &Tdfa,
	input: &str,
	pos: usize,
	char_before: u32,
) -> Option<LeftmostMatch> {
	let limit: usize = input[pos..]
		.find('\n')
		.map(|i| pos + i + 1)
		.unwrap_or(input.len());

	let mut cb: u32 = char_before;
	let mut starts: Vec<usize> = Vec::from_iter(
		input
			.char_indices()
			.map(|(i, _)| i)
			.chain(std::iter::once(input.len())),
	);
	starts.retain(|&s| (pos..=limit).contains(&s) && s != input.len());
	starts.dedup();

	for s in starts {
		if let Some(m) = tdfa.execute_without_captures(&input[s..], cb) {
			return Some(LeftmostMatch {
				rule_idx: m.rule_idx,
				maybe_encoding_idx: m.maybe_encoding_idx,
				start: s,
				end: s + m.lexeme.len(),
			});
		}
		cb = u32::from(input.as_bytes()[s] as char);
	}
	None
}

/// Assert [`FlattenedDfa::execute_leftmost`] agrees with [`brute_force_leftmost`]
/// over `input` at every character boundary, in both directions of lookahead history.
#[track_caller]
fn assert_leftmost_everywhere(spec: &crate::parsing_spec::ParsingSpec, input: &str) {
	let tdfa: Tdfa = Tdfa::for_rules(&spec.rules, &spec.delimiters, &spec.encodings);
	let dfa: &FlattenedDfa = &spec.combined_dfa;
	let mut scratch: LeftmostScratch = dfa.leftmost_scratch();

	for (pos, _) in input
		.char_indices()
		.chain(std::iter::once((input.len(), ' ')))
	{
		let char_before: u32 = input[..pos]
			.chars()
			.next_back()
			.map_or(u32::from('\n'), u32::from);

		let expected: Option<LeftmostMatch> = brute_force_leftmost(&tdfa, input, pos, char_before);
		let actual: Option<LeftmostMatch> =
			dfa.execute_leftmost(input.as_bytes(), pos, char_before, &mut scratch);
		assert_eq!(actual, expected, "input={input:?} pos={pos}");
	}
}

#[test]
fn leftmost_basic_cases() {
	let spec: crate::parsing_spec::ParsingSpec = spec_of(
		r#"
		delimiters: " \t\r\n,;"

		int: "^[0-9]+"
		word: "^[a-z]+"
		hex: "0x[0-9a-f]+"
		"#,
	);

	// Unanchored `hex` matches mid-token where an anchored attempt would never start.
	assert_leftmost_everywhere(&spec, "see 0xff and 0xg");
	// Anchored rules only fire at boundaries.
	assert_leftmost_everywhere(&spec, "abc 123 abc123 123abc");
	assert_leftmost_everywhere(&spec, "12 0x9f\n34 0x10");
	assert_leftmost_everywhere(&spec, "");
	assert_leftmost_everywhere(&spec, "  , ; ");
}

/// Same start, different lengths: longest wins, even over higher priority.
/// Same start and length: earliest rule in the spec wins.
#[test]
fn leftmost_tie_breaks() {
	let spec: crate::parsing_spec::ParsingSpec = spec_of(
		r#"
		short: "ab"
		pair: "^a[bcd]$"
		long: "a."
		"#,
	);
	assert_leftmost_everywhere(&spec, "ab abc abd xab");

	let spec2: crate::parsing_spec::ParsingSpec = spec_of(
		r#"
		a: "^[ab]+$"
		b: "^(?<x>ab)$"
		"#,
	);
	// Same end: `a` (earlier in the file) wins; longer still trumps.
	assert_leftmost_everywhere(&spec2, "ab aba aba");
}

/// Priorities (integer values) select the rule order
/// that leftmost-longest falls back to last.
#[test]
fn leftmost_priority() {
	let spec: crate::parsing_spec::ParsingSpec = spec_of(
		r#"
		a (5): "x[yz]"
		b (-1): "xy"
		c: "x[ay]"
		"#,
	);
	assert_leftmost_everywhere(&spec, "xy xa xyz");
}

/// End-of-input lookahead and UTF-8 lookbehind/transitions.
#[test]
fn leftmost_boundaries() {
	let spec: crate::parsing_spec::ParsingSpec = spec_of(
		r#"
		int: "^[0-9]+$"
		any2: "^\..$"
		"#,
	);
	assert_leftmost_everywhere(&spec, "abc 12 \u{e9}4 \u{4e16}5 67");
}

/// A rule may span newlines; only its start position is window-bounded.
#[test]
fn leftmost_match_spanning_newline() {
	let spec: crate::parsing_spec::ParsingSpec = spec_of(
		r#"
		trace: "a(\nbc)+d"
		int: "^[0-9]+$"
		"#,
	);
	assert_leftmost_everywhere(&spec, "za\nbc\nbcd1 9");
}

/// Exhaustive fuzz: many small specs over a small alphabet, comparing against
/// brute force at every position of random inputs.
#[test]
fn leftmost_differential_fuzz() {
	const SPECS: &[&str] = &[
		"a: \"a+\"",
		"a: \"a+\"\nb: \"ab+\"",
		"a: \"^a+$\"\nb: \"ab\"",
		"a: \"ab\"\nb: \"b+\"",
		"a: \"a|bc\"\nb: \"^abc$\"",
		"num: \"[0-9]+\"\nword: \"[a-z]+\"\nsym: \"[^ a-z0-9\\n]+\"",
		"a: \"^a\"\nb: \"b$\"\nc: \"^c$\"",
		// Wide first sets keep multiple threads alive.
		"x: \"[a-c]+x\"\ny: \"[b-d]+y\"\nz: \"w?z\"",
	];

	let mut state: u64 = 0x9E3779B97F4A7C15;
	let mut next = move || {
		state ^= state << 13;
		state ^= state >> 7;
		state ^= state << 17;
		state
	};

	let alphabet: &[char] = &[
		'a', 'b', 'c', 'd', 'w', 'x', 'y', 'z', '0', '1', ' ', '\n', '\u{e9}',
	];

	for definition in SPECS {
		let spec: crate::parsing_spec::ParsingSpec = spec_of(definition);
		for _ in 0..200 {
			let len: usize = (next() % 40) as usize;
			let input: String =
				String::from_iter((0..len).map(|_| alphabet[(next() as usize) % alphabet.len()]));
			assert_leftmost_everywhere(&spec, &input);
		}
	}
}

/// The compressed DFA must decode non-ASCII scalars
/// identically to the char iterator it replaced:
/// both must consume a whole multi-byte scalar
/// and report its full byte length as the lexeme.
#[test]
fn compressed_execute_non_ascii_scalars() {
	let compressed: FlattenedDfa = compressed_for_pattern("a.b");

	// 2-byte scalar in the middle of an otherwise-ASCII lexeme.
	let input: &str = "a\u{e9}b";
	let matched: MatchedRule<'_> = compressed
		.execute(input.as_bytes(), u32::from('\n'))
		.unwrap();
	assert_eq!(&*matched.lexeme, input);

	// 3-byte and 4-byte scalars in the wildcard position.
	for input in ["a\u{4e16}b", "a\u{1f600}b"] {
		let matched: MatchedRule<'_> = compressed
			.execute(input.as_bytes(), u32::from('\n'))
			.unwrap();
		assert_eq!(&*matched.lexeme, input, "input={input:?}");
	}

	// A non-nullable any-char repetition consumes the entire non-ASCII input,
	// exercising every scalar width as the *first* byte decoded.
	let any: FlattenedDfa = compressed_for_pattern(".+");
	let input: &str = "\u{e9}\u{4e16}\u{1f600}";
	let matched: MatchedRule<'_> = any.execute(input.as_bytes(), u32::from('\n')).unwrap();
	assert_eq!(&*matched.lexeme, input);
}

/// Invalid UTF-8 in the input must panic.
///
/// The parser only ever sees pre-validated `&str`,
/// so this is a programming error, not a recoverable condition.
#[test]
#[should_panic(expected = "invalid UTF-8")]
fn compressed_execute_rejects_invalid_utf8() {
	let compressed: FlattenedDfa = compressed_for_pattern("a.b");
	// A lone continuation byte, which is not a valid scalar.
	let input: &[u8] = b"a\x80b";
	let _ = compressed.execute(input, u32::from('\n'));
}

/// A non-ASCII scalar at the very end of the input (and a truncated lead byte)
/// must not read past the slice;
/// the truncated case panics as invalid UTF-8 rather than panicking on an index.
#[test]
fn compressed_execute_non_ascii_at_end() {
	let compressed: FlattenedDfa = compressed_for_pattern("a.b");

	let input: &str = "a\u{e9}b";
	assert_eq!(
		compressed
			.execute(input.as_bytes(), u32::from('\n'))
			.unwrap()
			.lexeme,
		input
	);

	// A 3-byte lead with no continuation bytes is invalid UTF-8.
	let truncated: &[u8] = b"a\xe4";
	let result = std::panic::catch_unwind(|| compressed.execute(truncated, u32::from('\n')));
	assert!(result.is_err());
}
