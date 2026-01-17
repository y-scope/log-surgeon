use std::num::NonZero;
use std::str::Chars;

use nom::Err as NomErr;
use nom::IResult;
use nom::Parser;
use nom::error::ErrorKind as NomErrorKind;
use nom::error::ParseError;

const SPECIAL_CHARACTERS: &str = r"\()[]{}<>*+?-.|^";

#[derive(Debug, Clone)]
pub enum Regex {
	AnyChar,
	Literal(char),
	Capture {
		name: String,
		item: Box<Regex>,
		meta: CaptureMeta,
	},
	// TODO flatten group like in nfa?
	Group {
		negated: bool,
		items: Vec<(char, char)>,
	},
	KleeneClosure(Box<Regex>),
	BoundedRepetition {
		start: u32,
		end: u32,
		item: Box<Regex>,
	},
	Sequence(Vec<Regex>),
	Alternation(Vec<Regex>),
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureMeta {
	pub id: Option<NonZero<u32>>,
	pub parent: Option<NonZero<u32>>,
	pub descendents: usize,
}

#[derive(Debug)]
pub struct RegexError<'a> {
	consumed: &'a str,
	remaining: &'a str,
	kind: RegexErrorKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RegexErrorKind {
	ExpectedChar(char),
	MissingClose(char, char),
	EmptyTerm,
	InvalidLiteral,
	InvalidEscape,
	UnexpectedEnd,
	NumberTooBig,
	ExpectedNumber,
	ExpectedVariableName,
	TooManyCaptures,
	Nom(NomErrorKind),
}

#[derive(Debug)]
struct RegexParsingError<'a> {
	pub input: &'a str,
	pub kind: RegexErrorKind,
}

impl<'a> ParseError<&'a str> for RegexParsingError<'a> {
	fn from_error_kind(input: &'a str, nom: NomErrorKind) -> Self {
		Self {
			input,
			kind: RegexErrorKind::Nom(nom),
		}
	}

	fn append(input: &'a str, kind: NomErrorKind, other: Self) -> Self {
		other
	}
}

impl<'a> RegexParsingError<'a> {
	fn new(input: &'a str, kind: RegexErrorKind) -> Self {
		Self { input, kind }
	}
}

type ParsingResult<'a, T> = IResult<&'a str, T, RegexParsingError<'a>>;

impl Regex {
	pub fn from_pattern(pattern: &str) -> Result<Self, RegexError<'_>> {
		match parse_to_end(pattern) {
			Ok((remaining, mut regex)) => {
				assert!(remaining.is_empty(), "Remaining is {remaining:?}");
				// TODO unwrap
				regex
					.number_captures(&mut { NonZero::<u32>::MIN }, &mut Vec::new())
					.unwrap();
				Ok(regex)
			},
			Err(NomErr::Incomplete(_)) => {
				// unexpected end
				todo!();
			},
			Err(NomErr::Error(err) | NomErr::Failure(err)) => {
				// TODO unwrap
				let consumed: &str = pattern.strip_suffix(err.input).unwrap();
				Err(RegexError {
					consumed,
					remaining: err.input,
					kind: err.kind,
				})
			},
		}
	}
}

impl Regex {
	fn number_captures(&mut self, id: &mut NonZero<u32>, stack: &mut Vec<NonZero<u32>>) -> Option<usize> {
		let mut bread: usize = 0;
		match self {
			Self::AnyChar | Self::Literal(..) | Self::Group { .. } => (),
			Self::Capture { name: _, item, meta } => {
				meta.parent = stack.last().copied();
				meta.id = Some(*id);
				stack.push(*id);
				*id = id.checked_add(1)?;
				meta.descendents = item.number_captures(id, stack)?;
				bread = 1 + meta.descendents;
				stack.pop();
			},
			Self::KleeneClosure(item) | Self::BoundedRepetition { item, .. } => {
				bread += item.number_captures(id, stack)?;
			},
			Self::Sequence(items) | Self::Alternation(items) => {
				for sub_item in items.iter_mut() {
					bread += sub_item.number_captures(id, stack)?;
				}
			},
		};
		Some(bread)
	}
}

fn parse_to_end(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::eof;

	let (input, regex): (&str, Regex) = parse_alternation(input)?;

	// TODO type inference weirdness
	// TODO handle incomplete more uniformly
	match eof(input) {
		Ok((input, _)) => Ok((input, regex)),
		Err(err @ NomErr::Incomplete(_)) => Err(err),
		Err(_) => Err(NomErr::Error(RegexParsingError::new(
			input,
			RegexErrorKind::UnexpectedEnd,
		))),
	}
}

fn parse_alternation(input: &str) -> ParsingResult<'_, Regex> {
	// use nom::combinator::cut;
	use nom::multi::separated_list1;

	let (input, mut items): (&str, Vec<Regex>) =
		// separated_list1(parse_char::<'|'>, cut(parse_sequence)).parse(input)?;
		separated_list1(parse_char::<'|'>, parse_sequence).parse(input)?;

	if items.len() == 1 {
		Ok((input, items.pop().unwrap()))
	} else {
		Ok((input, Regex::Alternation(items)))
	}
}

fn parse_sequence(input: &str) -> ParsingResult<'_, Regex> {
	// use nom::multi::many1;
	use nom::combinator::cut;

	let (mut input, first): (&str, Regex) = cut(parse_suffixed).parse(input)?;

	let mut items: Vec<Regex> = vec![first];

	loop {
		match parse_suffixed(input) {
			Ok((remaining, item)) => {
				input = remaining;
				items.push(item);
			},
			Err(NomErr::Error(_)) => {
				break;
			},
			Err(err @ (NomErr::Incomplete(_) | NomErr::Failure(_))) => {
				return Err(err);
			},
		}
	}

	// TODO many1 error
	// let (input, mut items): (&str, Vec<Regex>) = many1(parse_suffixed).parse(input)?;

	if items.len() == 1 {
		Ok((input, items.pop().unwrap()))
	} else {
		Ok((input, Regex::Sequence(items)))
	}
}

fn parse_suffixed(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;
	use nom::combinator::opt;

	enum Suffix {
		Range(u32, u32),
		Star,
		Plus,
	}

	let (input, regex): (&str, Regex) = parse_term.parse(input)?;

	let (input, maybe_suffix): (&str, Option<Suffix>) = opt(alt((
		parse_char::<'*'>.map(|_| Suffix::Star),
		parse_char::<'+'>.map(|_| Suffix::Plus),
		parse_repetition_bound.map(|(start, end)| Suffix::Range(start, end)),
	)))
	.parse(input)?;

	if let Some(suffix) = maybe_suffix {
		match suffix {
			Suffix::Range(start, end) => Ok((
				input,
				Regex::BoundedRepetition {
					start,
					end,
					item: Box::new(regex),
				},
			)),
			Suffix::Star => Ok((input, Regex::KleeneClosure(Box::new(regex)))),
			Suffix::Plus => {
				let one: Regex = regex.clone();
				let star: Regex = Regex::KleeneClosure(Box::new(regex));
				Ok((input, Regex::Sequence(vec![one, star])))
			},
		}
	} else {
		Ok((input, regex))
	}
}

fn parse_repetition_bound(input: &str) -> ParsingResult<'_, (u32, u32)> {
	use nom::branch::alt;
	use nom::combinator::cut;
	use nom::sequence::separated_pair;

	let (input, (min, max)): (&str, (u32, u32)) = combinator_surrounded_cut::<'{', '}', _, _>(alt((
		separated_pair(parse_digits, parse_char::<','>, cut(parse_digits)).map(|(min, max)| (min, max)),
		parse_digits.map(|count| (count, count)),
	)))
	.parse(input)?;

	Ok((input, (min, max)))
}

fn parse_term(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	alt((
		parse_char::<'.'>.map(|_| Regex::AnyChar),
		parse_literal_character.map(|ch| Regex::Literal(ch)),
		parse_parenthesized,
		parse_group,
		diagnostic_empty_term,
	))
	.parse(input)
}

fn parse_parenthesized(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	// use nom::combinator::cut;
	// use nom::sequence::delimited;

	// delimited(
	// 	parse_char::<'('>,
	// 	cut(alt((parse_capture, parse_alternation))),
	// 	parse_char::<')'>,
	// )
	// .parse(input)

	combinator_surrounded_cut::<'(', ')', _, _>(alt((parse_capture, parse_alternation))).parse(input)
}

fn parse_capture(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;
	// use nom::sequence::delimited;

	let remaining: usize = input.len();

	let (input, _): (&str, char) = parse_char::<'?'>(input)?;

	// let (input, name): (&str, &str) = cut(delimited(
	// 	parse_char::<'<'>,
	// 	cut(parse_variable_name),
	// 	parse_char::<'>'>,
	// ))
	// .parse(input)?;

	let (input, name): (&str, &str) =
		cut(combinator_surrounded_cut::<'<', '>', _, _>(parse_variable_name)).parse(input)?;

	let (input, regex): (&str, Regex) = parse_alternation(input)?;

	Ok((
		input,
		Regex::Capture {
			name: name.to_owned(),
			item: Box::new(regex),
			meta: CaptureMeta {
				id: None,
				parent: None,
				descendents: 0,
			},
		},
	))
}

// ========================================

fn parse_group(input: &str) -> ParsingResult<'_, Regex> {
	// use nom::combinator::cut;
	// use nom::sequence::delimited;

	let (input, (negated, items)): (&str, (bool, Vec<(char, char)>)) =
		combinator_surrounded_cut::<'[', ']', _, _>(parse_group_inside).parse(input)?;
	// 	delimited(parse_char::<'['>, cut(parse_group_inside), parse_char::<']'>).parse(input)?;

	Ok((input, Regex::Group { negated, items }))
}

fn parse_group_inside(input: &str) -> ParsingResult<'_, (bool, Vec<(char, char)>)> {
	use nom::combinator::opt;
	use nom::multi::many1;

	let (input, negated): (&str, Option<char>) = opt(parse_char::<'^'>).parse(input)?;

	let (input, items): (&str, Vec<(char, char)>) = many1(parse_group_item).parse(input)?;

	Ok((input, (negated.is_some(), items)))
}

fn parse_group_item(input: &str) -> ParsingResult<'_, (char, char)> {
	use nom::combinator::cut;
	use nom::combinator::opt;

	let (input, start): (&str, char) = parse_literal_character(input)?;

	let (input, maybe_dash): (&str, Option<char>) = opt(parse_char::<'-'>).parse(input)?;

	if maybe_dash.is_some() {
		let (input, end): (&str, char) = cut(parse_literal_character).parse(input)?;
		Ok((input, (start, end)))
	} else {
		Ok((input, (start, start)))
	}
}

// ========================================

fn parse_literal_character(input: &str) -> ParsingResult<'_, char> {
	use nom::branch::alt;
	use nom::character::complete::none_of;

	alt((
		parse_escaped_character,
		none_of(SPECIAL_CHARACTERS),
		diagnostic_expected_literal,
	))
	.parse(input)
}

fn parse_escaped_character(original_input: &str) -> ParsingResult<'_, char> {
	use nom::branch::alt;
	use nom::combinator::cut;

	let (input, _): (&str, char) = parse_char::<'\\'>(original_input)?;

	// Cut: if we parsed a '\\', we necessarily are looking for an escape character.
	cut(alt((parse_one_char_of(SPECIAL_CHARACTERS), parse_standard_escape))
		.or(|_| diagnostic(original_input, RegexErrorKind::InvalidEscape)))
	.parse(input)
}

// TODO: custom implementation
fn parse_one_char_of<'a>(any: &'static str) -> impl Parser<&'a str, Output = char, Error = RegexParsingError<'a>> {
	use nom::character::complete::one_of;

	one_of(any)
}

fn parse_standard_escape(input: &str) -> ParsingResult<'_, char> {
	let mut chars: Chars = input.chars();

	// We use the NUL character as a marker/equivalent to EOF;
	// it's not a valid escape character.
	let ch: char = chars.next().unwrap_or('\0');

	let unescaped: char = match ch {
		't' => '\t',
		'r' => '\r',
		'n' => '\n',
		_ => {
			return Err(NomErr::Error(RegexParsingError::new(
				input,
				RegexErrorKind::InvalidEscape,
			)));
		},
	};

	return Ok((chars.as_str(), ch));
}

fn parse_char<const CHAR: char>(input: &str) -> ParsingResult<'_, char> {
	let mut chars: Chars = input.chars();

	if let Some(ch) = chars.next() {
		if ch == CHAR {
			return Ok((chars.as_str(), ch));
		}
	}

	Err(NomErr::Error(RegexParsingError::new(
		input,
		RegexErrorKind::ExpectedChar(CHAR),
	)))
}

// =======================================

fn parse_variable_name(input: &str) -> ParsingResult<'_, &str> {
	use nom::character::complete::alphanumeric1;

	match alphanumeric1(input) {
		output @ Ok(_) => output,
		Err(err @ NomErr::Incomplete(_)) => {
			// Propagate
			Err(err)
		},
		Err(NomErr::Error(_) | NomErr::Failure(_)) => Err(NomErr::Error(RegexParsingError::new(
			input,
			RegexErrorKind::ExpectedVariableName,
		))),
	}
}

fn parse_digits(input: &str) -> ParsingResult<'_, u32> {
	use nom::character::complete::digit1;

	match digit1(input) {
		Ok((remaining, lexeme)) => match lexeme.parse::<u32>() {
			Ok(n) => Ok((remaining, n)),
			Err(_) => Err(NomErr::Error(RegexParsingError::new(
				input,
				RegexErrorKind::NumberTooBig,
			))),
		},
		Err(err @ NomErr::Incomplete(_)) => {
			// Propagate
			Err(err)
		},
		Err(NomErr::Error(_) | NomErr::Failure(_)) => Err(NomErr::Error(RegexParsingError::new(
			input,
			RegexErrorKind::ExpectedNumber,
		))),
	}
}

// ==================================

fn diagnostic(input: &str, kind: RegexErrorKind) -> ParsingResult<'_, char> {
	Err(NomErr::Error(RegexParsingError::new(input, kind)))
}

fn diagnostic_empty_term(input: &str) -> ParsingResult<'_, Regex> {
	Err(NomErr::Error(RegexParsingError::new(input, RegexErrorKind::EmptyTerm)))
}

fn diagnostic_expected_literal(input: &str) -> ParsingResult<'_, char> {
	Err(NomErr::Error(RegexParsingError::new(
		input,
		RegexErrorKind::InvalidLiteral,
	)))
}

fn diagnostic_invalid_escape(input: &str) -> ParsingResult<'_, char> {
	Err(NomErr::Failure(RegexParsingError::new(
		input,
		RegexErrorKind::InvalidEscape,
	)))
}

// ==================================
fn combinator_surrounded_cut<'a, const OPEN: char, const CLOSE: char, O, F>(
	mut inside: F,
) -> impl Parser<&'a str, Output = O, Error = RegexParsingError<'a>>
where
	F: Parser<&'a str, Output = O, Error = RegexParsingError<'a>>,
{
	move |input| {
		let (input, _): (&str, char) = parse_char::<OPEN>(input)?;

		let (input, output): (&str, O) = match inside.parse(input) {
			Ok(ok) => ok,
			Err(err @ NomErr::Incomplete(_)) => {
				// Propagate the "not enough input", although this shouldn't be relevant for us.
				return Err(err);
			},
			Err(NomErr::Error(err) | NomErr::Failure(err)) => {
				// Since we already matched the opening character, we require the inside to match too;
				// fold `Error` (meaning "something else may match") to a `Failure` ("input is malformed"),
				// and propagate the inside's error message.
				return Err(NomErr::Failure(err));
			},
		};
		// TODO cut prevents fnmut
		// let (input, output): (&str, O) = cut(inside).parse(input)?;

		let (input, _): (&str, char) = match parse_char::<CLOSE>(input) {
			Ok(ok) => ok,
			Err(_) => {
				return Err(NomErr::Failure(RegexParsingError::new(
					input,
					RegexErrorKind::MissingClose(OPEN, CLOSE),
				)));
			},
		};

		Ok((input, output))
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn good() {
		Regex::from_pattern("abc").unwrap();
		Regex::from_pattern("abc|def").unwrap();
		Regex::from_pattern("abc|def.ghi").unwrap();
		Regex::from_pattern("abc|def.ghi*").unwrap();
		Regex::from_pattern("abc|def(.ghi)*").unwrap();
		Regex::from_pattern("abc|def(?<hello>.ghi)*").unwrap();

		Regex::from_pattern(r"[ \t]").unwrap();
	}

	#[test]
	fn empty_term() {
		{
			let e: RegexError<'_> = Regex::from_pattern("|abc").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EmptyTerm);
			assert_eq!(e.consumed, "");
			assert_eq!(e.remaining, "|abc");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern("abc|").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EmptyTerm);
			assert_eq!(e.consumed, "abc|");
			assert_eq!(e.remaining, "");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern("a||bc").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EmptyTerm);
			assert_eq!(e.consumed, "a|");
			assert_eq!(e.remaining, "|bc");
		}
	}

	#[test]
	fn unclosed_parentheses() {
		{
			let e: RegexError<'_> = Regex::from_pattern("(abc").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::MissingClose('(', ')'));
			assert_eq!(e.consumed, "(abc");
			assert_eq!(e.remaining, "");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern("(?<abc*").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::MissingClose('<', '>'));
			assert_eq!(e.consumed, "(?<abc");
			assert_eq!(e.remaining, "*");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern("(abc[def)").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::MissingClose('[', ']'));
			assert_eq!(e.consumed, "(abc[def");
			assert_eq!(e.remaining, ")");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern(".{123a}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::MissingClose('{', '}'));
			assert_eq!(e.consumed, ".{123");
			assert_eq!(e.remaining, "a}");
		}
	}

	#[test]
	fn expected_number() {
		{
			let e: RegexError<'_> = Regex::from_pattern(".{ }").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedNumber);
			assert_eq!(e.consumed, ".{");
			assert_eq!(e.remaining, " }");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern(".{123,").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedNumber);
			assert_eq!(e.consumed, ".{123,");
			assert_eq!(e.remaining, "");
		}
	}

	#[test]
	fn number_too_big() {
		{
			let pattern: String = format!(".{{{}}}", "9".repeat(64));
			let e: RegexError<'_> = Regex::from_pattern(&pattern).unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::NumberTooBig);
			assert_eq!(e.consumed, ".{");
			assert_eq!(e.remaining, &pattern[".{".len()..]);
		}
	}

	#[test]
	fn variable_name() {
		{
			let e: RegexError<'_> = Regex::from_pattern("(?< ").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedVariableName);
			assert_eq!(e.consumed, "(?<");
			assert_eq!(e.remaining, " ");
		}
	}

	#[test]
	fn expected_char() {
		{
			let e: RegexError<'_> = Regex::from_pattern("(?a").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedChar('<'));
			assert_eq!(e.consumed, "(?");
			assert_eq!(e.remaining, "a");
		}
	}

	// TODO explain
	#[test]
	fn unexpected_char() {
		{
			let e: RegexError<'_> = Regex::from_pattern("*").unwrap_err();
			// assert_eq!(e.kind, RegexErrorKind::UnexpectedChar('*'));
			assert_eq!(e.kind, RegexErrorKind::EmptyTerm);
			assert_eq!(e.consumed, "");
			assert_eq!(e.remaining, "*");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern("a**").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::UnexpectedEnd);
			// assert_eq!(e.kind, RegexErrorKind::EmptyTerm);
			assert_eq!(e.consumed, "a*");
			assert_eq!(e.remaining, "*");
		}
	}

	#[test]
	fn empty_group() {
		{
			let e: RegexError<'_> = Regex::from_pattern("[^]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidLiteral);
			assert_eq!(e.consumed, "[^");
			assert_eq!(e.remaining, "]");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern("[]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidLiteral);
			assert_eq!(e.consumed, "[");
			assert_eq!(e.remaining, "]");
		}
	}

	#[test]
	fn invalid_escapes() {
		{
			let e: RegexError<'_> = Regex::from_pattern(r"[ \a]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidEscape);
			assert_eq!(e.consumed, "[ ");
			assert_eq!(e.remaining, r"\a]");
		}
	}
}
