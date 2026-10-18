use std::sync::Arc;

use nom::Err as NomErr;
use nom::IResult;
use nom::Parser;
use nom::error::Error as NomError;
use nom::error::ParseError;

use crate::dfa::FlattenedDfa;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::regex::AnchoredRegex;
use crate::regex::Regex;
use crate::regex::RegexError;
use crate::utils::Escaped;
use crate::utils::InvalidEscape;
use crate::utils::NomUtils;

#[derive(Debug)]
pub struct ParsingSpecFileError {
	/// 0-indexed line number.
	pub line_offset: usize,
	pub kind: ParsingSpecFileErrorKind,
}

#[derive(Debug)]
pub enum ParsingSpecFileErrorKind {
	InvalidPriority,
	MissingColon,
	EmptyDelimiters,
	InvalidDelimiters,
	BadLine,
	InvalidName(String),
	InvalidEscape(InvalidEscape),
	InvalidPattern(RegexError),
	DuplicatePlaceholder(String),
	UndefinedPlaceholder(String),
}

/// Currently, this is (almost) trivial,
/// but this enum makes the intent more clear and allows for future additions.
#[derive(Debug)]
enum SpecFileLine<'a> {
	Delimiters(&'a str),
	Rule(i32, &'a str, &'a str),
	Placeholder(&'a str, &'a str),
}

impl ParsingSpec {
	pub fn to_parsing_spec_definition(&self) -> String {
		std::iter::once(format!(
			"delimiters: \"{}\"",
			escape_delimiters(&self.delimiters)
		))
		// Empty line, pretty.
		.chain(std::iter::once(String::new()))
		// Placeholders.
		.chain(self.placeholders.iter().map(|(name, regex)| {
			let pattern: String = regex.to_pattern();
			format!("!{name}: \"{pattern}\"")
		}))
		// Empty line, pretty.
		.chain(std::iter::once(String::new()))
		// Rules.
		.chain(self.rules.iter().map(|rule| {
			let pattern: String = rule.regex.to_pattern();
			format!("{} ({}): \"{pattern}\"", rule.name, rule.priority)
		}))
		.chain(std::iter::once(String::new()))
		.chain(std::iter::once(String::from("===")))
		.chain(std::iter::once(
			serde_json::to_string_pretty(&self.combined_dfa).unwrap(),
		))
		.fold(String::new(), |mut accumulated, line| {
			accumulated.push_str(&line);
			accumulated.push('\n');
			accumulated
		})
	}
}

impl ParsingSpecBuilder {
	pub fn from_parsing_spec_definition(contents: &str) -> Result<Self, ParsingSpecFileError> {
		let mut builder: Self = Self::new();

		let mut maybe_cached_dfa: Option<String> = None;

		for (line_offset, line) in contents.lines().enumerate() {
			if let Some(cached) = &mut maybe_cached_dfa {
				cached.push_str(line);
				continue;
			}

			let line: &str = line.trim();

			if line.is_empty() {
				continue;
			}

			if line.starts_with('#') {
				continue;
			}

			if line.starts_with("===") {
				maybe_cached_dfa = Some(String::new());
				continue;
			}

			let (_remaining, line): (&str, SpecFileLine<'_>) =
				parse_line(line).map_err(|_| ParsingSpecFileError {
					line_offset,
					kind: ParsingSpecFileErrorKind::BadLine,
				})?;

			// TODO: validate remaining empty/whitespace

			match line {
				SpecFileLine::Delimiters(delimiters) => {
					if delimiters.is_empty() {
						return Err(ParsingSpecFileError {
							line_offset,
							kind: ParsingSpecFileErrorKind::EmptyDelimiters,
						});
					}

					let delimiters: String =
						unescape(delimiters).map_err(|err| ParsingSpecFileError {
							line_offset,
							kind: ParsingSpecFileErrorKind::InvalidEscape(err),
						})?;

					builder.set_delimiters(delimiters);
				},
				SpecFileLine::Placeholder(name, pattern) => {
					if name.is_empty() || (name == "delimiters") {
						return Err(ParsingSpecFileError {
							line_offset,
							kind: ParsingSpecFileErrorKind::InvalidName(name.to_owned()),
						});
					}

					let regex: Regex = Regex::from_pattern_with_placeholders(pattern, &mut builder)
						.map_err(|e| ParsingSpecFileError {
							line_offset,
							kind: ParsingSpecFileErrorKind::InvalidPattern(e),
						})?;

					builder
						.add_placeholder(name.to_owned(), regex)
						.map_err(|_| ParsingSpecFileError {
							line_offset,
							kind: ParsingSpecFileErrorKind::DuplicatePlaceholder(name.to_owned()),
						})?;
				},
				SpecFileLine::Rule(priority, name, pattern) => {
					if name.is_empty() || (name == "delimiters") {
						return Err(ParsingSpecFileError {
							line_offset,
							kind: ParsingSpecFileErrorKind::InvalidName(name.to_owned()),
						});
					}

					let regex: AnchoredRegex = AnchoredRegex::from_pattern_with_placeholders(
						pattern,
						Arc::from(name),
						&mut builder,
					)
					.map_err(|e| ParsingSpecFileError {
						line_offset,
						kind: ParsingSpecFileErrorKind::InvalidPattern(e),
					})?;

					let _ = builder.add_rule_parsed(priority, name, regex);
				},
			}
		}

		if let Some(cached) = maybe_cached_dfa {
			match serde_json::from_str::<FlattenedDfa>(&cached) {
				Ok(dfa) => {
					builder.set_cached_dfa(dfa);
				},
				Err(err) => {
					eprintln!("error deserializing cached dfa (ignoring cached value): {err}");
				},
			}
		}

		Ok(builder)
	}
}

fn parse_line(input: &str) -> IResult<&str, SpecFileLine<'_>> {
	use nom::branch::alt;

	alt((parse_placeholder, parse_delimiters, parse_rule)).parse(input)
}

fn parse_placeholder(input: &str) -> IResult<&str, SpecFileLine<'_>> {
	use nom::combinator::cut;
	use nom::sequence::preceded;

	preceded(parse_char::<'!'>, cut(parse_name_pattern))
		.map(|(name, pattern)| SpecFileLine::Placeholder(name, pattern))
		.parse(input)
}

fn parse_delimiters(original_input: &str) -> IResult<&str, SpecFileLine<'_>> {
	use nom::combinator::fail;

	let (input, (name, delimiters)): (&str, (&str, &str)) = parse_name_pattern(original_input)?;

	if name != "delimiters" {
		return fail().parse(input);
	}

	Ok((input, SpecFileLine::Delimiters(delimiters)))
}

fn parse_rule(input: &str) -> IResult<&str, SpecFileLine<'_>> {
	use nom::combinator::opt;

	let (input, name): (&str, &str) = parse_name(input)?;
	let input: &str = input.trim_start();

	let (input, maybe_priority): (&str, Option<i32>) = opt(parse_priority).parse(input)?;
	let input: &str = input.trim_start();

	let priority: i32 = maybe_priority.unwrap_or(0);

	let (input, _): (&str, char) = parse_char::<':'>(input)?;
	let input: &str = input.trim_start();

	let (input, pattern): (&str, &str) = parse_pattern(input)?;

	Ok((input, SpecFileLine::Rule(priority, name, pattern)))
}

fn parse_name_pattern(input: &str) -> IResult<&str, (&str, &str)> {
	let (input, name): (&str, &str) = parse_name(input)?;
	let input: &str = input.trim_start();

	let (input, _): (&str, char) = parse_char::<':'>(input)?;
	let input: &str = input.trim_start();

	let (input, pattern): (&str, &str) = parse_pattern(input)?;

	Ok((input, (name, pattern)))
}

fn parse_name(input: &str) -> IResult<&str, &str> {
	use nom::AsChar;
	use nom::bytes::take_while;

	take_while(|ch| AsChar::is_alphanum(ch) || ch == '_').parse(input)
}

fn parse_priority(input: &str) -> IResult<&str, i32> {
	use nom::character::complete::i32 as i32_parser;

	NomUtils::surrounded_cut::<'(', ')', _, _, _, _>(i32_parser, |input| {
		Err(NomErr::Error(NomError::from_char(input, ')')))
	})
	.parse(input)
}

fn parse_pattern(input: &str) -> IResult<&str, &str> {
	use nom::combinator::recognize;

	NomUtils::surrounded_cut::<'"', '"', _, _, _, _>(recognize(parse_char_sequence), |input| {
		Err(NomErr::Error(NomError::from_char(input, '"')))
	})
	.parse(input)
}

fn parse_char<const CHAR: char>(input: &str) -> IResult<&str, char> {
	NomUtils::parse_char::<CHAR, NomError<&str>>(input)
}

fn parse_char_sequence(input: &str) -> IResult<&str, Vec<char>> {
	use nom::branch::alt;
	use nom::multi::many0;

	many0(alt((parse_escaped_char, parse_regular_char))).parse(input)
}

/// Parse a backslash and any character;
/// the actual escape processing must happen later,
/// depending on whether the outer value is for the delimiter set or a rule regex pattern.
fn parse_escaped_char(input: &str) -> IResult<&str, char> {
	use nom::character::complete::none_of;
	use nom::combinator::cut;
	use nom::sequence::preceded;

	preceded(parse_char::<'\\'>, cut(none_of(""))).parse(input)
}

/// Parse anything but:
///
/// - a backslash (handled by [`parse_escaped_char`]), or
/// - a double quote (handled as the opening/closing characters in [`parse_pattern`]).
fn parse_regular_char(input: &str) -> IResult<&str, char> {
	use nom::character::complete::none_of;

	none_of("\"\\").parse(input)
}

fn unescape(mut input: &str) -> Result<String, InvalidEscape> {
	let mut unescaped: String = String::new();

	while !input.is_empty() {
		let ch: char;
		(input, ch) = Escaped::unescape(input)?;
		unescaped.push(ch);
	}

	Ok(unescaped)
}

fn escape_delimiters(input: &str) -> String {
	input
		.chars()
		.map(|ch| Escaped::escape(ch).to_string())
		.collect::<String>()
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn simple_roundtrip() {
		let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();

		builder.set_delimiters(" .\t");

		builder.add_rule("foo", r"hello world|goodbye").unwrap();
		builder
			.add_rule_with_priority(10, "bar", r"[^a-b-]*z")
			.unwrap();
		builder
			.add_rule_with_priority(-10, "baz", r"(?<quux>\.{6,7}){9}")
			.unwrap();
		builder
			.add_rule_with_priority(-10, "foobar", r"^\^\$$")
			.unwrap();

		let spec1: ParsingSpec = builder.build();

		let serialized1: String = spec1.to_parsing_spec_definition();

		let spec2: ParsingSpec = ParsingSpecBuilder::from_parsing_spec_definition(&serialized1)
			.unwrap()
			.build();
		let serialized2: String = spec2.to_parsing_spec_definition();

		assert_eq!(serialized1, serialized2);
	}

	#[test]
	fn pattern_begins_or_ends_with_whitespace() {
		let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();

		builder.add_rule("foo", r"hello ").unwrap();
		builder.add_rule("foo", r" world").unwrap();

		let spec1: ParsingSpec = builder.build();

		let serialized1: String = spec1.to_parsing_spec_definition();

		let spec2: ParsingSpec = ParsingSpecBuilder::from_parsing_spec_definition(&serialized1)
			.unwrap()
			.build();
		let serialized2: String = spec2.to_parsing_spec_definition();

		assert_eq!(serialized1, serialized2);
	}

	#[test]
	fn normalizing_parentheses() {
		let spec1: ParsingSpec = spec! {
			r#"
			foo: "abcd"
			"#
		};
		let spec2: ParsingSpec = spec! {
			r#"
			foo: "(ab)(cd)"
			"#
		};

		assert_eq!(spec1, spec2);
	}

	#[test]
	fn placeholder_inside_placeholder() {
		// Just making sure this succeeds.
		let _spec: ParsingSpec = spec! {
			r#"
			!p1: "hello"
			!p2: "(?<p1>) world"

			r1: "(?<p2>)+"
			"#
		};
	}

	// TODO good way to test symbolically represented placeholders? nolonger flattened
}
