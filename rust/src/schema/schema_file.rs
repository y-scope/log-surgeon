use nom::Err as NomErr;
use nom::IResult;
use nom::Parser;
use nom::error::Error as NomError;

use super::*;
use crate::regex::RegexError;
use crate::utils::Escaped;

#[derive(Debug)]
pub struct SchemaFileError<'input> {
	/// 0-indexed line number.
	pub line_offset: usize,
	pub kind: SchemaParsingErrorKind<'input>,
}

#[derive(Debug)]
pub enum SchemaParsingErrorKind<'input> {
	InvalidName,
	InvalidPriority,
	MissingColon,
	EmptyDelimiters,
	InvalidDelimiters,
	InvalidPattern(RegexError<'input>),
	DuplicatePlaceholder(String),
	UndefinedPlaceholder(String),
}

/// Currently, this is (almost) trivial,
/// but this enum makes the intent more clear and allows for future additions.
#[derive(Debug)]
enum SchemaFileLine<'a> {
	Delimiters(String),
	Rule(i32, &'a str, &'a str),
	Placeholder(&'a str, &'a str),
}

impl Schema {
	pub fn to_schema_definition(&self) -> String {
		std::iter::once(format!("delimiters:{}", escape_delimiters(&self.delimiters)))
			// Empty line, pretty.
			.chain(std::iter::once(String::new()))
			// Placeholders.
			.chain(self.placeholders.iter().map(|(name, regex)| {
				let pattern: String = AnchoredRegex::unanchored(regex.clone()).to_pattern();
				format!("!{name}: {pattern}")
			}))
			// Empty line, pretty.
			.chain(std::iter::once(String::new()))
			// Rules.
			.chain(self.rules.iter().map(|rule| {
				let pattern: String = rule.regex.to_pattern();
				format!("{} ({}): {pattern}", rule.name, rule.priority)
			}))
			.chain(std::iter::once(String::new()))
			.chain(std::iter::once(format!("===")))
			.chain(std::iter::once(serde_json::to_string(&self.main_dfa).unwrap()))
			.fold(String::new(), |mut accumulated, line| {
				accumulated.push_str(&line);
				accumulated.push('\n');
				accumulated
			})
	}
}

impl SchemaBuilder {
	pub fn from_schema_definition(contents: &str) -> Result<Self, SchemaFileError<'_>> {
		let mut builder: Self = Self::new();

		let mut maybe_cached_dfa: Option<String> = None;

		for (line_offset, line) in contents.lines().enumerate() {
			if let Some(cached) = &mut maybe_cached_dfa {
				cached.push_str(line);
				continue;
			}
			// TODO: line offset 0/1 based (currently 0).
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

			let line: SchemaFileLine = parse_line(line).map_err(|kind| SchemaFileError { line_offset, kind })?;

			match line {
				SchemaFileLine::Delimiters(delimiters) => {
					if delimiters.is_empty() {
						return Err(SchemaFileError {
							line_offset,
							kind: SchemaParsingErrorKind::EmptyDelimiters,
						});
					}
					builder.set_delimiters(delimiters);
				},
				SchemaFileLine::Placeholder(name, pattern) => {
					let regex: Regex = Regex::from_pattern_with_placeholders(pattern, &mut builder)
						.map_err(SchemaFileError::with_line(
							line_offset,
							SchemaParsingErrorKind::InvalidPattern,
						))?
						.inner;

					builder
						.add_placeholder(name.to_owned(), regex)
						.map_err(|_| SchemaFileError {
							line_offset,
							kind: SchemaParsingErrorKind::DuplicatePlaceholder(name.to_owned()),
						})?;
				},
				SchemaFileLine::Rule(priority, name, pattern) => {
					let regex: AnchoredRegex = Regex::from_pattern_with_placeholders(pattern, &mut builder).map_err(
						SchemaFileError::with_line(line_offset, SchemaParsingErrorKind::InvalidPattern),
					)?;
					let Ok(_) = builder.add_rule_with_priority(priority, name, regex);
				},
			}
		}

		if let Some(cached) = maybe_cached_dfa {
			builder.set_cached_dfa(serde_json::from_str(&cached).unwrap());
		}

		Ok(builder)
	}
}

impl<'input> SchemaFileError<'input> {
	fn with_line<E, F>(line_offset: usize, kind: F) -> impl FnOnce(E) -> Self
	where
		F: FnOnce(E) -> SchemaParsingErrorKind<'input>,
	{
		move |e| Self {
			line_offset,
			kind: kind(e),
		}
	}
}

fn parse_line(input: &str) -> Result<SchemaFileLine<'_>, SchemaParsingErrorKind<'_>> {
	use nom::character::complete::char as char_parser;
	use nom::combinator::opt;

	let (input, is_placeholder): (&str, bool) = if let Some(suffix) = input.strip_prefix('!') {
		(suffix, true)
	} else {
		(input, false)
	};

	let (input, name): (&str, &str) = parse_name(input).map_err(|_| SchemaParsingErrorKind::InvalidName)?;

	let input: &str = input.trim_start();

	let (input, priority): (&str, i32) = if name != "delimiters" {
		let (input, maybe_priority): (&str, Option<i32>) = opt(parse_priority)
			.parse(input)
			.map_err(|_| SchemaParsingErrorKind::InvalidPriority)?;
		(input.trim_start(), maybe_priority.unwrap_or(0))
	} else {
		(input, 0)
	};

	let (input, _): (&str, char) = char_parser::<&str, NomError<&str>>(':')
		.parse(input)
		.map_err(|_| SchemaParsingErrorKind::MissingColon)?;

	let input: &str = input.trim_start();

	if name == "delimiters" {
		// TODO error if `name == "delimiters" && is_placeholder`
		let delimiters: String = parse_delimiters(input).map_err(|_| SchemaParsingErrorKind::InvalidDelimiters)?;
		Ok(SchemaFileLine::Delimiters(delimiters))
	} else if is_placeholder {
		Ok(SchemaFileLine::Placeholder(name, input))
	} else {
		Ok(SchemaFileLine::Rule(priority, name, input))
	}
}

fn parse_name(input: &str) -> IResult<&str, &str> {
	use nom::AsChar;
	use nom::bytes::take_while1;

	take_while1(|ch| AsChar::is_alphanum(ch) || ch == '_').parse(input)
}

fn parse_priority(input: &str) -> IResult<&str, i32> {
	use nom::character::complete::char as char_parser;
	use nom::character::complete::i32 as i32_parser;
	use nom::combinator::cut;
	use nom::sequence::delimited;

	delimited(char_parser('('), cut(i32_parser), char_parser(')')).parse(input)
}

fn parse_delimiters(mut input: &str) -> Result<String, NomErr<NomError<&str>>> {
	let mut delimiters: String = String::new();

	loop {
		let (rest, chars): (&str, &str) = take_non_escaped(input)?;
		delimiters.push_str(chars);

		let Some((rest, _)): Option<(&str, ())> = take_backslash(rest).ok() else {
			break;
		};

		let (rest, ch): (&str, char) = parse_escape(rest)?;
		delimiters.push(ch);

		input = rest;
	}

	Ok(delimiters)
}

fn take_non_escaped(input: &str) -> IResult<&str, &str> {
	use nom::bytes::complete::take_while;

	take_while(|ch| ch != '\\').parse(input)
}

fn take_backslash(input: &str) -> IResult<&str, ()> {
	use nom::character::complete::char as char_parser;

	char_parser('\\').map(|_| ()).parse(input)
}

fn parse_escape(input: &str) -> IResult<&str, char> {
	use nom::combinator::fail;

	match Escaped::unescape(input) {
		Ok((input, ch)) => Ok((input, ch)),
		Err(_) => fail().parse(input),
	}
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
		let mut builder: SchemaBuilder = SchemaBuilder::new();

		builder.set_delimiters(" .\t");

		builder.add_rule("foo", r"hello world|goodbye").unwrap();
		builder.add_rule_with_priority(10, "bar", r"[^a-b-]*").unwrap();
		builder
			.add_rule_with_priority(-10, "baz", r"(?<quux>\.{6,7}){9}")
			.unwrap();
		builder.add_rule_with_priority(-10, "foobar", r"^\^\$$").unwrap();

		let schema: Schema = builder.build();

		let serialized: String = schema.to_schema_definition();

		let schema2: Schema = SchemaBuilder::from_schema_definition(&serialized).unwrap().build();

		assert_eq!(schema, schema2);
	}

	#[test]
	fn pattern_begins_or_ends_with_whitespace() {
		let mut builder: SchemaBuilder = SchemaBuilder::new();

		builder.add_rule("foo", r"hello ").unwrap();
		builder.add_rule("foo", r" world").unwrap();

		let schema: Schema = builder.build();

		let serialized: String = schema.to_schema_definition();

		let schema2: Schema = SchemaBuilder::from_schema_definition(&serialized).unwrap().build();
		let serialized2: String = schema2.to_schema_definition();

		assert_eq!(serialized, serialized2);
	}

	// TODO good way to test symbolically represented placeholders? nolonger flattened
	/*
	#[test]
	fn test_placeholders() {
		let schema1: Schema = schema!(
			r#"
			!p1: [a-z]
			!p2: (?<p2>[0-9])

			foo: (?<p1>)(?<p2>)
			"#
		);
		let schema2: Schema = schema!(
			r#"
			foo: [a-z](?<p2>[0-9])
			"#
		);

		assert_eq!(schema1, schema2);
	}

	#[test]
	fn test_nested_placeholders() {
		let schema1: Schema = schema!(
			r#"
			!p1: [a-z]
			!p2: hello(?<p1>)

			foo: (?<p1>)(?<p2>)world
			"#
		);
		let schema2: Schema = schema!(
			r#"
			foo: [a-z]hello[a-z]world
			"#
		);

		assert_eq!(schema1.to_schema_definition(), schema2.to_schema_definition());
	}
	*/
}
