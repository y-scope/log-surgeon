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
			r"^(?<ts>(?<y>[0-9]{4})-(?<m>[0-9]{2}))(:(?<tag>[a-z]))*(?<lvl>(?<i>I)|(?<w>W)|(?<n>[0-9]+))$",
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
