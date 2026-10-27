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
				out.push(format!(
					"V({}:{})",
					rule.name.escape_default(),
					lexeme.escape_default()
				));
			},
			Token::Newline => out.push("NL".to_owned()),
			Token::StaticText(text) => out.push(format!("S({})", text.escape_default())),
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
		vec!["S(z)", "V(t:a\\nbcd)", "END"]
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
		vec!["S(\\u{e9} \\u{4e16})", "V(n:1)", "END"]
	);
	// `^x` may not match after a non-delimiter, even a multi-byte one.
	assert_eq!(tokens_of(&spec, "\u{e9}x"), vec!["S(\\u{e9}x)", "END"]);
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

/// Reference token stream: the pre-split engine, i.e. [`Lexer`] behaviour as if
/// `ParsingSpec::unanchored_scan` were always `None` (the combined-DFA leftmost engine).
fn reference_tokens_of(spec: &ParsingSpec, input: &str) -> Vec<String> {
	let bytes: &[u8] = input.as_bytes();
	let mut scratch: crate::dfa::LeftmostScratch = spec.combined_dfa.leftmost_scratch();
	let mut pending: Option<crate::dfa::LeftmostMatch> = None;
	let mut out: Vec<String> = Vec::new();
	let mut pos: usize = 0;

	let static_chunk =
		|out: &mut Vec<String>, pos: &mut usize, end: usize| match input[*pos..end].find('\n') {
			Some(0) => {
				out.push("NL".to_owned());
				*pos += 1;
			},
			Some(k) => {
				out.push(format!("S({})", input[*pos..*pos + k].escape_default()));
				*pos += k;
			},
			None => {
				out.push(format!("S({})", input[*pos..end].escape_default()));
				*pos = end;
			},
		};

	loop {
		if let Some(m) = pending {
			if pos == m.start {
				pending = None;
				let rule: &RootRule = &spec[m.rule_idx];
				out.push(format!(
					"V({}:{})",
					rule.name.escape_default(),
					input[m.start..m.end].escape_default()
				));
				pos = m.end;
			} else {
				static_chunk(&mut out, &mut pos, m.start);
			}
			continue;
		}
		if pos == bytes.len() {
			out.push("END".to_owned());
			break;
		}
		if bytes[pos] == b'\n' {
			out.push("NL".to_owned());
			pos += 1;
			continue;
		}
		let char_before: u32 = u32::from(
			crate::utils::utf8::Utf8Chars::new(&bytes[..pos])
				.next_back()
				.unwrap_or('\n'),
		);
		match spec
			.combined_dfa
			.execute_leftmost(bytes, pos, char_before, &mut scratch)
		{
			None => {
				assert!(pos < bytes.len());
				// One chunk only; the newline (if any) is emitted next,
				// and the search restarts after it.
				static_chunk(&mut out, &mut pos, bytes.len());
			},
			Some(m) if m.start == pos => {
				let rule: &RootRule = &spec[m.rule_idx];
				out.push(format!(
					"V({}:{})",
					rule.name.escape_default(),
					input[m.start..m.end].escape_default()
				));
				pos = m.end;
			},
			Some(m) => {
				pending = Some(m);
			},
		}
	}
	out
}

/// The split engine (delimiter/first-byte scans + per-DFA single-threaded `execute`)
/// must produce the same token stream as the combined-DFA reference.
#[test]
fn split_engine_differential() {
	const SPECS: &[&str] = &[
		// Unanchored rule with a rare first byte (spec4's `javaHash` shape).
		"word: \"^[a-z]+$\"\nhash: \"@[0-9a-f]+\"",
		// Anchored and unanchored rules competing for the same positions.
		"a: \"^[0-9]+$\"\nb: \"[0-9]+\"\nc: \"^x[0-9]*$\"",
		// The unanchored first-byte set is dense/inexact:
		// the spec falls back to the combined-DFA engine, so both sides agree trivially.
		"a: \"^foo$\"\nb: \"[^ ab0-9].\"",
		// A rule spanning a newline (first byte `a`), plus an anchored rule.
		"t: \"a(\\nbc)+d\"\nn: \"^[0-9]+$\"",
		// Non-ASCII first bytes and lookbehinds.
		"e: \"\u{e9}\u{e9}\"\nu: \"^\u{4e16}$\"",
		// A non-ASCII unanchored rule next to a wide ASCII one:
		// only exact with an exact `initial_bytes` (27 bytes, not 78).
		"x: \"[a-z]+\"\ne: \"\u{e9}+\"\nz: \"^[0-9]+$\"",
		// Non-delimiters sharing the lead byte of a non-ASCII delimiter (`\u{3001}`),
		// and an inexact `scan` (delimiters spanning more than 8 high nibbles).
		INEXACT_SCAN,
		// As above, with rules that can match at false-positive hits.
		"delimiters: \" \\t\\n:@[`{\\u{e9}\\u{3000}\"\nh: \"#[0-9]+\"\nn: \"^[0-9\\u{03}]+$\"\nm: \"^\\u{9577}+\"",
	];
	let mut state: u64 = 0x243F6A8885A308D3;
	let mut next = move || {
		state ^= state << 13;
		state ^= state >> 7;
		state ^= state << 17;
		state
	};
	let alphabet: &[char] = &[
		'a', 'b', 'c', 'd', 'e', 'f', 'o', 'x', 'z', '@', '#', '0', '1', '9', ' ', '\t', '\n', '.',
		':', '{', '\u{3}', '\u{e8}', '\u{e9}', '\u{3000}', '\u{3001}', '\u{4e16}', '\u{9577}',
		'\u{1f600}',
	];
	for definition in SPECS {
		let spec: ParsingSpec = crate::spec! { definition };
		assert!(
			spec.unanchored_scan.is_some() || definition.contains("[^ ab0-9]"),
			"expected the split path for {definition:?}",
		);
		for _ in 0..300 {
			let len: usize = (next() % 50) as usize;
			let input: String =
				String::from_iter((0..len).map(|_| alphabet[(next() as usize) % alphabet.len()]));
			assert_eq!(
				tokens_of(&spec, &input),
				reference_tokens_of(&spec, &input),
				"definition={definition:?} input={input:?}",
			);
		}
	}
}

/// Delimiters spanning more than 8 high nibbles (`0`, `2`-`7`, `C`, `E`),
/// so [`ParsingSpec::scan`] is not exact; with one sparse unanchored rule (`#...`),
/// so the split engine is used.
const INEXACT_SCAN: &str =
	"delimiters: \" \\t\\n:@[`{\\u{e9}\\u{3000}\"\nh: \"#[0-9]+\"\nn: \"^[0-9]+$\"";

/// Scan hits that are neither delimiters nor first bytes of an unanchored rule
/// are skipped, not probed as such (this used to fail a `debug_assert!`):
///
/// - `\u{3001}` shares its lead byte `E3` with the delimiter `\u{3000}`;
/// - `\u{3}` and `\u{9577}` (lead byte `E9`) are only hits because `scan` is not exact.
#[test]
fn false_positive_scan_hits() {
	let spec: ParsingSpec = crate::spec! { INEXACT_SCAN };
	assert!(spec.unanchored_scan.is_some());
	assert!(!spec.scan.is_exact());
	for byte in [0x03, 0xE3, 0xE9] {
		assert!(spec.scan.contains(byte), "{byte:#04x} is a scan hit");
	}

	assert_eq!(
		tokens_of(&spec, "a\u{3001}12\u{3001}#3"),
		vec!["S(a\\u{3001}12\\u{3001})", "V(h:#3)", "END"]
	);
	assert_eq!(
		tokens_of(&spec, "ab\u{3}cd 12"),
		vec!["S(ab\\u{3}cd )", "V(n:12)", "END"]
	);
	// `\u{9577}` is not a delimiter, so `^[0-9]+$` cannot match right after it.
	assert_eq!(
		tokens_of(&spec, "\u{9577} 1 \u{9577}2#4"),
		vec!["S(\\u{9577} )", "V(n:1)", "S( \\u{9577}2)", "V(h:#4)", "END"]
	);
}
