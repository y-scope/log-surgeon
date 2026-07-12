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
use crate::regex::RegexError;
use crate::regex::RegexErrorKind;
use crate::regex::SPECIAL_CHARACTERS;
use crate::regex::SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS;
use crate::utils::NomUtils;

/// A helper trait for filling in placeholders when parsing regex patterns.
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

type ParsingResult<'a, T> = IResult<&'a str, T, RegexParsingError<'a>>;

#[derive(Debug)]
struct RegexParsingError<'a> {
	input: &'a str,
	kind: RegexErrorKind,
}

#[derive(Debug, Clone)]
enum Term {
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

	fn from_char(input: &'a str, ch: char) -> Self {
		Self {
			input,
			kind: RegexErrorKind::ExpectedChar(ch),
		}
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

impl AnchoredRegex {
	pub fn from_pattern_with_placeholders<T>(mut pattern: &str, lookup: &mut T) -> Result<Self, RegexError>
	where
		T: RegexPlaceholderLookup,
	{
		let mut anchor_before: bool = false;
		let mut anchor_after: bool = false;

		if let Some(suffix) = pattern.strip_prefix('^') {
			anchor_before = true;
			pattern = suffix;
		}

		if let Some(prefix) = pattern.strip_suffix('$') {
			anchor_after = true;
			pattern = prefix;
		}

		let mut regex: Regex = Regex::from_pattern_with_placeholders::<false, _>(pattern, lookup)?;

		let mut total_captures: NonZero<u16> = NonZero::<u16>::MIN;

		regex
			.initialize_captures(&mut total_captures, &mut Vec::new())
			.ok_or(RegexError {
				consumed: pattern.to_owned(),
				remaining: String::new(),
				kind: RegexErrorKind::TooManyCaptures,
			})?;

		Ok(Self {
			anchor_before,
			anchor_after,
			regex,
			total_captures,
		})
	}
}

impl Regex {
	pub fn from_pattern(pattern: &str) -> Result<Self, RegexError> {
		Self::from_pattern_with_placeholders::<false, _>(pattern, &mut ())
	}

	/// Parse an unanchored regex pattern.
	/// Replaces placeholders with their subexpressions,
	/// but does not call [`Regex::initialize_captures`],
	/// since the ID numbering is per-root rule/pattern
	/// (i.e. it is done at the [`AnchoredRegex`] level).
	pub fn from_pattern_with_placeholders<const ALLOW_NULLABLE: bool, T>(
		pattern: &str,
		lookup: &mut T,
	) -> Result<Self, RegexError>
	where
		T: RegexPlaceholderLookup,
	{
		match parse_to_end(pattern) {
			Ok((remaining, mut regex)) => {
				assert_eq!(remaining, "");

				regex.replace_with_placeholders(lookup).map_err(|kind| RegexError {
					consumed: pattern.to_owned(),
					remaining: remaining.to_owned(),
					kind,
				})?;

				if !ALLOW_NULLABLE {
					if let Some(item) = regex.is_nullable() {
						return Err(RegexError {
							consumed: pattern.to_owned(),
							remaining: String::new(),
							kind: RegexErrorKind::NullableExpression(Box::new(item.clone())),
						});
					}
				}

				Ok(regex)
			},
			Err(NomErr::Incomplete(_)) => {
				panic!("we shouldn't be using anything that can return this");
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

	/// Initializes sub-rule data for capture expressions.
	/// Should be called with `next_id == NonZero::<u16>::MAX`.
	/// Returns 1 plus the number of sub-rules; i.e. the root rule plus sub-rules.
	///
	/// [`SubRule::id`] defaults to [`NonZero::<u16>::MAX`];
	/// this max value isn't otherwise a valid value ID
	/// since if we were to assign a sub-rule ID with the max value here,
	/// `next_id` would overflow (and we would return `None`).
	///
	/// Invariant: `parent_id < id`.
	fn initialize_captures(
		&mut self,
		next_id: &mut NonZero<u16>,
		stack: &mut Vec<(NonZero<u16>, Arc<str>)>,
	) -> Option<usize> {
		let mut bread: usize = 0;
		match self {
			Self::AnyChar | Self::Literal(..) | Self::BracketedRanges { .. } => (),
			Self::Capture(sub_rule) => {
				let sub_rule: &mut SubRule = Arc::get_mut(sub_rule).unwrap();
				let maybe_parent: Option<&(NonZero<u16>, Arc<str>)> = stack.last();
				sub_rule.parent_id = maybe_parent.map(|(id, _)| *id);
				sub_rule.id = *next_id;
				sub_rule.qualified_name = Arc::from(format!(
					"{}.{}",
					maybe_parent.map_or("", |(_, name)| name),
					sub_rule.name
				));
				stack.push((sub_rule.id, sub_rule.qualified_name.clone()));
				// `id` is `u16`.
				*next_id = next_id.checked_add(1)?;
				sub_rule.descendents = sub_rule.regex.initialize_captures(next_id, stack)?;
				// `bread` is `usize`.
				bread = 1 + sub_rule.descendents;
				stack.pop();
			},
			Self::KleeneClosure(item)
			| Self::KleenePlus(item)
			| Self::BoundedRepetition { item, .. }
			| Self::Placeholder { item, .. } => {
				bread += item.initialize_captures(next_id, stack)?;
			},
			Self::Sequence(items) | Self::Alternation(items) => {
				for sub_item in items.iter_mut() {
					bread += sub_item.initialize_captures(next_id, stack)?;
				}
			},
		}
		Some(bread)
	}
}

impl From<std::convert::Infallible> for RegexError {
	fn from(infallible: std::convert::Infallible) -> Self {
		infallible.into()
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
		return Err(RegexErrorKind::InvalidTerm.error(input));
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

fn parse_repetition_suffix_modifier(input: &str) -> ParsingResult<'_, (u32, u32)> {
	let (input, (min, max)): (&str, (u32, u32)) =
		surrounded_cut::<'{', '}', _, _>(parse_repetition_bounds).parse(input)?;

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
			Term::Char(ch) => Regex::Literal(ch),
			Term::Bracketed { negated, items } => Regex::BracketedRanges { negated, items },
		}),
		parse_parenthesized,
		parse_bracketed_expression,
		RegexErrorKind::InvalidTerm.diagnostic(),
	))
	.parse(input)
}

fn parse_parenthesized(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	surrounded_cut::<'(', ')', _, _>(alt((parse_capture, parse_alternation))).parse(input)
}

fn parse_capture(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;

	let (input, _): (&str, char) = parse_char::<'?'>(input)?;

	// Cut: After seeing a '?', we necessarily are expecting a capture.
	let (input, name): (&str, &str) = cut(surrounded_cut::<'<', '>', _, _>(parse_capture_name)).parse(input)?;

	if input.starts_with(')') {
		// This function is called from [`parse_parenthesized`] inside [`surrounded_cut`];
		// we do not consume the opening or closing parentheses.
		// Instead, "peek" for the closing parenthesis to determine if we are an empty capture,
		// indicating a regex placeholder.
		//
		// We can't just "try" the [`parse_alternation`],
		// since that may fail for other reasons than empty input.
		//
		// Note: There is a [`nom::combinator::peek`] combinator,
		// but it's arguably easier to read like this.

		Ok((
			input,
			Regex::Placeholder {
				name: name.to_owned(),
				item: Box::new(Regex::NIL),
			},
		))
	} else {
		let (input, regex): (&str, Regex) = parse_alternation(input)?;

		Ok((
			input,
			Regex::Capture(Arc::new(SubRule {
				name: name.to_owned(),
				regex,
				// [`Regex::initialize_captures`], called after the AST is parsed, sets these next 4 values;
				// see also its comment on why this `MAX` is a valid temporary value.
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
		surrounded_cut::<'[', ']', _, _>(parse_bracketed_inside).parse(input)?;

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
		let new_items: Vec<(char, char)>;
		(input, new_items) = parse_bracketed_item(input)?;
		if new_items.is_empty() {
			break;
		}
		items.extend(&new_items);
	}

	if !negated && items.is_empty() {
		return Err(RegexErrorKind::EmptyBrackets.error(input));
	}

	Ok((input, (negated, items)))
}

fn parse_bracketed_item(original_input: &str) -> ParsingResult<'_, Vec<(char, char)>> {
	use nom::combinator::opt;

	let (input, maybe_start): (&str, Option<Term>) = parse_literal_char_in_bracketed_expression(original_input)?;

	let Some(start): Option<Term> = maybe_start else {
		return Ok((input, Vec::new()));
	};

	let (input_after_dash, maybe_dash): (&str, Option<char>) = opt(parse_char::<'-'>).parse(input)?;

	if maybe_dash.is_some() {
		match start {
			Term::Char(start) => {
				let (input, maybe_end): (&str, Option<Term>) =
					parse_literal_char_in_bracketed_expression(input_after_dash)?;

				let Some(end): Option<Term> = maybe_end else {
					return Ok((input, vec![(start, start), ('-', '-')]));
				};

				match end {
					Term::Char(end) => {
						if start > end {
							return Err(RegexErrorKind::InvalidBracketRange(start, end).fail(input_after_dash));
						}
						Ok((input, vec![(start, end)]))
					},
					Term::Bracketed { .. } => Err(RegexErrorKind::EscapeClassInBracketRange.fail(input_after_dash)),
				}
			},
			Term::Bracketed { negated, mut items } => {
				if negated {
					return Err(RegexErrorKind::InvertedEscapeClassInBrackets.fail(original_input));
				}
				if let Some(ch) = input_after_dash.chars().next() {
					if ch == ']' {
						// Cases like `[\d-]`.
						items.push(('-', '-'));
						Ok((input, items))
					} else {
						// Cases like `[\d-abc]`.
						Err(RegexErrorKind::EscapeClassInBracketRange.fail(original_input))
					}
				} else {
					// Cases like `[\d-`.
					// Not an explicit here (as in the other branches);
					// farther up we'll return [`RegexErrorKind::ExpectedClose`] instead.
					Ok((input, items))
				}
			},
		}
	} else {
		match start {
			Term::Char(ch) => Ok((input, vec![(ch, ch)])),
			Term::Bracketed { negated, items } => {
				if negated {
					return Err(RegexErrorKind::InvertedEscapeClassInBrackets.fail(original_input));
				}
				Ok((input, items))
			},
		}
	}
}

// ========================================

fn parse_literal_character(input: &str) -> ParsingResult<'_, Term> {
	use nom::branch::alt;

	alt((
		parse_escaped_character,
		parse_one_char_of::<true>(SPECIAL_CHARACTERS).map(Term::Char),
	))
	.parse(input)
}

fn parse_escaped_character(original_input: &str) -> ParsingResult<'_, Term> {
	use nom::branch::alt;
	use nom::combinator::cut;

	let (input, _): (&str, char) = NomUtils::parse_char::<'\\', RegexParsingError<'_>>(original_input)?;

	// Cut: If we parsed a '\\', we necessarily are looking for an escape character.
	cut(alt((
		parse_one_char_of::<false>(SPECIAL_CHARACTERS).map(Term::Char),
		// TODO: deprecate allowing this outside bracketed expressions.
		parse_char::<'-'>.map(Term::Char),
		parse_standard_escape,
	))
	// Outside of the `alt` since the error starts at the original input, still inside the `cut`.
	.or(|_| Err(RegexErrorKind::InvalidEscape.error(original_input))))
	.parse(input)
}

fn parse_literal_char_in_bracketed_expression(input: &str) -> ParsingResult<'_, Option<Term>> {
	use nom::branch::alt;
	use nom::combinator::eof;
	use nom::combinator::peek;
	use nom::combinator::value;

	alt((
		parse_one_char_of::<true>(SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS)
			.map(Term::Char)
			.map(Some),
		parse_escaped_character.map(Some),
		value(None, peek(parse_char::<']'>)),
		// This branch is for a more intuitive [`RegexErrorKind::ExpectedClose`] error,
		// as opposed to `ExpectedLiteralInBracketedExpression` for a pattern like `[abc`.
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
			if any.contains(ch) == NEGATE {
				return Err(RegexErrorKind::ExpectedOneOf {
					characters: any,
					negate: NEGATE,
				}
				.error(input));
			} else {
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

fn parse_standard_escape(input: &str) -> ParsingResult<'_, Term> {
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
			Term::Bracketed {
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
		Ok((input, ch)) => Ok((input, Term::Char(ch))),
		Err(InvalidEscape::Eof) => Err(RegexErrorKind::InvalidEscape.error(input)),
		Err(InvalidEscape::Malformed) => Err(RegexErrorKind::ExpectedHexDigits.fail(input)),
		Err(InvalidEscape::BadCodePoint(x)) => Err(RegexErrorKind::InvalidCodePoint(x).fail(input)),
		Err(InvalidEscape::Unknown(_)) => Err(RegexErrorKind::InvalidEscape.error(input)),
	}
}

// =======================================

fn parse_capture_name(original_input: &str) -> ParsingResult<'_, &str> {
	// Don't error here if we didn't find a closing `>`;
	// let the caller return [`RegexErrorKind::ExpectedClose`].
	let pos: usize = original_input.find('>').unwrap_or(original_input.len());
	let (name, input): (&str, &str) = original_input.split_at(pos);

	if name.is_empty() {
		return Err(RegexErrorKind::InvalidCaptureName.error(original_input));
	}

	if let Some(pos) = name.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
		return Err(RegexErrorKind::InvalidCaptureName.error(&original_input[pos..]));
	}

	Ok((input, name))
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
		Err(err @ NomErr::Incomplete(_)) => Err(err),
		Err(NomErr::Error(_) | NomErr::Failure(_)) => Err(NomErr::Error(RegexParsingError::new(
			input,
			RegexErrorKind::ExpectedDecimalDigits,
		))),
	}
}

// ==================================

fn parse_char<const CHAR: char>(input: &str) -> ParsingResult<'_, char> {
	NomUtils::parse_char::<CHAR, RegexParsingError<'_>>(input)
}

fn surrounded_cut<'a, const OPEN: char, const CLOSE: char, O, F>(
	inside: F,
) -> impl Parser<&'a str, Output = O, Error = RegexParsingError<'a>>
where
	F: Parser<&'a str, Output = O, Error = RegexParsingError<'a>>,
{
	NomUtils::surrounded_cut::<OPEN, CLOSE, _, _, _, _>(inside, RegexErrorKind::ExpectedClose(OPEN, CLOSE).diagnostic())
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

		{
			let a: Regex = Regex::from_pattern(r"(((abc)))").unwrap();
			let b: Regex = Regex::from_pattern(r"abc").unwrap();
			assert_eq!(a, b);
		}
		{
			let a: Regex = Regex::from_pattern(r"([abc])").unwrap();
			let b: Regex = Regex::from_pattern(r"[abc]").unwrap();
			assert_eq!(a, b);
		}
	}

	#[test]
	fn hex_code_points() {
		{
			const POINTS: &[char] = &['\u{20}', '\u{D7FF}', '\u{10FFFF}'];

			for &p in POINTS.iter() {
				assert_eq!(
					Regex::from_pattern(&format!("\\u{{{:x}}}", u32::from(p))).unwrap(),
					Regex::Literal(p)
				);
			}

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
		{
			let e: RegexError = Regex::from_pattern(r"{3}").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidTerm);
			assert_eq!(e.consumed, r"");
			assert_eq!(e.remaining, r"{3}");
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
			let e: RegexError = Regex::from_pattern("(?<abc").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::ExpectedClose('<', '>'));
			assert_eq!(e.consumed, "(?<abc");
			assert_eq!(e.remaining, "");
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
			Regex::from_pattern("(?<foo_bar>.)").unwrap();
		}
		{
			let e: RegexError = Regex::from_pattern("(?<>").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidCaptureName);
			assert_eq!(e.consumed, "(?<");
			assert_eq!(e.remaining, ">");
		}
		{
			let e: RegexError = Regex::from_pattern("(?< ").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidCaptureName);
			assert_eq!(e.consumed, "(?<");
			assert_eq!(e.remaining, " ");
		}
		{
			let e: RegexError = Regex::from_pattern("(?<abc-def>").unwrap_err();
			assert_eq!(e.kind, RegexErrorKind::InvalidCaptureName);
			assert_eq!(e.consumed, "(?<abc");
			assert_eq!(e.remaining, "-def>");
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
	fn repetition_bounds() {
		{
			Regex::from_pattern("abc{3}").unwrap();

			std::assert_matches!(
				Regex::from_pattern("(abc){3}").unwrap(),
				Regex::BoundedRepetition { min: 3, max: 3, .. }
			);

			std::assert_matches!(
				Regex::from_pattern("(abc){6,7}").unwrap(),
				Regex::BoundedRepetition { min: 6, max: 7, .. }
			);
		}
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

	#[test]
	fn nullable_subexpression() {
		{
			let e: RegexError = Regex::from_pattern(r"a{0,3}").unwrap_err();
			assert_eq!(
				e.kind,
				RegexErrorKind::NullableExpression(Box::new(Regex::BoundedRepetition {
					min: 0,
					max: 3,
					item: Box::new(Regex::Literal('a')),
				}))
			);
		}
		{
			let e: RegexError = Regex::from_pattern(r"a|b?").unwrap_err();
			assert_eq!(
				e.kind,
				RegexErrorKind::NullableExpression(Box::new(Regex::BoundedRepetition {
					min: 0,
					max: 1,
					item: Box::new(Regex::Literal('b')),
				}))
			);
		}
		{
			let e: RegexError = Regex::from_pattern(r"a|b*").unwrap_err();
			assert_eq!(
				e.kind,
				RegexErrorKind::NullableExpression(Box::new(Regex::KleeneClosure(Box::new(Regex::Literal('b')),)))
			);
		}
		{
			let e: RegexError = Regex::from_pattern(r"(a|b*)+").unwrap_err();
			assert_eq!(
				e.kind,
				RegexErrorKind::NullableExpression(Box::new(Regex::KleeneClosure(Box::new(Regex::Literal('b')),)))
			);
		}
	}

	#[test]
	fn standard_escape_classes() {
		const CLASSES: &[(&str, &[(char, char)])] = &[
			(r"\d", &[('0', '9')]),
			(r"\w", &[('0', '9'), ('a', 'z'), ('A', 'Z')]),
			(r"\s", &[(' ', ' '), ('\t', '\t'), ('\r', '\r'), ('\n', '\n')]),
		];

		for (pattern, items) in CLASSES.iter() {
			assert_eq!(
				Regex::from_pattern(pattern).unwrap(),
				Regex::BracketedRanges {
					negated: false,
					items: items.to_vec(),
				}
			);
			assert_eq!(
				Regex::from_pattern(&pattern.to_ascii_uppercase()).unwrap(),
				Regex::BracketedRanges {
					negated: true,
					items: items.to_vec(),
				}
			);
		}
	}
}
