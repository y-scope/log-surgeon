use std::collections::BTreeMap;
use std::num::NonZero;
use std::str::Chars;
use std::sync::Arc;

use nom::Err as NomErr;
use nom::IResult;
use nom::Parser;
use nom::error::ErrorKind as NomErrorKind;
use nom::error::FromExternalError;
use nom::error::ParseError;

use crate::parsing_spec::SubRule;
use crate::regex::AnchoredRegex;
use crate::regex::Regex;
use crate::regex::SPECIAL_CHARACTERS;
use crate::regex::SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS;

pub trait RegexPlaceholderLookup {
	fn lookup(&mut self, name: &str) -> Option<Regex>;
}

impl RegexPlaceholderLookup for BTreeMap<String, Regex> {
	fn lookup(&mut self, name: &str) -> Option<Regex> {
		self.get(name).cloned()
	}
}

impl<T> RegexPlaceholderLookup for T
where
	T: FnMut(&str) -> Option<Regex>,
{
	fn lookup(&mut self, name: &str) -> Option<Regex> {
		self(name)
	}
}

impl RegexPlaceholderLookup for () {
	fn lookup(&mut self, _name: &str) -> Option<Regex> {
		None
	}
}

// These fields are not currently explicitly used,
// but are relevant in the `Debug` implementation
// and would be needed to provide better (more specific)
// error messages in the future.
#[derive(Debug)]
pub struct RegexError {
	#[allow(unused)]
	consumed: String,
	#[allow(unused)]
	remaining: String,
	#[allow(unused)]
	kind: RegexErrorKind,
}

type ParsingResult<'a, T> = IResult<&'a str, T, RegexParsingError<'a>>;

#[derive(Debug, Clone, Eq, PartialEq)]
enum RegexErrorKind {
	/// Expected a certain character, e.g. '<' after '?' in a capture.
	ExpectedChar(char),
	/// Missing the closing delimiter for the following pair.
	ExpectedClose(char, char),
	/// "General" error kind, e.g. an isolated repetition suffix operator (e.g. the pattern "*").
	InvalidTerm,
	/// Expected a literal character in a bracketed expression.
	ExpectedLiteralInBracketedExpression,
	/// An empty bracketed expression "[]".
	/// Note that "[^]" is allowed, being equivalent to the wildcard ".".
	EmptyBrackets,
	/// Bracket range `min > max`.
	InvalidBracketRange(char, char),
	/// Invalid escape character.
	InvalidEscape,
	/// Invalid repetition bound; `min > max` or `max == 0`.
	InvalidRepetitionBound(u32, u32),
	/// Too large of a repetition bound.
	NumberTooBig,
	/// Expected decimal digits (for repetition bound).
	ExpectedDecimalDigits,
	/// Expected hex digits (for unicode escape).
	ExpectedHexDigits,
	/// Invalid code point in unicode escape.
	InvalidCodePoint(u32),
	/// Invalid capture name.
	InvalidCaptureName,
	/// Too many captures.
	TooManyCaptures,
	/// An escape class (e.g. "\\d") was used as the start/end point of a bracket range.
	EscapeClassInBracketRange,
	/// An inverted escape class (e.g. "\\D") was used inside a bracketed expression.
	InvertedEscapeClassInBrackets,
	/// Used for parsing a non-special character (`negate == true`)
	/// and for parsing an escaped special character (`negate == false`).
	/// This shouldn't actually bubble up publicly;
	/// it'll either get consumed by/turned into `ExpectedLiteralInBracketedExpression` or `InvalidTerm`,
	/// but exists because 1. it models "what's happening", and 2. it's useful for debugging.
	ExpectedOneOf { characters: &'static str, negate: bool },
	/// No definition for placeholder.
	UndefinedPlaceholder(String),
	/// An error from nom; shouldn't happen, but in implementation of [`nom::error::ParseError`]
	/// (useful for debugging/failing gracefully in a non-critical scenario).
	Nom(NomErrorKind),
}

#[derive(Debug)]
struct RegexParsingError<'a> {
	input: &'a str,
	kind: RegexErrorKind,
}

#[derive(Debug, Clone)]
enum Literal {
	Char(char),
	Bracketed { negated: bool, items: Vec<(char, char)> },
}

impl<'a> ParseError<&'a str> for RegexParsingError<'a> {
	fn from_error_kind(input: &'a str, kind: NomErrorKind) -> Self {
		Self {
			input,
			kind: RegexErrorKind::Nom(kind),
		}
	}

	fn append(_input: &'a str, _kind: NomErrorKind, other: Self) -> Self {
		other
	}
}

impl<'a> FromExternalError<&'a str, Self> for RegexParsingError<'a> {
	fn from_external_error(_input: &'a str, _kind: NomErrorKind, e: Self) -> Self {
		e
	}
}

impl<'a> RegexParsingError<'a> {
	fn new(input: &'a str, kind: RegexErrorKind) -> Self {
		Self { input, kind }
	}
}

impl Regex {
	pub fn from_pattern(pattern: &str) -> Result<AnchoredRegex, RegexError> {
		Self::from_pattern_with_placeholders(pattern, &mut ())
	}

	pub fn from_pattern_with_placeholders<T>(pattern: &str, lookup: &mut T) -> Result<AnchoredRegex, RegexError>
	where
		T: RegexPlaceholderLookup,
	{
		match parse_to_end(pattern) {
			Ok((remaining, mut regex)) => {
				assert_eq!(remaining, "");

				regex
					.inner
					.replace_with_placeholders(lookup)
					.map_err(|kind| RegexError {
						consumed: pattern.to_owned(),
						remaining: remaining.to_owned(),
						kind,
					})?;

				regex
					.inner
					.number_captures(&mut { NonZero::<u16>::MIN }, &mut Vec::new())
					.ok_or(RegexError {
						consumed: pattern.to_owned(),
						remaining: String::new(),
						kind: RegexErrorKind::TooManyCaptures,
					})?;
				Ok(regex)
			},
			Err(NomErr::Incomplete(_)) => {
				panic!("We shouldn't be using anything that can return this!");
			},
			Err(NomErr::Error(err) | NomErr::Failure(err)) => {
				let consumed: &str = pattern.strip_suffix(err.input).unwrap();
				Err(RegexError {
					consumed: consumed.to_owned(),
					remaining: err.input.to_owned(),
					kind: err.kind,
				})
			},
		}
	}

	fn replace_with_placeholders<F>(&mut self, get_placeholder: &mut F) -> Result<(), RegexErrorKind>
	where
		F: RegexPlaceholderLookup,
	{
		match self {
			Self::AnyChar | Self::Literal(..) | Self::BracketedRanges { .. } => Ok(()),
			Self::Capture(sub_rule) => Arc::get_mut(sub_rule)
				.unwrap()
				.regex
				.replace_with_placeholders(get_placeholder),
			Self::Placeholder { name, item } => {
				let Some(placeholder): Option<Regex> = get_placeholder.lookup(name) else {
					return Err(RegexErrorKind::UndefinedPlaceholder(name.clone()));
				};
				**item = placeholder;
				Ok(())
			},
			Self::KleeneClosure(item) | Self::KleenePlus(item) | Self::BoundedRepetition { item, .. } => {
				item.replace_with_placeholders(get_placeholder)
			},
			Self::Sequence(items) | Self::Alternation(items) => {
				for sub_item in items.iter_mut() {
					sub_item.replace_with_placeholders(get_placeholder)?;
				}
				Ok(())
			},
		}
	}

	/// [`RegexCapture::id`] defaults to [`NonZero::<u16>::MAX`];
	/// if we actually reach this, `next_id` will overflow,
	/// so it naturally works as a placeholder/invalid value.
	///
	/// Invariant: `parent_id < id`.
	fn number_captures(&mut self, id: &mut NonZero<u16>, stack: &mut Vec<(NonZero<u16>, Arc<str>)>) -> Option<usize> {
		let mut bread: usize = 0;
		match self {
			Self::AnyChar | Self::Literal(..) | Self::BracketedRanges { .. } => (),
			Self::Capture(sub_rule) => {
				let sub_rule: &mut SubRule = Arc::get_mut(sub_rule).unwrap();
				let maybe_parent: Option<&(NonZero<u16>, Arc<str>)> = stack.last();
				sub_rule.parent_id = maybe_parent.map(|(id, _)| *id);
				sub_rule.id = *id;
				sub_rule.qualified_name = Arc::from(format!(
					"{}.{}",
					maybe_parent.map_or("", |(_, name)| name),
					sub_rule.name
				));
				stack.push((sub_rule.id, sub_rule.qualified_name.clone()));
				// `id` is `u16`.
				*id = id.checked_add(1)?;
				sub_rule.descendents = sub_rule.regex.number_captures(id, stack)?;
				// `bread` is `usize`.
				bread = 1 + sub_rule.descendents;
				stack.pop();
			},
			Self::KleeneClosure(item)
			| Self::KleenePlus(item)
			| Self::BoundedRepetition { item, .. }
			| Self::Placeholder { item, .. } => {
				bread += item.number_captures(id, stack)?;
			},
			Self::Sequence(items) | Self::Alternation(items) => {
				for sub_item in items.iter_mut() {
					bread += sub_item.number_captures(id, stack)?;
				}
			},
		}
		Some(bread)
	}
}

impl RegexErrorKind {
	fn error(self, input: &str) -> NomErr<RegexParsingError<'_>> {
		NomErr::Error(RegexParsingError::new(input, self))
	}

	fn fail(self, input: &str) -> NomErr<RegexParsingError<'_>> {
		NomErr::Failure(RegexParsingError::new(input, self))
	}

	fn diagnostic<'a, T>(self) -> impl Fn(&'a str) -> ParsingResult<'a, T> {
		move |input| Err(self.clone().error(input))
	}
}

fn parse_to_end(input: &str) -> ParsingResult<'_, AnchoredRegex> {
	use nom::combinator::opt;

	let (input, anchor_before): (&str, bool) = opt(parse_char::<'^'>)
		.map(|maybe_anchor| maybe_anchor.is_some())
		.parse(input)?;

	// `parse_sequence` (and consequently `parse_alternation`) may swallow errors from
	// `parse_suffixed`, since the former two are "lists" that simply terminate when
	// no more elements (suffixed terms) can be parsed.
	// `parse_alternation` is called at the top level (here), or inside parentheses (possibly a capture).
	// Inside parentheses, after failing to parse a term (i.e. reaching the end of the list),
	// we look for the closing parenthesis.
	// Here, after reaching the end of the list, we ensure we're at the end of input,
	// otherwise "reproduce" the invalid term error.
	let (input, regex): (&str, Regex) = parse_alternation(input)?;

	let (input, anchor_after): (&str, bool) = opt(parse_char::<'$'>)
		.map(|maybe_anchor| maybe_anchor.is_some())
		.parse(input)?;

	if !input.is_empty() {
		return Err(RegexErrorKind::InvalidTerm.error(input));
	}

	Ok((
		input,
		AnchoredRegex {
			anchor_before,
			anchor_after,
			inner: regex,
		},
	))
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
		parse_repetition_suffix_modifier.map(|(start, end)| Suffix::Range(start, end)),
	)))
	.parse(input)?;

	if let Some(suffix) = maybe_suffix {
		match suffix {
			Suffix::Range(min, max) => Ok((
				input,
				Regex::BoundedRepetition {
					min,
					max,
					item: Box::new(regex),
				},
			)),
			Suffix::Star => Ok((input, Regex::KleeneClosure(Box::new(regex)))),
			Suffix::Plus => Ok((input, Regex::KleenePlus(Box::new(regex)))),
			Suffix::Question => Ok((
				input,
				Regex::BoundedRepetition {
					min: 0,
					max: 1,
					item: Box::new(regex),
				},
			)),
		}
	} else {
		Ok((input, regex))
	}
}

fn parse_repetition_suffix_modifier(original_input: &str) -> ParsingResult<'_, (u32, u32)> {
	let (input, (min, max)): (&str, (u32, u32)) =
		combinator_surrounded_cut::<'{', '}', _, _>(parse_repetition_bounds).parse(original_input)?;

	Ok((input, (min, max)))
}

fn parse_repetition_bounds(original_input: &str) -> ParsingResult<'_, (u32, u32)> {
	use nom::combinator::cut;
	use nom::combinator::opt;

	let (input, x): (&str, u32) = parse_digits(original_input)?;

	let (input_after_comma, have_comma): (&str, bool) = opt(parse_char::<','>)
		.map(|maybe_comma| maybe_comma.is_some())
		.parse(input)?;

	if have_comma {
		// Cut: After seeing a ',', we necessarily are expecting an upper bound.
		let (input, y): (&str, u32) = cut(parse_digits).parse(input_after_comma)?;
		if y > 0 {
			if x <= y {
				Ok((input, (x, y)))
			} else {
				Err(RegexErrorKind::InvalidRepetitionBound(x, y).error(input_after_comma))
			}
		} else {
			Err(RegexErrorKind::InvalidRepetitionBound(x, y).error(input_after_comma))
		}
	} else {
		if x > 0 {
			Ok((input, (x, x)))
		} else {
			Err(RegexErrorKind::InvalidRepetitionBound(x, x).error(original_input))
		}
	}
}

fn parse_term(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	alt((
		parse_char::<'.'>.map(|_| Regex::AnyChar),
		parse_literal_character.map(|literal| match literal {
			Literal::Char(ch) => Regex::Literal(ch),
			Literal::Bracketed { negated, items } => Regex::BracketedRanges { negated, items },
		}),
		parse_parenthesized,
		parse_bracketed_expression,
		RegexErrorKind::InvalidTerm.diagnostic(),
	))
	.parse(input)
}

fn parse_parenthesized(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	combinator_surrounded_cut::<'(', ')', _, _>(alt((parse_capture, parse_alternation))).parse(input)
}

fn parse_capture(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;

	let (input, _): (&str, char) = parse_char::<'?'>(input)?;

	// Cut: After seeing a '?', we necessarily are expecting a capture.
	let (input, name): (&str, &str) =
		cut(combinator_surrounded_cut::<'<', '>', _, _>(parse_capture_name)).parse(input)?;

	if input.starts_with(')') {
		Ok((
			input,
			Regex::Placeholder {
				name: name.to_owned(),
				item: Box::new(Regex::AnyChar),
			},
		))
	} else {
		let (input, regex): (&str, Regex) = parse_alternation(input)?;

		Ok((
			input,
			Regex::Capture(Arc::new(SubRule {
				name: name.to_owned(),
				regex,
				// This is a valid placeholder; see note for [`Regex::number_captures`].
				id: NonZero::<u16>::MAX,
				parent_id: None,
				descendents: 0,
				qualified_name: Arc::from(""),
			})),
		))
	}
}

// ========================================

fn parse_bracketed_expression(input: &str) -> ParsingResult<'_, Regex> {
	let (input, (negated, items)): (&str, (bool, Vec<(char, char)>)) =
		combinator_surrounded_cut::<'[', ']', _, _>(parse_bracketed_inside).parse(input)?;

	Ok((input, Regex::BracketedRanges { negated, items }))
}

fn parse_bracketed_inside(input: &str) -> ParsingResult<'_, (bool, Vec<(char, char)>)> {
	use nom::branch::alt;
	use nom::combinator::success;
	use nom::combinator::value;
	use nom::sequence::pair;

	#[derive(Clone, Copy, Eq, PartialEq)]
	enum FirstChar {
		Negation,
		EscapedCaret,
	}

	let (mut input, maybe_first_char): (&str, Option<FirstChar>) = alt((
		value(Some(FirstChar::Negation), parse_char::<'^'>),
		value(
			Some(FirstChar::EscapedCaret),
			pair(parse_char::<'\\'>, parse_char::<'^'>),
		),
		success(None),
	))
	.parse(input)?;

	let negated: bool = maybe_first_char == Some(FirstChar::Negation);

	let mut items: Vec<(char, char)> = Vec::new();

	if maybe_first_char == Some(FirstChar::EscapedCaret) {
		items.push(('^', '^'));
	}

	loop {
		let (new_input, new_items): (&str, Vec<(char, char)>) = parse_bracketed_item(input)?;
		if new_items.is_empty() {
			break;
		}
		input = new_input;
		items.extend(&new_items);
	}

	if !negated && items.is_empty() {
		return Err(RegexErrorKind::EmptyBrackets.error(input));
	}

	Ok((input, (negated, items)))
}

fn parse_bracketed_item(original_input: &str) -> ParsingResult<'_, Vec<(char, char)>> {
	use nom::combinator::opt;

	let (input, maybe_start): (&str, Option<Literal>) = parse_literal_char_in_bracketed_expression(original_input)?;

	let Some(start): Option<Literal> = maybe_start else {
		return Ok((input, Vec::new()));
	};

	let (input_after_dash, maybe_dash): (&str, Option<char>) = opt(parse_char::<'-'>).parse(input)?;

	if maybe_dash.is_some() {
		match start {
			Literal::Char(start) => {
				let (input, maybe_end): (&str, Option<Literal>) =
					parse_literal_char_in_bracketed_expression(input_after_dash)?;

				let Some(end): Option<Literal> = maybe_end else {
					return Ok((input, vec![(start, start), ('-', '-')]));
				};

				match end {
					Literal::Char(end) => {
						if start > end {
							return Err(RegexErrorKind::InvalidBracketRange(start, end).fail(input_after_dash));
						}
						Ok((input, vec![(start, end)]))
					},
					Literal::Bracketed { .. } => Err(RegexErrorKind::EscapeClassInBracketRange.fail(input_after_dash)),
				}
			},
			Literal::Bracketed { .. } => Err(RegexErrorKind::EscapeClassInBracketRange.fail(original_input)),
		}
	} else {
		match start {
			Literal::Char(ch) => Ok((input, vec![(ch, ch)])),
			Literal::Bracketed { negated, items } => {
				if negated {
					return Err(RegexErrorKind::InvertedEscapeClassInBrackets.fail(original_input));
				}
				Ok((input, items))
			},
		}
	}
}

// ========================================

fn parse_literal_character(input: &str) -> ParsingResult<'_, Literal> {
	use nom::branch::alt;

	alt((
		parse_escaped_character,
		parse_one_char_of::<true>(SPECIAL_CHARACTERS).map(Literal::Char),
	))
	.parse(input)
}

fn parse_escaped_character(original_input: &str) -> ParsingResult<'_, Literal> {
	use nom::branch::alt;
	use nom::combinator::cut;

	let (input, _): (&str, char) = parse_char::<'\\'>(original_input)?;

	// Cut: If we parsed a '\\', we necessarily are looking for an escape character.
	cut(alt((
		parse_one_char_of::<false>(SPECIAL_CHARACTERS).map(Literal::Char),
		// TODO: deprecate this
		parse_char::<'-'>.map(Literal::Char),
		parse_standard_escape,
	))
	// Outside of the `alt` since the error starts at the original input, still inside the `cut`.
	.or(|_| Err(RegexErrorKind::InvalidEscape.error(original_input))))
	.parse(input)
}

fn parse_literal_char_in_bracketed_expression(input: &str) -> ParsingResult<'_, Option<Literal>> {
	use nom::branch::alt;
	use nom::combinator::eof;
	use nom::combinator::peek;
	use nom::combinator::value;

	alt((
		parse_one_char_of::<true>(SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS)
			.map(Literal::Char)
			.map(Some),
		parse_escaped_character.map(Some),
		value(None, peek(parse_char::<']'>)),
		value(None, eof),
		|input| Err(RegexErrorKind::ExpectedLiteralInBracketedExpression.fail(input)),
	))
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
					return Err(RegexErrorKind::ExpectedOneOf {
						characters: any,
						negate: NEGATE,
					}
					.error(input));
				}
			} else if NEGATE {
				return Ok((chars.as_str(), ch));
			}
		}

		Err(RegexErrorKind::ExpectedOneOf {
			characters: any,
			negate: NEGATE,
		}
		.error(input))
	}
}

fn parse_standard_escape(input: &str) -> ParsingResult<'_, Literal> {
	use crate::utils::Escaped;
	use crate::utils::InvalidEscape;

	let mut chars: Chars<'_> = input.chars();

	// We use the NUL character as a marker/equivalent to EOF;
	// it's not a valid escape character, and will be caught in the default branch of the `match` block below.
	let ch: char = chars.next().unwrap_or('\0');

	if matches!(ch, 'd' | 's' | 'w' | 'D' | 'S' | 'W') {
		let ch_lowercase: char = ch.to_ascii_lowercase();
		return Ok((
			chars.as_str(),
			Literal::Bracketed {
				negated: ch != ch_lowercase,
				items: match ch_lowercase {
					'd' => vec![('0', '9')],
					's' => vec![(' ', ' '), ('\t', '\t'), ('\r', '\r'), ('\n', '\n')],
					'w' => vec![('0', '9'), ('a', 'z'), ('A', 'Z')],
					_ => {
						// TODO better message
						unreachable!();
					},
				},
			},
		));
	}
	match Escaped::unescape(input) {
		Ok((input, ch)) => Ok((input, Literal::Char(ch))),
		Err(InvalidEscape::Eof) => Err(RegexErrorKind::InvalidEscape.error(input)),
		Err(InvalidEscape::Malformed) => Err(RegexErrorKind::ExpectedHexDigits.fail(input)),
		Err(InvalidEscape::BadCodePoint(x)) => Err(RegexErrorKind::InvalidCodePoint(x).fail(input)),
		Err(InvalidEscape::Unknown(_)) => Err(RegexErrorKind::InvalidEscape.error(input)),
	}
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
		.or(RegexErrorKind::InvalidCaptureName.diagnostic())
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
			RegexErrorKind::ExpectedDecimalDigits,
		))),
	}
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
			cut(parse_char::<CLOSE>.or(RegexErrorKind::ExpectedClose(OPEN, CLOSE).diagnostic())).parse(input)?;

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

		// Both are fine.
		Regex::from_pattern(r"a-b").unwrap();
		Regex::from_pattern(r"a\-b").unwrap();
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
			let e: RegexError = Regex::from_pattern(r"\u{z}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedHexDigits);
			assert_eq!(e.consumed, r"\");
			assert_eq!(e.remaining, "u{z}");
			// TODO: add these back/clean up error location
			// assert_eq!(e.consumed, r"\u{");
			// assert_eq!(e.remaining, "z}");
		}
		{
			let e: RegexError = Regex::from_pattern(r"\u{D800}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidCodePoint(0xD800));
			assert_eq!(e.consumed, r"\");
			assert_eq!(e.remaining, "u{D800}");
			// TODO: add these back/clean up error location
			// assert_eq!(e.consumed, r"\u{");
			// assert_eq!(e.remaining, "D800}");
		}
	}

	#[test]
	fn special_characters_in_bracketed_range() {
		{
			Regex::from_pattern("[^]").unwrap();
			Regex::from_pattern(r"[^^]").unwrap();
			Regex::from_pattern(r"[.]").unwrap();
			Regex::from_pattern(r"[$]").unwrap();
			Regex::from_pattern(r"[\.]").unwrap();
			Regex::from_pattern(r"[\$]").unwrap();
			Regex::from_pattern(r"[\\]").unwrap();
			Regex::from_pattern(r"[a-]").unwrap();
			Regex::from_pattern(r"[-a]").unwrap();
			Regex::from_pattern(r"[^-a]").unwrap();
			Regex::from_pattern(r"[-]").unwrap();
			Regex::from_pattern(r"[^-]").unwrap();
		}
		{
			let e: RegexError = Regex::from_pattern(r"[\]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedClose('[', ']'));
			assert_eq!(e.consumed, r"[\]");
			assert_eq!(e.remaining, "");
		}
		{
			let e: RegexError = Regex::from_pattern(r"[\a]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidEscape);
			assert_eq!(e.consumed, r"[");
			assert_eq!(e.remaining, r"\a]");
		}
		{
			let e: RegexError = Regex::from_pattern(r"[[").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedLiteralInBracketedExpression);
			assert_eq!(e.consumed, r"[");
			assert_eq!(e.remaining, r"[");
		}
	}

	#[test]
	fn invalid_term() {
		{
			let e: RegexError = Regex::from_pattern("|abc").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidTerm);
			assert_eq!(e.consumed, "");
			assert_eq!(e.remaining, "|abc");
		}
		{
			let e: RegexError = Regex::from_pattern("abc|").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidTerm);
			assert_eq!(e.consumed, "abc|");
			assert_eq!(e.remaining, "");
		}
		{
			let e: RegexError = Regex::from_pattern("a||bc").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidTerm);
			assert_eq!(e.consumed, "a|");
			assert_eq!(e.remaining, "|bc");
		}
		{
			let e: RegexError = Regex::from_pattern("*").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidTerm);
			assert_eq!(e.consumed, "");
			assert_eq!(e.remaining, "*");
		}
		{
			let e: RegexError = Regex::from_pattern("a**").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidTerm);
			assert_eq!(e.consumed, "a*");
			assert_eq!(e.remaining, "*");
		}
	}

	#[test]
	fn unclosed_parentheses() {
		{
			let e: RegexError = Regex::from_pattern("(abc").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedClose('(', ')'));
			assert_eq!(e.consumed, "(abc");
			assert_eq!(e.remaining, "");
		}
		{
			let e: RegexError = Regex::from_pattern("(?<abc*").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedClose('<', '>'));
			assert_eq!(e.consumed, "(?<abc");
			assert_eq!(e.remaining, "*");
		}
		{
			let e: RegexError = Regex::from_pattern("(abc[def)").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedClose('[', ']'));
			assert_eq!(e.consumed, "(abc[def)");
			assert_eq!(e.remaining, "");
		}
		{
			let e: RegexError = Regex::from_pattern(".{123a}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedClose('{', '}'));
			assert_eq!(e.consumed, ".{123");
			assert_eq!(e.remaining, "a}");
		}
	}

	#[test]
	fn expected_decimal() {
		{
			let e: RegexError = Regex::from_pattern(".{ }").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedDecimalDigits);
			assert_eq!(e.consumed, ".{");
			assert_eq!(e.remaining, " }");
		}
		{
			let e: RegexError = Regex::from_pattern(".{123,").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedDecimalDigits);
			assert_eq!(e.consumed, ".{123,");
			assert_eq!(e.remaining, "");
		}
	}

	#[test]
	fn number_too_big() {
		{
			let pattern: String = format!(".{{{}}}", "9".repeat(64));
			let e: RegexError = Regex::from_pattern(&pattern).unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::NumberTooBig);
			assert_eq!(e.consumed, ".{");
			assert_eq!(e.remaining, &pattern[".{".len()..]);
		}
	}

	#[test]
	fn capture_name() {
		{
			let e: RegexError = Regex::from_pattern("(?< ").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidCaptureName);
			assert_eq!(e.consumed, "(?<");
			assert_eq!(e.remaining, " ");
		}
	}

	#[test]
	fn expected_char() {
		{
			let e: RegexError = Regex::from_pattern("(?a").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedChar('<'));
			assert_eq!(e.consumed, "(?");
			assert_eq!(e.remaining, "a");
		}
	}

	#[test]
	fn empty_brackets() {
		{
			let e: RegexError = Regex::from_pattern("[]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EmptyBrackets);
			assert_eq!(e.consumed, "[");
			assert_eq!(e.remaining, "]");
		}
	}

	#[test]
	fn invalid_escapes() {
		{
			let e: RegexError = Regex::from_pattern(r"[ \a]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidEscape);
			assert_eq!(e.consumed, "[ ");
			assert_eq!(e.remaining, r"\a]");
		}
	}

	#[test]
	fn escape_class_in_bracketed_range() {
		{
			let e: RegexError = Regex::from_pattern(r"[\d-b]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EscapeClassInBracketRange);
			assert_eq!(e.consumed, r"[");
			assert_eq!(e.remaining, r"\d-b]");
		}
		{
			let e: RegexError = Regex::from_pattern(r"[b-\w]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::EscapeClassInBracketRange);
			assert_eq!(e.consumed, r"[b-");
			assert_eq!(e.remaining, r"\w]");
		}
	}

	#[test]
	fn inverted_escape_class_in_bracketed_range() {
		{
			let e: RegexError = Regex::from_pattern(r"[\W]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvertedEscapeClassInBrackets);
			assert_eq!(e.consumed, r"[");
			assert_eq!(e.remaining, r"\W]");
		}
	}

	#[test]
	fn reptition_bounds() {
		{
			let e: RegexError = Regex::from_pattern(r"a{2,1}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidRepetitionBound(2, 1));
			assert_eq!(e.consumed, r"a{2,");
			assert_eq!(e.remaining, r"1}");
		}
		{
			let e: RegexError = Regex::from_pattern(r"a{0,0}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidRepetitionBound(0, 0));
			assert_eq!(e.consumed, r"a{0,");
			assert_eq!(e.remaining, r"0}");
		}
		{
			let e: RegexError = Regex::from_pattern(r"a{0}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidRepetitionBound(0, 0));
			assert_eq!(e.consumed, r"a{");
			assert_eq!(e.remaining, r"0}");
		}
	}

	#[test]
	fn reversed_bracketed_range() {
		{
			let e: RegexError = Regex::from_pattern(r"[z-a]").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidBracketRange('z', 'a'));
			assert_eq!(e.consumed, r"[z-");
			assert_eq!(e.remaining, r"a]");
		}
	}
}
