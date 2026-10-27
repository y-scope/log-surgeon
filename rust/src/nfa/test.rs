//! [`Tnfa`] can only be executed by creating the [`Tdfa`],
//! but these are the tests limited to "matches" behaviour.
//! In other words, tags/captures are not executed.

use super::*;
use crate::dfa::Tdfa;
use crate::regex::Regex;

#[test]
fn intersect_match() {
	let nfa1: Tnfa = for_pattern(r"\w*\d\w*");
	let nfa2: Tnfa = for_pattern(r"\d+");
	let nfa3: Tnfa = for_pattern(r"\w+");

	let intersection12: Tnfa = nfa1.intersect::<false>(&nfa2);
	let intersection13: Tnfa = nfa1.intersect::<false>(&nfa3);

	assert!(!matches(&intersection12, "a1b"));
	assert!(matches(&intersection13, "a1b"));
}

#[test]
fn literal_sequence() {
	let nfa: Tnfa = for_pattern("abc");

	assert!(matches(&nfa, "abc"));

	assert!(!matches(&nfa, ""));
	assert!(!matches(&nfa, "ab"));
	assert!(!matches(&nfa, "abcd"));
	assert!(!matches(&nfa, "xyz"));
}

#[test]
fn alternation() {
	let alternation: Tnfa = for_pattern("abc|de");
	assert!(matches(&alternation, "abc"));
	assert!(matches(&alternation, "de"));
	assert!(!matches(&alternation, "abcde"));
	assert!(!matches(&alternation, "d"));
}

#[test]
fn repetition() {
	let repetition: Tnfa = for_pattern("a(bc)+d");
	assert!(matches(&repetition, "abcd"));
	assert!(matches(&repetition, "abcbcbcd"));
	assert!(!matches(&repetition, "ad"));
	assert!(!matches(&repetition, "abcb"));
}

#[test]
fn character_classes() {
	let nfa: Tnfa = for_pattern(r"\w+[ ]\d+");
	assert!(matches(&nfa, "abc 123"));
	assert!(matches(&nfa, "A0 9"));
	assert!(!matches(&nfa, "abc 12a"));
	assert!(!matches(&nfa, "abc  123"));

	let negated: Tnfa = for_pattern("[^0-9]+");
	assert!(matches(&negated, "abc"));
	assert!(!matches(&negated, "ab3"));
}

/// Captures don't affect what an NFA matches;
/// they only add tagged transitions.
#[test]
fn captures_do_not_affect_matching() {
	let with_captures: Tnfa = for_pattern("(?<a>ab)(?<b>cd)");
	let without_captures: Tnfa = for_pattern("abcd");

	const INPUTS: &[&str] = &["abcd", "ab", "abcde", ""];

	for &input in INPUTS.iter() {
		assert_eq!(
			matches(&with_captures, input),
			matches(&without_captures, input),
			"{input:?}"
		);
	}

	// Each capture contributes an open tag and a close tag.
	assert_eq!(without_captures.tags().len(), 0);
	assert_eq!(with_captures.tags().len(), 4);
}

#[test]
fn concat() {
	let concatenated: Tnfa = for_pattern("abc").concat(&for_pattern("de"));

	assert!(matches(&concatenated, "abcde"));

	assert!(!matches(&concatenated, "abc"));
	assert!(!matches(&concatenated, "de"));
	assert!(!matches(&concatenated, "abcd"));
	assert!(!matches(&concatenated, "abcdef"));
}

#[test]
fn or() {
	let alternated: Tnfa = for_pattern("abc").alternate(&for_pattern("de"));

	assert!(matches(&alternated, "abc"));
	assert!(matches(&alternated, "de"));

	assert!(!matches(&alternated, "abcde"));
	assert!(!matches(&alternated, "ab"));
	assert!(!matches(&alternated, ""));
}

#[test]
fn intersect_is_conjunction() {
	// Digits, but exactly three of them.
	let intersection: Tnfa = for_pattern(r"\d+").intersect::<false>(&for_pattern("..."));

	assert!(matches(&intersection, "123"));

	assert!(!matches(&intersection, "12"));
	assert!(!matches(&intersection, "1234"));
	assert!(!matches(&intersection, "12a"));
}

#[test]
fn intersect_with_no_common_language() {
	let digits: Tnfa = for_pattern(r"\d+");
	let letters: Tnfa = for_pattern("[a-z]+");

	let intersection: Tnfa = digits.intersect::<false>(&letters);

	assert!(!intersection.can_accept());
	assert!(intersection.definitely_cannot_accept());
	assert!(!matches(&intersection, "1"));
	assert!(!matches(&intersection, "a"));
}

#[test]
fn epsilon_intersection_can_accept() {
	let intersection: Tnfa = Tnfa::epsilon().intersect::<false>(&Tnfa::epsilon());

	assert_eq!(intersection.states.len(), 1);
	assert!(intersection.can_accept());
	assert!(!intersection.definitely_cannot_accept());
	assert!(matches(&intersection, ""));
}

#[test]
fn definitely_cannot_accept_agrees_with_can_accept_after_intersect() {
	let nfas: [Tnfa; 5] = [
		Tnfa::epsilon(),
		for_pattern("a"),
		for_pattern(r"\d+"),
		for_pattern("[a-z]+"),
		for_pattern("..."),
	];
	for lhs in nfas.iter() {
		for rhs in nfas.iter() {
			let intersection: Tnfa = lhs.intersect::<false>(rhs);
			assert_eq!(
				intersection.definitely_cannot_accept(),
				!intersection.can_accept()
			);
		}
	}
}

#[track_caller]
fn for_pattern(pattern: &str) -> Tnfa {
	let regex: Regex = Regex::from_pattern(pattern).unwrap();
	Tnfa::for_regex(&regex)
}

/// Simulate `nfa` (via determinization) and check that `input` is matched in full.
///
/// [`Tdfa::execute`] accepts any viable prefix, and [`Tdfa::execute_without_captures`]
/// needs anchor transitions, which [`Tnfa::for_regex`] doesn't build.
/// So instead we append a terminator
/// that is only reachable from `nfa`'s accepting states;
/// consuming it is then equivalent to `input` being matched in full.
fn matches(nfa: &Tnfa, input: &str) -> bool {
	const TERMINATOR: char = '\u{0}';
	assert!(!input.contains(TERMINATOR));

	let terminated: Tnfa = nfa.concat(&Tnfa::for_regex(&Regex::Literal(TERMINATOR)));
	let dfa: Tdfa = Tdfa::determinization::<false>(&terminated);
	dfa.execute(&format!("{input}{TERMINATOR}"))
}
