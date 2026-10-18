use super::*;
use crate::parsing_spec::ParsingSpec;

/// Compact rendering of the whole token stream, e.g. `V(int:12)` / `S(xy)` / `NL` / `END`.
fn tokens_of(spec: &ParsingSpec, input: &str) -> Vec<String> {
	let spec: Arc<ParsingSpec> = Arc::new(spec.clone());
	let mut lexer: Lexer = Lexer::new(spec.clone());
	let mut registers: usize = 0;
	let mut tags: usize = 0;
	for rule in spec.rules.iter() {
		registers = registers.max(rule.dfa.number_of_registers);
		tags = tags.max(rule.dfa.tags.len());
	}
	let mut execution: TdfaExecution = TdfaExecution::new(registers, tags);

	let mut out: Vec<String> = Vec::new();
	let mut pos: usize = 0;
	loop {
		match lexer.next_token(input.as_bytes(), &mut pos, &mut execution) {
			Token::Variable { rule, lexeme, .. } => {
				let rule_name: &str = rule.name.as_ref();
				out.push(format!("V({rule_name}:{lexeme})"));
			},
			Token::Newline => out.push("NL".to_owned()),
			Token::StaticText(text) => out.push(format!("S({text})")),
			Token::EndOfInput => {
				out.push("END".to_owned());
				break;
			},
		}
	}
	out
}

/// An unanchored rule matches even away from delimiters
/// (the old lexer could only match at the current position or just after a delimiter).
#[test]
fn leftmost_finds_unanchored_match_mid_gap() {
	let spec: ParsingSpec = crate::spec! {
		"int: \"[0-9]+\""
	};
	assert_eq!(
		tokens_of(&spec, "x1 y"),
		vec!["S(x)", "V(int:1)", "S( y)", "END"]
	);
}

/// Anchored and unanchored variants of the same pattern:
/// the anchored one only fires at boundaries; ties go to the earlier rule.
#[test]
fn anchored_vs_unanchored() {
	let spec: ParsingSpec = crate::spec! {
		"
		delimiters: \" \\t\\r\\n\"
		a: \"^[0-9]+$\"
		b: \"[0-9]+\"
		"
	};
	assert_eq!(
		tokens_of(&spec, "z12 34"),
		vec!["S(z)", "V(b:12)", "S( )", "V(a:34)", "END"]
	);
}

/// An end-anchored-only rule, matched at end of input via the lookahead.
#[test]
fn end_anchor_at_input_end() {
	let spec: ParsingSpec = crate::spec! {
		"t: \"end$\""
	};
	assert_eq!(tokens_of(&spec, "xxend"), vec!["S(xx)", "V(t:end)", "END"]);
}

/// Integer priorities break same-length ties before file order.
#[test]
fn priority_breaks_ties() {
	let spec: ParsingSpec = crate::spec! {
		"
		lo: \"ab\"
		hi (5): \"a.\"
		"
	};
	assert_eq!(tokens_of(&spec, "ab"), vec!["V(hi:ab)", "END"]);
}

/// Longer beats earlier/lower-priority order.
#[test]
fn longest_beats_order() {
	let spec: ParsingSpec = crate::spec! {
		"
		shorter: \"ab\"
		longer: \"a..\"
		"
	};
	assert_eq!(tokens_of(&spec, "abc"), vec!["V(longer:abc)", "END"]);
}

/// Gaps may contain newlines (chunked into their own tokens)
/// before a later match; nothing may start past the first newline.
#[test]
fn gap_newlines_chunked() {
	let spec: ParsingSpec = crate::spec! {
		"n: \"[0-9]+\""
	};
	assert_eq!(
		tokens_of(&spec, "ab\ncd\n12"),
		vec!["S(ab)", "NL", "S(cd)", "NL", "V(n:12)", "END"]
	);
	// Nothing may start at newline *or later* in the same line;
	// and a position right after a newline is a viable start.
	assert_eq!(
		tokens_of(&spec, "9\n9"),
		vec!["V(n:9)", "NL", "V(n:9)", "END"]
	);
}

/// A rule may span newlines; no newline token is produced inside a match.
#[test]
fn match_spanning_newline() {
	let spec: ParsingSpec = crate::spec! {
		"t: \"a(\\nbc)+d\""
	};
	assert_eq!(
		tokens_of(&spec, "za\nbcd"),
		vec!["S(z)", "V(t:a\nbcd)", "END"]
	);
}

/// Multi-byte characters in gaps and lookbehinds.
#[test]
fn utf8_gap_and_lookbehind() {
	let spec: ParsingSpec = crate::spec! {
		"
		delimiters: \" \\t\\r\\n\"
		n: \"[0-9]+\"
		x: \"^x\"
		"
	};
	assert_eq!(
		tokens_of(&spec, "\u{e9} \u{4e16}1"),
		vec!["S(\u{e9} \u{4e16})", "V(n:1)", "END"]
	);
	// `^x` may not match after a non-delimiter, even a multi-byte one.
	assert_eq!(tokens_of(&spec, "\u{e9}x"), vec!["S(\u{e9}x)", "END"]);
}

/// A spec with no rules lexes everything as static text.
#[test]
fn empty_spec() {
	let spec: ParsingSpec = crate::spec! { "" };
	assert_eq!(
		tokens_of(&spec, "ab\ncd"),
		vec!["S(ab)", "NL", "S(cd)", "END"]
	);
}
