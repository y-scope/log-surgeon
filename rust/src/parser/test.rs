use super::*;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::regex::Regex;

#[test]
fn ip_address() {
	let mut parser: Parser = parser! {
		r#"
		ip_address: "\d+\.\d+\.\d+\.\d+"
		"#
	};

	let input: &str = "hello 1.2.3.4";
	let mut pos: usize = 0;

	{
		let event: LogEvent<'_> = parser.next_event(input, &mut pos).unwrap();
		assert_eq!(&*event.message, "hello 1.2.3.4");
		assert_eq!(event.all_matches.len(), 1);
		assert_eq!(
			event.all_matches[0].range,
			CRange::new("hello ".len(), input.len())
		);
	}

	assert_eq!(pos, input.len());
}

/// Assert the structural invariants of an event, and render it one line per match:
/// `#i fully.qualified.name "lexeme" parent=#p [leaf] [enc=n]`,
/// preceded by a line with the event's message.
///
/// Checked:
///
/// - the documented sort order ([`LogEvent::check_invariants`]);
/// - `root_indices` and `leaf_indices` list exactly the roots and leaves, in order;
/// - a root is its own parent; a sub-rule's parent comes earlier, contains its range,
///   and is the parent capture in the spec;
/// - `is_leaf` agrees with the spec;
/// - the FFI pointers (parent, lexeme, names) agree with the safe fields.
fn render(event: &LogEvent<'_>) -> Vec<String> {
	event.check_invariants();

	let matches: &[Match] = event.all_matches.as_slice();
	let mut lines: Vec<String> = vec![format!("{:?}", event.message.as_str())];
	let mut roots: Vec<usize> = Vec::new();
	let mut leaves: Vec<usize> = Vec::new();

	for (i, mat) in matches.iter().enumerate() {
		let info: &RuleInfo = &event.spec[mat.rule_idx][mat.sub_rule_id];
		assert_eq!(mat.is_leaf, info.is_leaf(), "is_leaf of #{i}");

		if mat.is_root() {
			assert_eq!(mat.parent_index, i, "root #{i} is its own parent");
			assert!(mat.ffi_pointers.parent.is_null());
			roots.push(i);
		} else {
			assert!(mat.parent_index < i, "parent of #{i} comes first");
			let parent: &Match = &matches[mat.parent_index];
			assert_eq!(parent.rule_idx, mat.rule_idx);
			assert_eq!(
				parent.sub_rule_id.map_or(0, NonZero::get),
				info.parent_id,
				"parent of #{i} is its parent capture",
			);
			assert!(parent.range.start <= mat.range.start && mat.range.end <= parent.range.end);
			assert!(std::ptr::eq(mat.ffi_pointers.parent, parent));
		}
		if mat.is_leaf {
			leaves.push(i);
		}

		let lexeme: &str = &event.message[mat.range.start..mat.range.end];
		// SAFETY: the event (and so the parser's buffers) is borrowed for this whole function.
		unsafe {
			assert_eq!(mat.ffi_pointers.lexeme.as_str(), lexeme);
			assert_eq!(mat.ffi_pointers.rule_name.as_str(), &*info.name);
			assert_eq!(
				mat.ffi_pointers.fully_qualified_name.as_str(),
				&*info.fully_qualified_name
			);
		}

		let mut line: String = format!(
			"#{i} {} {lexeme:?} parent=#{}",
			info.fully_qualified_name, mat.parent_index
		);
		if mat.is_leaf {
			line.push_str(" leaf");
		}
		if let Some(encoding_idx) = mat.encoding_idx {
			line.push_str(&format!(" enc={encoding_idx}"));
		}
		lines.push(line);
	}

	assert_eq!(event.root_indices.as_slice(), roots.as_slice());
	assert_eq!(event.leaf_indices.as_slice(), leaves.as_slice());

	lines
}

/// Parse all of `input`, rendering every event with [`render`].
/// If `reset_after_each`, [`Parser::reset`] is called after every event,
/// and parsing resumes from the returned position.
///
/// Also checks that the events tile the input exactly,
/// i.e. each call's `*pos` is where the next event starts.
fn parse_all(parser: &mut Parser, input: &str, reset_after_each: bool) -> Vec<Vec<String>> {
	let mut events: Vec<Vec<String>> = Vec::new();
	let mut pos: usize = 0;
	loop {
		let start: usize = pos;
		let Some(event): Option<LogEvent<'_>> = parser.next_event(input, &mut pos) else {
			break;
		};
		assert_eq!(event.message.as_str(), &input[start..pos]);
		events.push(render(&event));
		if reset_after_each {
			parser.reset();
		}
	}
	assert_eq!(pos, input.len());
	assert!(parser.next_event(input, &mut pos).is_none());
	assert_eq!(pos, input.len());
	events
}

/// A `header` rule with nested, repeated, and alternated captures, and an encoding
/// on some of its leaves.
fn spec_with_structured_header() -> Arc<ParsingSpec> {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder
		.set_delimiters(" \n")
		.add_rule(
			"header",
			concat!(
				r"^(?<ts>(?<y>[0-9]{4})-(?<m>[0-9]{2}))",
				r"(:(?<tag>[a-z]))*",
				r"(?<lvl>(?<i>I)|(?<w>W)|(?<n>[0-9]+))$",
			),
		)
		.unwrap()
		.add_rule("w", r"^[a-z]+$")
		.unwrap()
		.add_rule("x", r"^(?<v>[0-9]+)$")
		.unwrap()
		.add_encoding("int", Regex::from_pattern(r"[0-9]+").unwrap())
		.unwrap();
	Arc::new(builder.build())
}

/// The header that separates two events belongs to the second.
/// Its captures are relative to that event and their parents index into that event,
/// however the header got there (found while parsing the previous event, see
/// [`crate::lexer::Lexer::unread`]).
///
/// Covers a continuation line, a repeated capture (`tag`),
/// each alternative of `lvl`, encodings on header leaves,
/// and a header that is the last thing in the input (no trailing newline).
#[test]
fn header_with_captures_separates_events() {
	let spec: Arc<ParsingSpec> = spec_with_structured_header();
	let input: &str = "2020-01I a 7\n2020-02:p:qW b\nc 8\n2020-0342 d\n2020-04:zW";

	let events: Vec<Vec<String>> = parse_all(&mut spec.create_parser(), input, false);

	let expected: Vec<Vec<&str>> = vec![
		vec![
			r#""2020-01I a 7\n""#,
			r#"#0 header "2020-01I" parent=#0"#,
			r#"#1 header.ts "2020-01" parent=#0"#,
			r#"#2 header.ts.y "2020" parent=#1 leaf enc=1"#,
			r#"#3 header.ts.m "01" parent=#1 leaf enc=1"#,
			r#"#4 header.lvl "I" parent=#0"#,
			r#"#5 header.lvl.i "I" parent=#4 leaf"#,
			r#"#6 w "a" parent=#6 leaf"#,
			r#"#7 x "7" parent=#7"#,
			r#"#8 x.v "7" parent=#7 leaf enc=1"#,
		],
		vec![
			r#""2020-02:p:qW b\nc 8\n""#,
			r#"#0 header "2020-02:p:qW" parent=#0"#,
			r#"#1 header.ts "2020-02" parent=#0"#,
			r#"#2 header.ts.y "2020" parent=#1 leaf enc=1"#,
			r#"#3 header.ts.m "02" parent=#1 leaf enc=1"#,
			r#"#4 header.tag "p" parent=#0 leaf"#,
			r#"#5 header.tag "q" parent=#0 leaf"#,
			r#"#6 header.lvl "W" parent=#0"#,
			r#"#7 header.lvl.w "W" parent=#6 leaf"#,
			r#"#8 w "b" parent=#8 leaf"#,
			r#"#9 w "c" parent=#9 leaf"#,
			r#"#10 x "8" parent=#10"#,
			r#"#11 x.v "8" parent=#10 leaf enc=1"#,
		],
		vec![
			r#""2020-0342 d\n""#,
			r#"#0 header "2020-0342" parent=#0"#,
			r#"#1 header.ts "2020-03" parent=#0"#,
			r#"#2 header.ts.y "2020" parent=#1 leaf enc=1"#,
			r#"#3 header.ts.m "03" parent=#1 leaf enc=1"#,
			r#"#4 header.lvl "42" parent=#0"#,
			r#"#5 header.lvl.n "42" parent=#4 leaf enc=1"#,
			r#"#6 w "d" parent=#6 leaf"#,
		],
		vec![
			r#""2020-04:zW""#,
			r#"#0 header "2020-04:zW" parent=#0"#,
			r#"#1 header.ts "2020-04" parent=#0"#,
			r#"#2 header.ts.y "2020" parent=#1 leaf enc=1"#,
			r#"#3 header.ts.m "04" parent=#1 leaf enc=1"#,
			r#"#4 header.tag "z" parent=#0 leaf"#,
			r#"#5 header.lvl "W" parent=#0"#,
			r#"#6 header.lvl.w "W" parent=#5 leaf"#,
		],
	];
	assert_eq!(events, expected);
}

/// Resetting after every event and resuming from the returned position
/// (as when refilling a buffer) gives the same events:
/// the header is found again instead of replayed from the lexer.
///
/// Header mode is forgotten on reset,
/// but every resumed event starts with a header, which re-enables it.
#[test]
fn reset_and_resume_at_header() {
	let spec: Arc<ParsingSpec> = spec_with_structured_header();
	let input: &str = "2020-01I a 7\n2020-02:p:qW b\nc 8\n2020-0342 d\n2020-04:zW";

	let uninterrupted: Vec<Vec<String>> = parse_all(&mut spec.create_parser(), input, false);
	let with_resets: Vec<Vec<String>> = parse_all(&mut spec.create_parser(), input, true);
	assert_eq!(with_resets, uninterrupted);
}

/// A header that is the last thing in the input is still its own event,
/// for a capture-less (leaf, encoded) header, with or without a trailing newline.
#[test]
fn trailing_header_is_an_event() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder
		.set_delimiters(" \n")
		.add_rule("header", r"^[0-9]+$")
		.unwrap()
		.add_rule("w", r"^[a-z]+$")
		.unwrap()
		.add_encoding("int", Regex::from_pattern(r"[0-9]+").unwrap())
		.unwrap();
	let spec: Arc<ParsingSpec> = Arc::new(builder.build());

	let events: Vec<Vec<String>> = parse_all(&mut spec.create_parser(), "1 a\n22", false);
	assert_eq!(
		events,
		vec![
			vec![
				r#""1 a\n""#,
				r#"#0 header "1" parent=#0 leaf enc=1"#,
				r#"#1 w "a" parent=#1 leaf"#,
			],
			vec![r#""22""#, r#"#0 header "22" parent=#0 leaf enc=1"#],
		],
	);

	let events: Vec<Vec<String>> = parse_all(&mut spec.create_parser(), "1 a\n22\n", false);
	assert_eq!(
		events,
		vec![
			vec![
				r#""1 a\n""#,
				r#"#0 header "1" parent=#0 leaf enc=1"#,
				r#"#1 w "a" parent=#1 leaf"#,
			],
			vec![r#""22\n""#, r#"#0 header "22" parent=#0 leaf enc=1"#],
		],
	);
}

/// After [`Parser::reset`], nothing from the previous input carries over,
/// in particular not the next event's header that the lexer was holding.
#[test]
fn reset_between_inputs() {
	let spec: Arc<ParsingSpec> = spec_with_structured_header();
	let mut parser: Parser = spec.create_parser();

	let mut pos: usize = 0;
	let input: &str = "2020-01I a\n2020-02W b";
	let _ = parser.next_event(input, &mut pos).unwrap();
	// The second header has been found and is pending in the lexer.
	assert_eq!(pos, "2020-01I a\n".len());

	parser.reset();
	assert_eq!(
		parse_all(&mut parser, "x y", false),
		vec![vec![
			r#""x y""#,
			r#"#0 w "x" parent=#0 leaf"#,
			r#"#1 w "y" parent=#1 leaf"#,
		]],
	);
}

/// Parse all of `input` with a parser for the spec `definition`, see [`parse_all`].
/// Also checks that resetting after every event gives the same events.
fn parse_with(definition: &str, input: &str) -> Vec<Vec<String>> {
	let events: Vec<Vec<String>> = parse_all(&mut crate::parser! { definition }, input, false);
	let with_resets: Vec<Vec<String>> = parse_all(&mut crate::parser! { definition }, input, true);
	assert_eq!(
		with_resets, events,
		"resetting after every event changed the events"
	);
	events
}

/// A `header` only separates events when it follows a newline (or starts the input);
/// e.g. timestamps embedded in a message (`[...]` after `Diagnostics:`)
/// are ordinary matches, even at the start of a continuation line
/// when they are preceded by another character (`[`).
///
/// Before the first separating header, events are separated on newlines.
/// Blank lines belong to the preceding event.
#[test]
fn header_not_after_newline_is_an_ordinary_match() {
	let definition: &str = r#"
		delimiters: " \t\r\n[]"
		header (0): "^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}[,.][0-9]+$"
		int (0): "^-?[0-9]+$"
	"#;
	let input: &str = concat!(
		"pre 1\n",
		"2018-06-19 12:07:57,096 Diagnostics: [2018-06-19 12:07:57.081]killed\n",
		"[2018-06-19 12:07:57.088]Exit code is 143\n",
		"2018-06-19 12:07:58,000 done\n",
		"\n",
	);
	assert_eq!(
		parse_with(definition, input),
		vec![
			vec![r#""pre 1\n""#, r#"#0 int "1" parent=#0 leaf"#],
			vec![
				concat!(
					r#""2018-06-19 12:07:57,096 Diagnostics: [2018-06-19 12:07:57.081]killed\n"#,
					r#"[2018-06-19 12:07:57.088]Exit code is 143\n""#,
				),
				r#"#0 header "2018-06-19 12:07:57,096" parent=#0 leaf"#,
				r#"#1 header "2018-06-19 12:07:57.081" parent=#1 leaf"#,
				r#"#2 header "2018-06-19 12:07:57.088" parent=#2 leaf"#,
				r#"#3 int "143" parent=#3 leaf"#,
			],
			vec![
				r#""2018-06-19 12:07:58,000 done\n\n""#,
				r#"#0 header "2018-06-19 12:07:58,000" parent=#0 leaf"#,
			],
		],
	);
}

/// A non-separating `header` (not after a newline) does not enable header mode;
/// neither does anything else, so a blank line is then its own (empty) event.
/// `\r` is not special: in `\r\n` only the `\n` ends a line.
#[test]
fn header_mode_starts_at_first_separating_header() {
	let definition: &str = r#"
		delimiters: " \t\r\n"
		header: "^H$"
		w: "^[a-z]+$"
	"#;
	assert_eq!(
		parse_with(definition, "a H\nb\n\nH c\nd\r\nH e\r\n\n"),
		vec![
			vec![
				r#""a H\n""#,
				r#"#0 w "a" parent=#0 leaf"#,
				r#"#1 header "H" parent=#1 leaf"#,
			],
			vec![r#""b\n""#, r#"#0 w "b" parent=#0 leaf"#],
			vec![r#""\n""#],
			vec![
				r#""H c\nd\r\n""#,
				r#"#0 header "H" parent=#0 leaf"#,
				r#"#1 w "c" parent=#1 leaf"#,
				r#"#2 w "d" parent=#2 leaf"#,
			],
			vec![
				r#""H e\r\n\n""#,
				r#"#0 header "H" parent=#0 leaf"#,
				r#"#1 w "e" parent=#1 leaf"#,
			],
		],
	);
}

/// Only a match of the rule *named* `header` separates events:
/// at a line start where another rule matches longer (`kv`),
/// or where an earlier rule with the same pattern wins the tie (`int`),
/// there is no header, so the line is a continuation (header mode)
/// or header mode is never entered (newline mode).
/// An integer priority lets `header` win the tie instead.
#[test]
fn header_must_win_the_match_to_separate() {
	let longer: &str = r#"
		delimiters: " \t\r\n"
		header: "^[0-9]+$"
		kv: "^[0-9]+:[a-z]+$"
	"#;
	assert_eq!(
		parse_with(longer, "1 a\n2:b c\n3 d"),
		vec![
			vec![
				r#""1 a\n2:b c\n""#,
				r#"#0 header "1" parent=#0 leaf"#,
				r#"#1 kv "2:b" parent=#1 leaf"#,
			],
			vec![r#""3 d""#, r#"#0 header "3" parent=#0 leaf"#],
		],
	);

	let tie: &str = r#"
		delimiters: " \t\r\n"
		int: "^[0-9]+$"
		header: "^[0-9]+$"
	"#;
	assert_eq!(
		parse_with(tie, "1 a\n2 b"),
		vec![
			vec![r#""1 a\n""#, r#"#0 int "1" parent=#0 leaf"#],
			vec![r#""2 b""#, r#"#0 int "2" parent=#0 leaf"#],
		],
	);

	let prioritized: &str = r#"
		delimiters: " \t\r\n"
		int: "^[0-9]+$"
		header (1): "^[0-9]+$"
	"#;
	assert_eq!(
		parse_with(prioritized, "1 a\n2 b"),
		vec![
			vec![r#""1 a\n""#, r#"#0 header "1" parent=#0 leaf"#],
			vec![r#""2 b""#, r#"#0 header "2" parent=#0 leaf"#],
		],
	);
}

/// A match spanning newlines (`stackTrace`) does not end the event in newline mode;
/// the event ends at the first newline *outside* a match.
#[test]
fn match_spanning_newlines_stays_in_one_event() {
	let definition: &str = concat!(
		r#"
		delimiters: " \t\r\n:"
		stackTrace (0): "^(?<stackTrace>"#,
		r#"[a-zA-Z][a-zA-Z0-9_$]*(\.[a-zA-Z][a-zA-Z0-9_$]*)+"#,
		r#"(:[^\n]+)?"#,
		r#"(\n\t+at [^\n]+)+"#,
		r#")$"
		"#,
	);
	let input: &str = concat!(
		"x java.io.IOException: boom\n",
		"\tat a.B.c(B.java:1)\n",
		"\tat a.B.d(B.java:2)\n",
		"next\n",
	);
	assert_eq!(
		parse_with(definition, input),
		vec![
			vec![
				concat!(
					r#""x java.io.IOException: boom\n"#,
					r#"\tat a.B.c(B.java:1)\n"#,
					r#"\tat a.B.d(B.java:2)\n""#,
				),
				concat!(
					r#"#0 stackTrace "java.io.IOException: boom\n"#,
					r#"\tat a.B.c(B.java:1)\n"#,
					r#"\tat a.B.d(B.java:2)" parent=#0"#,
				),
				concat!(
					r#"#1 stackTrace.stackTrace "java.io.IOException: boom\n"#,
					r#"\tat a.B.c(B.java:1)\n"#,
					r#"\tat a.B.d(B.java:2)" parent=#0 leaf"#,
				),
			],
			vec![r#""next\n""#],
		],
	);
}

/// Gotcha: a newline consumed by a match is not "a newline" for event separation,
/// so a `header` right after it does not separate events.
#[test]
fn header_after_newline_inside_a_match_does_not_separate() {
	let definition: &str = r#"
		delimiters: " \t\r\n"
		t: "^a\n"
		header: "^H$"
	"#;
	assert_eq!(
		parse_with(definition, "H x a\nH y"),
		vec![vec![
			r#""H x a\nH y""#,
			r#"#0 header "H" parent=#0 leaf"#,
			r#"#1 t "a\n" parent=#1 leaf"#,
			r#"#2 header "H" parent=#2 leaf"#,
		]],
	);
}

/// A start-unanchored rule (`javaHash`) matches mid-token,
/// and starting at the `@` delimiter it beats the anchored `objectHash`
/// that could start right after it (earliest start wins over rule order),
/// whether the `@` follows a non-delimiter or a delimiter.
#[test]
fn start_unanchored_rule_wins_by_starting_earlier() {
	let definition: &str = r#"
		delimiters: " \t\r\n()@"
		objectHash (0): "^(?<objectHash>[a-fA-F][0-9a-fA-F]{7}[0-9a-fA-F]*)$"
		javaHash (0): "@(?<javaHash>[0-9a-fA-F]+)$"
	"#;
	assert_eq!(
		parse_with(
			definition,
			"Container ServletHandler@456d6c1e + x\n(Foo)@3b84793)\nabcdef01 @abcdef01"
		),
		vec![
			vec![
				r#""Container ServletHandler@456d6c1e + x\n""#,
				r#"#0 javaHash "@456d6c1e" parent=#0"#,
				r#"#1 javaHash.javaHash "456d6c1e" parent=#0 leaf"#,
			],
			vec![
				r#""(Foo)@3b84793)\n""#,
				r#"#0 javaHash "@3b84793" parent=#0"#,
				r#"#1 javaHash.javaHash "3b84793" parent=#0 leaf"#,
			],
			vec![
				r#""abcdef01 @abcdef01""#,
				r#"#0 objectHash "abcdef01" parent=#0"#,
				r#"#1 objectHash.objectHash "abcdef01" parent=#0 leaf"#,
				r#"#2 javaHash "@abcdef01" parent=#2"#,
				r#"#3 javaHash.javaHash "abcdef01" parent=#2 leaf"#,
			],
		],
	);
}

/// End-unanchored rules (`numWithUnit`, `hostPort`) still match
/// before sentence-final punctuation (`.` is not a delimiter),
/// or any other non-delimiter (`12bytesx`);
/// and as the longer match they win over `int_fallback` from the same start.
#[test]
fn end_unanchored_rules_match_before_punctuation() {
	let definition: &str = r#"
		delimiters: " \t\r\n:"
		numWithUnit (0): "^(?<num>-?[0-9]+)[ ]?(?<unit>msecs|msec|bytes/s|bytes)"
		hostPort (0): "^(?<host>[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}):(?<port>[0-9]+)"
		int_fallback (0): "^-?[0-9]+$"
	"#;
	let input: &str = concat!(
		"from 172.18.0.22:39164 Wrote 166 bytes.\n",
		"client 172.18.0.17:46508. Number 7\n",
		"12bytesx 5msecs",
	);
	assert_eq!(
		parse_with(definition, input),
		vec![
			vec![
				r#""from 172.18.0.22:39164 Wrote 166 bytes.\n""#,
				r#"#0 hostPort "172.18.0.22:39164" parent=#0"#,
				r#"#1 hostPort.host "172.18.0.22" parent=#0 leaf"#,
				r#"#2 hostPort.port "39164" parent=#0 leaf"#,
				r#"#3 numWithUnit "166 bytes" parent=#3"#,
				r#"#4 numWithUnit.num "166" parent=#3 leaf"#,
				r#"#5 numWithUnit.unit "bytes" parent=#3 leaf"#,
			],
			vec![
				r#""client 172.18.0.17:46508. Number 7\n""#,
				r#"#0 hostPort "172.18.0.17:46508" parent=#0"#,
				r#"#1 hostPort.host "172.18.0.17" parent=#0 leaf"#,
				r#"#2 hostPort.port "46508" parent=#0 leaf"#,
				r#"#3 int_fallback "7" parent=#3 leaf"#,
			],
			vec![
				r#""12bytesx 5msecs""#,
				r#"#0 numWithUnit "12bytes" parent=#0"#,
				r#"#1 numWithUnit.num "12" parent=#0 leaf"#,
				r#"#2 numWithUnit.unit "bytes" parent=#0 leaf"#,
				r#"#3 numWithUnit "5msecs" parent=#3"#,
				r#"#4 numWithUnit.num "5" parent=#3 leaf"#,
				r#"#5 numWithUnit.unit "msecs" parent=#3 leaf"#,
			],
		],
	);
}

/// `^` and `$` apply to the whole alternation, i.e. `^NEW|RUNNING$` is `^(NEW|RUNNING)$`,
/// unlike common regex syntax where they would bind to the first/last alternative only.
#[test]
fn anchors_apply_to_the_whole_alternation() {
	let definition: &str = r#"
		delimiters: " \t\r\n"
		containerState (0): "^NEW|RUNNING|DONE$"
	"#;
	assert_eq!(
		parse_with(
			definition,
			"NEWx xNEW xRUNNING RUNNING RUNNINGx DONEx xDONE DONE"
		),
		vec![vec![
			r#""NEWx xNEW xRUNNING RUNNING RUNNINGx DONEx xDONE DONE""#,
			r#"#0 containerState "RUNNING" parent=#0 leaf"#,
			r#"#1 containerState "DONE" parent=#1 leaf"#,
		]],
	);
}

/// An optional literal prefix (`ContainerId: `) is included when present,
/// since the match then starts earlier;
/// if the prefix is malformed (two spaces), the rule still matches without it.
/// Being anchored, the rule does not match inside a token (`xcontainer_5_6`),
/// which leaves only the integers after the `_` delimiters.
#[test]
fn optional_prefix_extends_match_leftwards() {
	let definition: &str = concat!(
		r#"
		delimiters: " \t\r\n:,_[]"
		containerID (0): "^(container: |ContainerId: )?"#,
		r#"container_(?<cluster>[0-9]+)_(?<appSeq>[0-9]+)$"
		int (0): "^[0-9]+$"
		"#,
	);
	assert_eq!(
		parse_with(
			definition,
			"[ContainerId: container_1_2, ContainerId:  container_3_4 xcontainer_5_6"
		),
		vec![vec![
			r#""[ContainerId: container_1_2, ContainerId:  container_3_4 xcontainer_5_6""#,
			r#"#0 containerID "ContainerId: container_1_2" parent=#0"#,
			r#"#1 containerID.cluster "1" parent=#0 leaf"#,
			r#"#2 containerID.appSeq "2" parent=#0 leaf"#,
			r#"#3 containerID "container_3_4" parent=#3"#,
			r#"#4 containerID.cluster "3" parent=#3 leaf"#,
			r#"#5 containerID.appSeq "4" parent=#3 leaf"#,
			r#"#6 int "5" parent=#6 leaf"#,
			r#"#7 int "6" parent=#7 leaf"#,
		]],
	);
}

/// Several rules share the name `key_value`; one is picked per match by
/// longest match, then order (`int_value` before `hex_value` for `12`).
/// `str_value` only wins when it is strictly longer (`12x`).
///
/// Placeholder references (`(?<int>)`) are substituted in place
/// and are not sub-rules themselves, so `key_value.int_value` is a leaf.
#[test]
fn same_name_rules_disambiguated_by_length_then_order() {
	let definition: &str = r#"
		delimiters: " \t\r\n:="
		!float: "-?[0-9]+\.[0-9]+([eE]-?[0-9]+)?"
		!hex: "[0-9A-Fa-f]+"
		!int: "-?[0-9]+"
		!txt: "[0-9a-zA-Z_./\\\-]+"
		key_value (0): "^(?<key>(?<txt>))([:=]|: )(?<float_value>(?<float>))$"
		key_value (0): "^(?<key>(?<txt>))([:=]|: )(?<int_value>(?<int>))$"
		key_value (0): "^(?<key>(?<txt>))([:=]|: )(?<hex_value>(?<hex>))$"
		key_value (0): "^(?<key>(?<txt>))=(?<str_value>(?<txt>))$"
	"#;
	assert_eq!(
		parse_with(definition, "a=1.5 b: 12 c:ff d=zz e=12x f: zz"),
		vec![vec![
			r#""a=1.5 b: 12 c:ff d=zz e=12x f: zz""#,
			r#"#0 key_value "a=1.5" parent=#0"#,
			r#"#1 key_value.key "a" parent=#0 leaf"#,
			r#"#2 key_value.float_value "1.5" parent=#0 leaf"#,
			r#"#3 key_value "b: 12" parent=#3"#,
			r#"#4 key_value.key "b" parent=#3 leaf"#,
			r#"#5 key_value.int_value "12" parent=#3 leaf"#,
			r#"#6 key_value "c:ff" parent=#6"#,
			r#"#7 key_value.key "c" parent=#6 leaf"#,
			r#"#8 key_value.hex_value "ff" parent=#6 leaf"#,
			r#"#9 key_value "d=zz" parent=#9"#,
			r#"#10 key_value.key "d" parent=#9 leaf"#,
			r#"#11 key_value.str_value "zz" parent=#9 leaf"#,
			r#"#12 key_value "e=12x" parent=#12"#,
			r#"#13 key_value.key "e" parent=#12 leaf"#,
			r#"#14 key_value.str_value "12x" parent=#12 leaf"#,
		]],
	);
}

/// Root rule matches never nest: once `key_value` consumes the pool ID as its value,
/// `blockPoolID` does not also match inside it (only on its own, later).
#[test]
fn root_matches_do_not_nest() {
	let definition: &str = r#"
		delimiters: " \t\r\n=-"
		blockPoolID (0): "^BP-(?<poolID>[0-9]+)-(?<poolIP>[0-9.]+)-(?<poolTimestamp>[0-9]+)$"
		key_value (0): "^(?<key>[a-z]+)=(?<str_value>[0-9a-zA-Z.\-]+)$"
		int (0): "^[0-9]+$"
	"#;
	assert_eq!(
		parse_with(definition, "block=BP-1-1.2.3.4-5 BP-1-1.2.3.4-5"),
		vec![vec![
			r#""block=BP-1-1.2.3.4-5 BP-1-1.2.3.4-5""#,
			r#"#0 key_value "block=BP-1-1.2.3.4-5" parent=#0"#,
			r#"#1 key_value.key "block" parent=#0 leaf"#,
			r#"#2 key_value.str_value "BP-1-1.2.3.4-5" parent=#0 leaf"#,
			r#"#3 blockPoolID "BP-1-1.2.3.4-5" parent=#3"#,
			r#"#4 blockPoolID.poolID "1" parent=#3 leaf"#,
			r#"#5 blockPoolID.poolIP "1.2.3.4" parent=#3 leaf"#,
			r#"#6 blockPoolID.poolTimestamp "5" parent=#3 leaf"#,
		]],
	);
}

/// Repeated sub-rule names under different parents (`used`/`total`)
/// are distinguished by their fully qualified names.
/// The longer `physicalMemory` absorbs the `byteSize`s inside it;
/// where it fails to match (`-1B`), `byteSize` matches on its own.
#[test]
fn longer_rule_absorbs_shorter_ones() {
	let definition: &str = concat!(
		r#"
		delimiters: " \t\r\n;"
		byteSize (0): "^(?<byteValue>[0-9]+(\.[0-9]+)?)[ ](?<byteUnit>GB|MB|B)$"
		physicalMemory (0): "^"#,
		r#"(?<used>(?<byteValue>[0-9]+(\.[0-9]+)?) (?<byteUnit>GB|MB|B))"#,
		r#" of "#,
		r#"(?<total>(?<byteValue>[0-9]+(\.[0-9]+)?) (?<byteUnit>GB|MB|B))"#,
		r#" physical memory used$"
		"#,
	);
	assert_eq!(
		parse_with(
			definition,
			"182.9 MB of 1 GB physical memory used; -1B of 1 GB physical memory used;"
		),
		vec![vec![
			r#""182.9 MB of 1 GB physical memory used; -1B of 1 GB physical memory used;""#,
			r#"#0 physicalMemory "182.9 MB of 1 GB physical memory used" parent=#0"#,
			r#"#1 physicalMemory.used "182.9 MB" parent=#0"#,
			r#"#2 physicalMemory.used.byteValue "182.9" parent=#1 leaf"#,
			r#"#3 physicalMemory.used.byteUnit "MB" parent=#1 leaf"#,
			r#"#4 physicalMemory.total "1 GB" parent=#0"#,
			r#"#5 physicalMemory.total.byteValue "1" parent=#4 leaf"#,
			r#"#6 physicalMemory.total.byteUnit "GB" parent=#4 leaf"#,
			r#"#7 byteSize "1 GB" parent=#7"#,
			r#"#8 byteSize.byteValue "1" parent=#7 leaf"#,
			r#"#9 byteSize.byteUnit "GB" parent=#7 leaf"#,
		]],
	);
}

/// A negated class that does not exclude every delimiter
/// lets a match run over delimiters (`,` in `DFSClient_...,`)
/// up to one that it does exclude.
#[test]
fn negated_class_spans_delimiters() {
	let definition: &str = r#"
		delimiters: " \t\r\n,_"
		dfsClientID (0): "^DFSClient_[^ \t\r\n]+$"
	"#;
	assert_eq!(
		parse_with(definition, "by DFSClient_NONMAPREDUCE_-1_1, x DFSClient_y"),
		vec![vec![
			r#""by DFSClient_NONMAPREDUCE_-1_1, x DFSClient_y""#,
			r#"#0 dfsClientID "DFSClient_NONMAPREDUCE_-1_1," parent=#0 leaf"#,
			r#"#1 dfsClientID "DFSClient_y" parent=#1 leaf"#,
		]],
	);
}
