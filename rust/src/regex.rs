use std::num::NonZero;
use std::str::Chars;

use nom::Err as NomErr;
use nom::IResult;
use nom::Parser;
use nom::error::ErrorKind as NomErrorKind;
use nom::error::ParseError;

const SPECIAL_CHARACTERS: &str = r"\()[]{}<>*+?-.|^";

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum Regex {
	AnyChar,
	Literal(char),
	Capture(RegexCapture),
	// TODO flatten group like in nfa?
	Group { negated: bool, items: Vec<(char, char)> },
	KleeneClosure(Box<Regex>),
	BoundedRepetition { lo: u32, hi: u32, item: Box<Regex> },
	Sequence(Vec<Regex>),
	Alternation(Vec<Regex>),
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct RegexCapture {
	pub name: String,
	pub item: Box<Regex>,
	/// Capture ID local to the rule/regex pattern.
	pub id: NonZero<u32>,
	/// ID of parent capture, if any.
	pub parent_id: Option<NonZero<u32>>,
	/// Number of contained nested captures; it is `0` iff this is a "leaf" capture.
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
	NumberTooBig,
	ExpectedNumber,
	ExpectedHexDigits,
	InvalidCodePoint(u32),
	ExpectedCaptureName,
	TooManyCaptures,
	ExpectedOneOf(&'static str, bool),
	EscapeClassInGroupRange,
	Nom(NomErrorKind),
}

#[derive(Debug)]
struct RegexParsingError<'a> {
	pub input: &'a str,
	pub kind: RegexErrorKind,
}

#[derive(Debug)]
enum Literals {
	Single(char),
	Group(Vec<(char, char)>),
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
				assert_eq!(remaining, "");
				regex
					.number_captures(&mut { NonZero::<u32>::MIN }, &mut Vec::new())
					.ok_or(RegexError {
						consumed: pattern,
						remaining: "",
						kind: RegexErrorKind::TooManyCaptures,
					})?;
				Ok(regex)
			},
			Err(NomErr::Incomplete(_)) => {
				panic!("We shouldn't be using anything that can return this!");
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
	/// [`RegexCapture::id`] defaults to [`NonZero::<u32>::MAX`];
	/// if we actually reach this, `next_id` will overflow,
	/// so it naturally works as a placeholder/invalid value.
	fn number_captures(&mut self, id: &mut NonZero<u32>, stack: &mut Vec<NonZero<u32>>) -> Option<usize> {
		let mut bread: usize = 0;
		match self {
			Self::AnyChar | Self::Literal(..) | Self::Group { .. } => (),
			Self::Capture(capture) => {
				capture.parent_id = stack.last().copied();
				capture.id = *id;
				stack.push(*id);
				// `id` is `u32`.
				*id = id.checked_add(1)?;
				capture.descendents = capture.item.number_captures(id, stack)?;
				// `bread` is `usize`.
				bread = 1 + capture.descendents;
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

impl RegexErrorKind {
	fn error(self, input: &str) -> NomErr<RegexParsingError<'_>> {
		NomErr::Error(RegexParsingError::new(input, self))
	}

	fn diagnostic<'a, T>(self) -> impl Fn(&'a str) -> ParsingResult<'a, T> {
		move |input| Err(self.error(input))
	}
}

// ==================================

fn parse_to_end(input: &str) -> ParsingResult<'_, Regex> {
	// `parse_sequence` (and consequently `parse_alternation`) may swallow errors from
	// `parse_suffixed`, since the former two are "lists" that simply terminate when
	// no more elements (suffixed terms) can be parsed.
	// `parse_alternation` is called at the top level (here), or inside parentheses (possibly a capture).
	// Inside parentheses, after failing to parse a term (i.e. reaching the end of the list),
	// we look for the closing parenthesis.
	// Here, after reaching the end of the list, we ensure we're at the end of input,
	// otherwise "reproduce" the invalid term error.
	let (input, regex): (&str, Regex) = parse_alternation(input)?;

	if !input.is_empty() {
		return Err(RegexErrorKind::EmptyTerm.error(input));
	}

	Ok((input, regex))
}

fn parse_alternation(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;
	use nom::combinator::opt;

	// Cut: Any time we're "trying" to parse an alternation,
	// we necessarily are expecting at least one item.
	let (mut input, first): (&str, Regex) = cut(parse_sequence).parse(input)?;

	let mut items: Vec<Regex> = vec![first];

	loop {
		let maybe_bar: Option<char>;
		(input, maybe_bar) = opt(parse_char::<'|'>).parse(input)?;
		if maybe_bar.is_none() {
			break;
		}

		// Cut: After seeing a '|', we necessarily are expecting a sequence.
		match cut(parse_sequence).parse(input) {
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

	if items.len() == 1 {
		Ok((input, items.pop().unwrap()))
	} else {
		Ok((input, Regex::Alternation(items)))
	}
}

fn parse_sequence(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;

	// Cut: Any time we're "trying" to parse a sequence,
	// we necessarily are expecting at least one item.
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
		Question,
	}

	let (input, regex): (&str, Regex) = parse_term(input)?;

	let (input, maybe_suffix): (&str, Option<Suffix>) = opt(alt((
		parse_char::<'*'>.map(|_| Suffix::Star),
		parse_char::<'+'>.map(|_| Suffix::Plus),
		parse_char::<'?'>.map(|_| Suffix::Question),
		parse_repetition_bound.map(|(start, end)| Suffix::Range(start, end)),
	)))
	.parse(input)?;

	if let Some(suffix) = maybe_suffix {
		match suffix {
			Suffix::Range(lo, hi) => Ok((
				input,
				Regex::BoundedRepetition {
					lo,
					hi,
					item: Box::new(regex),
				},
			)),
			Suffix::Star => Ok((input, Regex::KleeneClosure(Box::new(regex)))),
			Suffix::Plus => {
				let one: Regex = regex.clone();
				let star: Regex = Regex::KleeneClosure(Box::new(regex));
				Ok((input, Regex::Sequence(vec![one, star])))
			},
			Suffix::Question => Ok((
				input,
				Regex::BoundedRepetition {
					lo: 0,
					hi: 1,
					item: Box::new(regex),
				},
			)),
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
		// Cut: After seeing a ',', we necessarily are expecting an upper bound.
		separated_pair(parse_digits, parse_char::<','>, cut(parse_digits)),
		parse_digits.map(|count| (count, count)),
	)))
	.parse(input)?;

	Ok((input, (min, max)))
}

fn parse_term(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	alt((
		parse_char::<'.'>.map(|_| Regex::AnyChar),
		parse_literal_character.map(|literal| match literal {
			Literals::Single(ch) => Regex::Literal(ch),
			Literals::Group(items) => Regex::Group { negated: false, items },
		}),
		parse_parenthesized,
		parse_group,
		RegexErrorKind::EmptyTerm.diagnostic(),
	))
	.parse(input)
}

fn parse_parenthesized(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	combinator_surrounded_cut::<'(', ')', _, _>(alt((parse_capture, parse_alternation))).parse(input)
}

fn parse_capture(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;

	let remaining: usize = input.len();

	let (input, _): (&str, char) = parse_char::<'?'>(input)?;

	// Cut: After seeing a '?', we necessarily are expecting a capture.
	let (input, name): (&str, &str) =
		cut(combinator_surrounded_cut::<'<', '>', _, _>(parse_capture_name)).parse(input)?;

	let (input, regex): (&str, Regex) = parse_alternation(input)?;

	Ok((
		input,
		Regex::Capture(RegexCapture {
			name: name.to_owned(),
			item: Box::new(regex),
			// See note for [`Regex::number_captures`].
			id: NonZero::<u32>::MAX,
			parent_id: None,
			descendents: 0,
		}),
	))
}

// ========================================

fn parse_group(input: &str) -> ParsingResult<'_, Regex> {
	let (input, (negated, items)): (&str, (bool, Vec<(char, char)>)) =
		combinator_surrounded_cut::<'[', ']', _, _>(parse_group_inside).parse(input)?;

	Ok((input, Regex::Group { negated, items }))
}

fn parse_group_inside(input: &str) -> ParsingResult<'_, (bool, Vec<(char, char)>)> {
	use nom::combinator::opt;
	use nom::multi::many1;

	let (input, negated): (&str, Option<char>) = opt(parse_char::<'^'>).parse(input)?;

	// let (input, items): (&str, Vec<(char, char)>) = many1(parse_group_item).parse(input)?;

	let (mut input, mut items): (&str, Vec<(char, char)>) = parse_group_item(input)?;
	loop {
		match parse_group_item(input) {
			Ok((new_input, new_items)) => {
				input = new_input;
				items.extend(&new_items);
			},
			Err(NomErr::Error(_)) => {
				break;
			},
			Err(err @ NomErr::Failure(_) | err @ NomErr::Incomplete(_)) => {
				return Err(err);
			},
		}
	}

	Ok((input, (negated.is_some(), items)))
}

fn parse_group_item(original_input: &str) -> ParsingResult<'_, Vec<(char, char)>> {
	use nom::combinator::cut;
	use nom::combinator::opt;

	let (input, start): (&str, Literals) = parse_literal_character(original_input)?;

	let (input_after_dash, maybe_dash): (&str, Option<char>) = opt(parse_char::<'-'>).parse(input)?;

	if maybe_dash.is_some() {
		match start {
			Literals::Single(start) => {
				let (input, end): (&str, Literals) = cut(parse_literal_character).parse(input_after_dash)?;
				match end {
					Literals::Single(end) => Ok((input, vec![(start, end)])),
					Literals::Group(items) => {
						return Err(RegexErrorKind::EscapeClassInGroupRange.error(input_after_dash));
					},
				}
			},
			Literals::Group(items) => {
				return Err(RegexErrorKind::EscapeClassInGroupRange.error(original_input));
			},
		}
	} else {
		match start {
			Literals::Single(ch) => Ok((input, vec![(ch, ch)])),
			Literals::Group(items) => Ok((input, items)),
		}
	}
}

// ========================================

fn parse_literal_character(input: &str) -> ParsingResult<'_, Literals> {
	use nom::branch::alt;

	alt((
		parse_escaped_character,
		parse_one_char_of::<true>(SPECIAL_CHARACTERS).map(Literals::Single),
		RegexErrorKind::InvalidLiteral.diagnostic(),
	))
	.parse(input)
}

fn parse_escaped_character(original_input: &str) -> ParsingResult<'_, Literals> {
	use nom::branch::alt;
	use nom::combinator::cut;

	let (input, _): (&str, char) = parse_char::<'\\'>(original_input)?;

	// Cut: If we parsed a '\\', we necessarily are looking for an escape character.
	cut(alt((
		parse_one_char_of::<false>(SPECIAL_CHARACTERS).map(Literals::Single),
		parse_standard_escape,
	))
	// Outside of the `alt` since the error starts at the original input.
	.or(|_| Err(RegexErrorKind::InvalidEscape.error(original_input))))
	.parse(input)
}

fn parse_one_char_of<'a, const NEGATE: bool>(
	any: &'static str,
) -> impl Parser<&'a str, Output = char, Error = RegexParsingError<'a>> {
	move |input: &'a str| {
		let mut chars: Chars<'_> = input.chars();

		if let Some(ch) = chars.next() {
			if any.contains(ch) {
				if !NEGATE {
					return Ok((chars.as_str(), ch));
				} else {
					return Err(RegexErrorKind::ExpectedOneOf(any, NEGATE).error(input));
				}
			} else if NEGATE {
				return Ok((chars.as_str(), ch));
			}
		}

		Err(RegexErrorKind::ExpectedOneOf(any, NEGATE).error(input))
	}
}

fn parse_standard_escape(input: &str) -> ParsingResult<'_, Literals> {
	let mut chars: Chars<'_> = input.chars();

	// We use the NUL character as a marker/equivalent to EOF;
	// it's not a valid escape character, and will be caught in the default branch of the `match` block below.
	let ch: char = chars.next().unwrap_or('\0');

	let unescaped: char = match ch {
		't' => '\t',
		'r' => '\r',
		'n' => '\n',
		'u' => {
			return combinator_surrounded_cut::<'{', '}', _, _>(parse_hex_code_point)
				.map(Literals::Single)
				.parse(chars.as_str());
		},
		'd' | 's' | 'w' => {
			return Ok((
				chars.as_str(),
				Literals::Group(match ch {
					'd' => vec![('0', '9')],
					's' => vec![(' ', ' '), ('\t', '\t'), ('\r', '\r'), ('\n', '\n')],
					'w' => vec![('0', '9'), ('a', 'z'), ('A', 'Z')],
					_ => {
						// TODO better message
						unreachable!();
					},
				}),
			));
		},
		_ => {
			return Err(RegexErrorKind::InvalidEscape.error(input));
		},
	};

	return Ok((chars.as_str(), Literals::Single(unescaped)));
}

fn parse_char<const CHAR: char>(input: &str) -> ParsingResult<'_, char> {
	let mut chars: Chars<'_> = input.chars();

	if let Some(ch) = chars.next() {
		if ch == CHAR {
			return Ok((chars.as_str(), ch));
		} else {
			return Err(RegexErrorKind::ExpectedChar(CHAR).error(input));
		}
	}

	Err(RegexErrorKind::ExpectedChar(CHAR).error(input))
}

// =======================================

fn parse_capture_name(input: &str) -> ParsingResult<'_, &str> {
	// use nom::character::complete::alphanumeric1;
	use nom::AsChar;
	use nom::bytes::take_while1;

	take_while1(|ch| AsChar::is_alphanum(ch) || ch == '_')
		.or(RegexErrorKind::ExpectedCaptureName.diagnostic())
		.parse(input)
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

fn parse_hex_code_point(input: &str) -> ParsingResult<'_, char> {
	use nom::multi::fold_many_m_n;

	// Sigh... rust const eval and casts...
	const MAX_BYTES_PER_CODE_POINT: usize = (char::MAX as u32).ilog2() as usize + 1;

	let (remaining, code_point): (&str, u32) = fold_many_m_n(
		1,
		MAX_BYTES_PER_CODE_POINT,
		parse_hex_digit_pair,
		|| 0,
		|folded, b| (folded << u8::BITS) | b,
	)
	.parse(input)
	.map_err(|_| RegexErrorKind::ExpectedHexDigits.error(input))?;

	if let Some(ch) = char::from_u32(code_point) {
		Ok((remaining, ch))
	} else {
		Err(RegexErrorKind::InvalidCodePoint(code_point).error(input))
	}

	// let original_input: &str = input;

	// let mut value: u32 = 0;
	// let mut x;
	// for _ in 0..MAX_BYTES_PER_CODE_POINT {
	// 	(input, x) = parse_hex_digit_pair(input).ok_or(NomErr::Error(RegexParsingError::new(
	// 		original_input,
	// 		RegexErrorKind::ExpectedHexDigits,
	// 	)))?;
	// 	value = (value << 8) | x;
	// }

	// if let Some(ch) = char::from_u32(value) {
	// 	Ok((input, ch))
	// } else {
	// 	Err(NomErr::Error(RegexParsingError::new(
	// 		original_input,
	// 		RegexErrorKind::InvalidCodePoint(value),
	// 	)))
	// }
}

fn parse_hex_digit_pair(input: &str) -> ParsingResult<'_, u32> {
	let mut chars: Chars<'_> = input.chars();

	if let Some(upper) = chars.next() {
		if let Some(lower) = chars.next() {
			match (upper.to_digit(16), lower.to_digit(16)) {
				(Some(upper), Some(lower)) => {
					return Ok((chars.as_str(), (upper << 4) + lower));
				},
				_ => (),
			}
		}
	}

	Err(RegexErrorKind::ExpectedHexDigits.error(input))
}

// ==================================
fn combinator_surrounded_cut<'a, const OPEN: char, const CLOSE: char, O, F>(
	mut inside: F,
) -> impl Parser<&'a str, Output = O, Error = RegexParsingError<'a>>
where
	F: Parser<&'a str, Output = O, Error = RegexParsingError<'a>>,
{
	use nom::combinator::cut;

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

		let (input, _): (&str, char) =
			cut(parse_char::<CLOSE>.or(RegexErrorKind::MissingClose(OPEN, CLOSE).diagnostic())).parse(input)?;

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
		Regex::from_pattern(r" ~?").unwrap();
	}

	#[test]
	fn hex_code_points() {
		{
			Regex::from_pattern(r"\u{20}").unwrap();
			Regex::from_pattern(r"\u{D7FF}").unwrap();
			Regex::from_pattern(r"\u{d7ff}").unwrap();
			Regex::from_pattern(r"\u{10FFFF}").unwrap();
			Regex::from_pattern(r"\u{10ffff}").unwrap();
		}
		{
			let e: RegexError<'_> = Regex::from_pattern(r"\u{z}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedHexDigits);
			assert_eq!(e.consumed, r"\u{");
			assert_eq!(e.remaining, "z}");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern(r"\u{D800}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidCodePoint(0xD800));
			assert_eq!(e.consumed, r"\u{");
			assert_eq!(e.remaining, "D800}");
		}
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
		{
			let e: RegexError<'_> = Regex::from_pattern("*").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EmptyTerm);
			assert_eq!(e.consumed, "");
			assert_eq!(e.remaining, "*");
		}
		{
			let e: RegexError<'_> = Regex::from_pattern("a**").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EmptyTerm);
			assert_eq!(e.consumed, "a*");
			assert_eq!(e.remaining, "*");
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
	fn capture_name() {
		{
			let e: RegexError<'_> = Regex::from_pattern("(?< ").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedCaptureName);
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
