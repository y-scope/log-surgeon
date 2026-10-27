use super::*;
use crate::parsing_spec::ParsingSpecBuilder;

fn charset_for_pattern(pattern: &str) -> Charset {
	let regex: Regex = Regex::from_pattern_with_placeholders(pattern, &mut ()).unwrap();
	let mut charset: Charset = Charset::empty();
	charset.add_regex(&regex);
	charset
}

fn assert_contains_exactly(charset: &Charset, expected: &str) {
	for c in expected.chars() {
		assert!(charset.contains(c), "expected {c:?} to be in the charset");
	}
	// Sample the rest of ASCII, plus a non-ASCII scalar value.
	for c in (0..0x80u32)
		.filter_map(char::from_u32)
		.chain(['\u{e9}', '\u{10FFFF}'])
	{
		if expected.contains(c) {
			continue;
		}
		assert!(
			!charset.contains(c),
			"expected {c:?} not to be in the charset"
		);
	}
}

#[test]
fn empty_and_universal() {
	let empty: Charset = Charset::empty();
	assert!(empty.is_empty());
	assert!(!empty.is_universal());
	assert!(!empty.contains('a'));

	let all: Charset = Charset::all();
	assert!(!all.is_empty());
	assert!(all.is_universal());
	assert!(all.contains('a'));
	assert!(all.contains('\0'));
	assert!(all.contains(char::MAX));
}

#[test]
fn insert_single_characters() {
	let mut charset: Charset = Charset::empty();
	charset.insert('a');
	charset.insert('c');
	assert_contains_exactly(&charset, "ac");
	assert!(!charset.is_universal());
}

#[test]
fn literal_and_sequence() {
	assert_contains_exactly(&charset_for_pattern("abc"), "abc");
	// A sequence contributes the union of its items' alphabets, not their concatenation.
	assert_contains_exactly(&charset_for_pattern("aab"), "ab");
}

#[test]
fn bracketed_ranges() {
	assert_contains_exactly(&charset_for_pattern("[0-9]"), "0123456789");
	assert_contains_exactly(&charset_for_pattern("[a-cx-z]"), "abcxyz");
	// Overlapping ranges merge rather than duplicate.
	assert_contains_exactly(&charset_for_pattern("[a-ca-e]"), "abcde");
}

#[test]
fn negated_bracketed_ranges_are_exact() {
	// The key precision win over a bitmap-with-universal-fallback:
	// a negated range excludes exactly its members, instead of widening to every character.
	let charset: Charset = charset_for_pattern("[^0-9]");
	assert!(!charset.is_universal());
	for c in "0123456789".chars() {
		assert!(!charset.contains(c), "expected {c:?} to be excluded");
	}
	for c in "aZ_ ".chars() {
		assert!(charset.contains(c), "expected {c:?} to be included");
	}
	assert!(charset.contains('\u{e9}'));
}

#[test]
fn negated_range_covering_everything_contains_no_char() {
	// The complement is taken over `u32`,
	// so it retains the (sound, deliberately widened) tail above `char::MAX`;
	// what matters is that no actual `char` is admitted.
	let charset: Charset = charset_for_pattern(r"[^\u{00}-\u{10FFFF}]");
	assert!(!charset.is_universal());
	assert!(!charset.contains('a'));
	assert!(!charset.contains('\0'));
	assert!(!charset.contains(char::MAX));
}

#[test]
fn adjacent_ranges_are_universal() {
	// `IntervalTree` does not coalesce adjacent entries,
	// so a set covering everything may be split across several intervals;
	// `is_universal` must still recognize it.
	let charset: Charset = charset_for_pattern(r"[\u{00}-\u{7F}]|[\u{80}-\u{10FFFF}]");
	assert!(charset.is_universal());
	assert!(charset.contains('a'));
	assert!(charset.contains(char::MAX));

	// A set with a one-character gap is not universal.
	let charset: Charset = charset_for_pattern(r"[\u{00}-\u{7F}]|[\u{81}-\u{10FFFF}]");
	assert!(!charset.is_universal());
	assert!(!charset.contains('\u{80}'));
}

#[test]
fn any_char_is_universal() {
	let charset: Charset = charset_for_pattern(".");
	assert!(charset.is_universal());
	assert!(charset.contains('\n'));
}

#[test]
fn alternation_unions_branches() {
	assert_contains_exactly(&charset_for_pattern("abc|xyz"), "abcxyz");
}

#[test]
fn repetition_recurses_into_item() {
	// Repetition cannot introduce a character the item itself could not emit.
	assert_contains_exactly(&charset_for_pattern("[ab]*"), "ab");
	assert_contains_exactly(&charset_for_pattern("[ab]+"), "ab");
	assert_contains_exactly(&charset_for_pattern("[ab]{2,5}"), "ab");
}

#[test]
fn captures_recurse_into_sub_rules() {
	assert_contains_exactly(&charset_for_pattern("a(?<num>[0-9]+)z"), "a0123456789z");
}

#[test]
fn union_of_charsets() {
	let mut charset: Charset = charset_for_pattern("[a-c]");
	charset.union(&charset_for_pattern("[x-z]"));
	assert_contains_exactly(&charset, "abcxyz");

	// Unioning with a universal set saturates.
	charset.union(&Charset::all());
	assert!(charset.is_universal());
}

#[test]
fn charset_for_name_resolves_rules() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder.add_rule("digits", "[0-9]+").unwrap();
	builder.add_rule("blockID", r"blk_(?<num>[0-9]+)").unwrap();
	let spec: ParsingSpec = builder.build();

	assert_contains_exactly(&charset_for_name(&spec, "digits"), "0123456789");
	assert_contains_exactly(&charset_for_name(&spec, "blockID"), "_bkl0123456789");
	// A sub-rule resolves to just its own alphabet.
	assert_contains_exactly(&charset_for_name(&spec, "blockID.num"), "0123456789");
}

#[test]
fn charset_for_unknown_name_is_universal() {
	let spec: ParsingSpec = ParsingSpecBuilder::new().build();
	// Never prune on a rule we could not resolve.
	assert!(charset_for_name(&spec, "nonexistent").is_universal());
}
