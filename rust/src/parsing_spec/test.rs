use super::*;
use crate::log_event::LogEvent;
use crate::parser::Parser;

#[test]
fn non_ascii_definition_round_trips() {
	let definition: &str = concat!(
		"delimiters: \" \\u{0434}\\u{01f600}\"\n",
		"cyrillic: \"\\u{0434}+\"\n",
		"emoji: \"[\\u{01f600}-\\u{01f64f}]+\"\n",
	);
	let spec: ParsingSpec = crate::spec! { definition };
	let roundtrip: String = spec.to_parsing_spec_definition();
	ParsingSpecBuilder::from_parsing_spec_definition(&roundtrip).unwrap();
}

#[test]
fn number_encoding() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder
		.add_rule("has_number", r"\w*\d\w*")
		.unwrap()
		.add_rule("ip_address", r"(?<first>\d+)(\.\d+){3}")
		.unwrap()
		.add_encoding("int", Regex::from_pattern(r"\d+").unwrap())
		.unwrap();

	let spec: ParsingSpec = builder.build();
	assert_eq!(spec.rules.len(), 2);

	let mut parser: Parser = Arc::new(spec).create_parser();

	let event: LogEvent<'_> = parser.next_event("a1b", &mut 0).unwrap();
	assert_eq!(event.all_matches.len(), 1);
	assert_eq!(
		event.all_matches[0].rule_idx,
		RuleIdx::from(NonZero::new(1).unwrap())
	);
	assert_eq!(event.all_matches[0].encoding_idx, None);

	let event: LogEvent<'_> = parser.next_event("123", &mut 0).unwrap();
	assert_eq!(event.all_matches.len(), 1);
	assert_eq!(
		event.all_matches[0].rule_idx,
		RuleIdx::from(NonZero::new(1).unwrap())
	);
	assert_eq!(
		event.all_matches[0].encoding_idx.unwrap(),
		NonZero::new(1).unwrap()
	);

	let event: LogEvent<'_> = parser.next_event("12.34.56.78", &mut 0).unwrap();
	assert_eq!(event.message.as_str(), "12.34.56.78");
	println!("matches are {:?}", event.all_matches.as_slice());
	assert_eq!(event.all_matches.len(), 2);
	assert_eq!(
		event.all_matches[0].rule_idx,
		RuleIdx::from(NonZero::new(2).unwrap())
	);
	assert_eq!(event.all_matches[0].encoding_idx, None);
	assert_eq!(
		event.all_matches[1].encoding_idx.unwrap(),
		NonZero::new(1).unwrap()
	);
}
