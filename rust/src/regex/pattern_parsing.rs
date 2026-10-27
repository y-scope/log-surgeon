use std::collections::BTreeMap;
use std::str::Chars;
use std::sync::Arc;

use nom::Err as NomErr;
use nom::IResult;
use nom::Parser;
use nom::error::ErrorKind as NomErrorKind;
use nom::error::FromExternalError;
use nom::error::ParseError;

use crate::regex::AnchoredRegex;
use crate::regex::Capture;
use crate::regex::CaptureInfo;
use crate::regex::Regex;
use crate::regex::RegexError;
use crate::regex::RegexErrorKind;
use crate::regex::SPECIAL_CHARACTERS;
use crate::regex::SPECIAL_CHARACTERS_IN_BRACKETED_RANGES;
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
	Bracketed {
		negated: bool,
		items: Vec<(char, char)>,
	},
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
	/// Like [`Regex::from_pattern_with_placeholders`],
	/// but for a (potentially) anchored regex/pattern.
	///
	/// The parsed expression is implicitly wrapped in a [`Regex::Capture`] named `base_name`
	/// (the root rule name), so that captures are numbered uniformly and the root rule itself
	/// has a [`CaptureInfo`] entry (id `0`).
	pub fn from_pattern_with_placeholders<T>(
		mut pattern: &str,
		base_name: Arc<str>,
		lookup: &mut T,
	) -> Result<Self, RegexError>
	where
		T: RegexPlaceholderLookup,
	{
		let mut anchor_before: bool = false;
		let anchor_after: bool;

		if let Some(suffix) = pattern.strip_prefix('^') {
			anchor_before = true;
			pattern = suffix;
		}

		// We can't strip the suffix the same way, because it may be escaped.
		let regex: Regex = match parse_to_end_possibly_anchor(pattern) {
			Ok((remaining, (mut regex, anchor))) => {
				assert_eq!(remaining, "");

				regex
					.replace_with_placeholders(lookup)
					.map_err(|kind| RegexError {
						consumed: pattern.to_owned(),
						remaining: String::new(),
						kind,
					})?;

				anchor_after = anchor;

				regex
			},
			Err(NomErr::Incomplete(_)) => {
				unreachable!("we shouldn't be using anything that can return this");
			},
			Err(NomErr::Error(err) | NomErr::Failure(err)) => {
				let consumed: &str = pattern.strip_suffix(err.input).unwrap();
				return Err(RegexError {
					consumed: consumed.to_owned(),
					remaining: err.input.to_owned(),
					kind: err.kind,
				});
			},
		};

		let regex: Regex = regex.ensure_not_nullable(pattern)?;

		let mut regex: Regex = Regex::Capture(Capture::new(base_name, Box::new(regex)));

		let mut captures: Vec<CaptureInfo> = Vec::new();
		if regex
			.initialize_captures(&mut 0, &mut captures, &mut Vec::new())
			.is_none()
		{
			return Err(RegexError {
				consumed: pattern.to_owned(),
				remaining: String::new(),
				kind: RegexErrorKind::TooManyCaptures,
			});
		}

		Ok(Self {
			anchor_before,
			anchor_after,
			regex,
			captures,
		})
	}
}

/// See the [Parsing Specification File document][parsing-spec-file]
/// for more details on regex syntax and semantics.
///
/// In summary, a regular expression is composed of terms
/// ([`parse_term`] and [`parse_parenthesized`])
/// and operators to recursively combine subexpressions;
/// from highest to lowest precedence:
///
/// - suffixed repetition operators ([`parse_suffixed`]),
/// - concatenation ([`parse_sequence`]), and
/// - alternation ([`parse_alternation`]).
///
/// TODO after merge: update link
/// [parsing-spec-file]:
/// https://github.com/y-scope/log-surgeon/tree/log-mechanic/rust/docs/parsing-specification.md
impl Regex {
	/// Parse an unanchored regex pattern.
	/// Replaces placeholders with their subexpressions and numbers captures starting at `1`
	/// (`0` is reserved for the implicit root capture of an [`AnchoredRegex`]).
	pub fn from_pattern_with_placeholders<T>(
		pattern: &str,
		lookup: &mut T,
	) -> Result<Self, RegexError>
	where
		T: RegexPlaceholderLookup,
	{
		match parse_to_end(pattern) {
			Ok((remaining, mut regex)) => {
				assert_eq!(remaining, "");

				regex
					.replace_with_placeholders(lookup)
					.map_err(|kind| RegexError {
						consumed: pattern.to_owned(),
						remaining: String::new(),
						kind,
					})?;

				if regex
					.initialize_captures(&mut 1, &mut Vec::new(), &mut Vec::new())
					.is_none()
				{
					return Err(RegexError {
						consumed: pattern.to_owned(),
						remaining: String::new(),
						kind: RegexErrorKind::TooManyCaptures,
					});
				}

				Ok(regex)
			},
			Err(NomErr::Incomplete(_)) => {
				unreachable!("we shouldn't be using anything that can return this");
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

	fn replace_with_placeholders<F>(
		&mut self,
		placeholder_lookup: &mut F,
	) -> Result<(), RegexErrorKind>
	where
		F: RegexPlaceholderLookup,
	{
		match self {
			Self::AnyChar | Self::Literal(..) | Self::BracketedRanges { .. } => Ok(()),
			Self::Capture(capture) => capture.item.replace_with_placeholders(placeholder_lookup),
			Self::Placeholder { name, item } => {
				let Some(placeholder): Option<Regex> = placeholder_lookup.lookup(name) else {
					return Err(RegexErrorKind::UndefinedPlaceholder(name.to_string()));
				};
				**item = placeholder.clone();
				Ok(())
			},
			Self::KleeneClosure(item)
			| Self::KleenePlus(item)
			| Self::BoundedRepetition { item, .. } => item.replace_with_placeholders(placeholder_lookup),
			Self::Sequence(items) | Self::Alternation(items) => {
				for sub_item in items.iter_mut() {
					sub_item.replace_with_placeholders(placeholder_lookup)?;
				}
				Ok(())
			},
		}
	}

	/// Short form of [`Regex::from_pattern_with_placeholders`] for writing tests;
	/// parses non-nullable patterns without placeholders.
	pub fn from_pattern(pattern: &str) -> Result<Self, RegexError> {
		Self::from_pattern_with_placeholders(pattern, &mut ())?.ensure_not_nullable(pattern)
	}

	/// Initializes [`Capture::id`] and appends the corresponding [`CaptureInfo`] for each capture.
	///
	/// Captures are numbered in pre-order. When `next_id` starts at `0` and the expression is a
	/// root [`Regex::Capture`], `id == index` in `captures`, and `parent_id < id`; the root capture
	/// gets id `0` at index `0`. `ancestors` carries the enclosing `(id, qualified_name)` chain and
	/// is initially empty.
	///
	/// Returns the number of nested captures, or `None` if there are more than `u16::MAX` captures.
	fn initialize_captures(
		&mut self,
		next_id: &mut u16,
		captures: &mut Vec<CaptureInfo>,
		ancestors: &mut Vec<(u16, Arc<str>)>,
	) -> Option<u16> {
		let mut total: u16 = 0;
		match self {
			Self::AnyChar | Self::Literal(..) | Self::BracketedRanges { .. } => (),
			Self::Capture(capture) => {
				let id: u16 = *next_id;
				capture.id = id;
				*next_id = next_id.checked_add(1)?;

				let maybe_parent: Option<&(u16, Arc<str>)> = ancestors.last();
				let parent_id: u16 = maybe_parent.map_or(0, |(id, _)| *id);
				// The root capture is qualified by the empty string, so that the fully-qualified
				// name of a (sub-)rule is `root_name + qualified_name`.
				let qualified_name: Arc<str> = match maybe_parent {
					Some((_, parent_name)) => Arc::from(format!("{parent_name}.{}", capture.name)),
					None => Arc::<str>::default(),
				};

				ancestors.push((id, qualified_name.clone()));
				let index: usize = captures.len();
				captures.push(CaptureInfo {
					name: capture.name.clone(),
					id,
					parent_id,
					descendants: 0,
					qualified_name,
				});

				let descendants: u16 = capture
					.item
					.initialize_captures(next_id, captures, ancestors)?;
				captures[index].descendants = descendants;
				ancestors.pop();

				total = descendants.checked_add(1)?;
			},
			Self::KleeneClosure(item)
			| Self::KleenePlus(item)
			| Self::BoundedRepetition { item, .. }
			| Self::Placeholder { item, .. } => {
				total = item.initialize_captures(next_id, captures, ancestors)?;
			},
			Self::Sequence(items) | Self::Alternation(items) => {
				for sub_item in items.iter_mut() {
					total = total
						.checked_add(sub_item.initialize_captures(next_id, captures, ancestors)?)?;
				}
			},
		}
		Some(total)
	}

	/// Returns `Ok(self)` if the pattern does not accept the empty string,
	/// and a [`RegexError`] with [`RegexErrorKind::NullableExpression`]
	/// with minimal nullable (sub)expression per [`Regex::is_nullable`] otherwise.
	fn ensure_not_nullable(self, pattern: &str) -> Result<Self, RegexError> {
		if let Some(expr) = self.is_nullable() {
			Err(RegexError {
				consumed: pattern.to_owned(),
				remaining: String::new(),
				kind: RegexErrorKind::NullableExpression(Box::new(expr.clone())),
			})
		} else {
			Ok(self)
		}
	}
}

impl From<std::convert::Infallible> for RegexError {
	fn from(infallible: std::convert::Infallible) -> Self {
		infallible.into()
	}
}

impl RegexErrorKind {
	/// Helper to create a [`NomErr<_>::Error`] out of this error kind.
	fn error(self, input: &str) -> NomErr<RegexParsingError<'_>> {
		NomErr::Error(RegexParsingError::new(input, self))
	}

	/// Helper to create a [`NomErr<_>::Failure`] out of this error kind.
	fn fail(self, input: &str) -> NomErr<RegexParsingError<'_>> {
		NomErr::Failure(RegexParsingError::new(input, self))
	}

	/// Helper to create a "parser" that simply errors with this error kind.
	/// Used in [`nom::branch::alt`] fallbacks to generate more intuitive error messages.
	fn diagnostic<'a, T>(self) -> impl Fn(&'a str) -> ParsingResult<'a, T> {
		move |input| Err(self.clone().error(input))
	}
}

/// Same as [`parse_to_end`], but possibly with an ending `$` anchor.
/// Unlike the leading `^` anchor, it cannot be trivially stripped,
/// since the pattern is parsed left to right
/// (we don't know if a trailing `$` has been escaped or not without processing left to right).
fn parse_to_end_possibly_anchor(input: &str) -> ParsingResult<'_, (Regex, bool)> {
	let (input, regex): (&str, Regex) = parse_alternation(input)?;

	if input == "$" {
		Ok(("", (regex, true)))
	} else if input.is_empty() {
		Ok(("", (regex, false)))
	} else {
		Err(RegexErrorKind::InvalidTerm.error(input))
	}
}

/// See comment on [`parse_alternation`] and [`parse_sequence`] on swallowed errors;
/// reproduce [`RegexErrorKind::InvalidTerm`] errors if not at end of input.
fn parse_to_end(input: &str) -> ParsingResult<'_, Regex> {
	let (input, regex): (&str, Regex) = parse_alternation(input)?;

	if !input.is_empty() {
		return Err(RegexErrorKind::InvalidTerm.error(input));
	}

	Ok((input, regex))
}

/// A non-empty "list" of [`parse_sequence`]s, separated by bar `|`s.
/// Note that the first sequence is required (by non-emptiness),
/// and if a bar `|` is encountered, another sequence is necessarily expected.
///
/// `parse_alternation` is called at the top level by [`parse_to_end`],
/// and inside parentheses by [`parse_parenthesized`].
/// The former (re)produces [`RegexErrorKind::InvalidTerm`] swallowed inside [`parse_sequence`]
/// if not at end of input,
/// and the latter will produce a [`RegexErrorKind::ExpectedClose`] if not terminated properly.
fn parse_alternation(mut input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;
	use nom::combinator::opt;

	let mut items: Vec<Regex> = Vec::new();

	loop {
		// Cut: Any time we're "trying" to parse an alternation,
		// we necessarily are expecting at least one item;
		// and, the loop only continues if we parsed a bar '|',
		// so we're looking for another item.
		let sequence: Regex;
		(input, sequence) = cut(parse_sequence).parse(input)?;

		items.push(sequence);

		let maybe_bar: Option<char>;
		(input, maybe_bar) = opt(parse_char::<'|'>).parse(input)?;

		if maybe_bar.is_none() {
			break;
		}
	}

	if items.len() == 1 {
		Ok((input, items.pop().unwrap()))
	} else {
		Ok((input, Regex::Alternation(items)))
	}
}

/// A non-empty "list" of [`parse_suffixed`]s;
/// unlike [`parse_alternation`], there's no explicit separator/joining character,
/// so beyond the first `parse_suffixed`,
/// the list is terminated once `parse_suffixed` fails,
/// and the error is swallowed.
///
/// Note that we can't "peek" to check if we're at the end of input,
/// since we don't know if we're inside a parenthesized expression
/// (followed by a closing parentheses),
/// or at the top level (followed by end of input).
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
			Err(NomErr::Incomplete(_)) => {
				unreachable!("we shouldn't be using anything that can return this");
			},
			Err(err @ NomErr::Failure(_)) => {
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
		if (x > y) || (y == 0) {
			// For a suffix `{min,max}`, `min == max` is allowed, except if `max == 0`.
			return Err(RegexErrorKind::InvalidRepetitionBound(x, y).error(input_after_comma));
		}
		Ok((input, (x, y)))
	} else {
		if x == 0 {
			// Suffix `{0}` is equivalent to `{0,0}`, which is not allowed as above.
			return Err(RegexErrorKind::InvalidRepetitionBound(x, x).error(original_input));
		}
		Ok((input, (x, x)))
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

/// See also: [`parse_alternation`] for details on error propagation.
fn parse_parenthesized(input: &str) -> ParsingResult<'_, Regex> {
	use nom::branch::alt;

	surrounded_cut::<'(', ')', _, _>(alt((parse_capture, parse_alternation))).parse(input)
}

fn parse_capture(input: &str) -> ParsingResult<'_, Regex> {
	use nom::combinator::cut;

	let (input, _): (&str, char) = parse_char::<'?'>(input)?;

	// Cut: After seeing a '?', we necessarily are expecting a capture.
	let (input, name): (&str, &str) =
		cut(surrounded_cut::<'<', '>', _, _>(parse_capture_name)).parse(input)?;
	let name: Arc<str> = Arc::from(name);

	if input.starts_with(')') {
		// This function is called from [`parse_parenthesized`] inside a [`surrounded_cut`];
		// we do not consume the opening or closing parentheses here,
		// as those are consumed by [`surrounded_cut`].
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
				name,
				item: Box::new(Regex::NIL),
			},
		))
	} else {
		let (input, regex): (&str, Regex) = parse_alternation(input)?;

		Ok((input, Regex::Capture(Capture::new(name, Box::new(regex)))))
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

	let (input, maybe_start): (&str, Option<Term>) =
		parse_literal_char_in_bracketed_expression(original_input)?;

	let Some(start): Option<Term> = maybe_start else {
		return Ok((input, Vec::new()));
	};

	let (input_after_dash, maybe_dash): (&str, Option<char>) =
		opt(parse_char::<'-'>).parse(input)?;

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
							return Err(RegexErrorKind::InvalidBracketRange(start, end)
								.fail(input_after_dash));
						}
						Ok((input, vec![(start, end)]))
					},
					Term::Bracketed { .. } => {
						Err(RegexErrorKind::EscapeClassInBracketRange.fail(input_after_dash))
					},
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

	let (input, _): (&str, char) =
		NomUtils::parse_char::<'\\', RegexParsingError<'_>>(original_input)?;

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
		parse_one_char_of::<true>(SPECIAL_CHARACTERS_IN_BRACKETED_RANGES)
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
	// it's not a valid escape character,
	// and will be caught in the default branch of the `match` block below.
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
						unreachable!("unexpected escape character '\\{ch}'");
					},
				},
			},
		));
	}
	match Escaped::unescape_on_control_character(input) {
		Ok((input, ch)) => Ok((input, Term::Char(ch))),
		Err(InvalidEscape::Eof) => Err(RegexErrorKind::InvalidEscape.error(input)),
		Err(InvalidEscape::MalformedCodePoint) => {
			Err(RegexErrorKind::ExpectedHexDigits.fail(input))
		},
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

/// Helper version of [`NomUtils::parse_char`] with monomorphized error type.
fn parse_char<const CHAR: char>(input: &str) -> ParsingResult<'_, char> {
	NomUtils::parse_char::<CHAR, RegexParsingError<'_>>(input)
}

/// Helper version of [`NomUtils::surrounded_cut`] with monomorphized error type.
fn surrounded_cut<'a, const OPEN: char, const CLOSE: char, O, F>(
	inside: F,
) -> impl Parser<&'a str, Output = O, Error = RegexParsingError<'a>>
where
	F: Parser<&'a str, Output = O, Error = RegexParsingError<'a>>,
{
	NomUtils::surrounded_cut::<OPEN, CLOSE, _, _, _, _>(
		inside,
		RegexErrorKind::ExpectedClose(OPEN, CLOSE).diagnostic(),
	)
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
				RegexErrorKind::NullableExpression(Box::new(Regex::KleeneClosure(Box::new(
					Regex::Literal('b')
				),)))
			);
		}
		{
			let e: RegexError = Regex::from_pattern(r"(a|b*)+").unwrap_err();
			assert_eq!(
				e.kind,
				RegexErrorKind::NullableExpression(Box::new(Regex::KleeneClosure(Box::new(
					Regex::Literal('b')
				),)))
			);
		}
	}

	#[test]
	fn to_pattern_round_trips() {
		const PATTERNS: &[&str] = &[
			// Literal caret in brackets.
			r"[\^a]",
			r"[\^^]",
			r"[a^]",
			r"[^^]",
			r"[^\^a]",
			r"[\^-a]",
			// Nested repetitions.
			r"(a+)+",
			r"(a+)?b",
			r"(a*)+b",
			r"(a?)*b",
			r"(a{2}){3}",
			r"(a{2,3})+",
			r"((a+)+)+",
			r"(?<x>a+)+",
			// Non-ASCII characters with an odd number of minimal hex digits.
			"\u{434}+",
			"[\u{434}-\u{44f}]",
			"\u{1f600}",
			"[\u{1}-\u{1f600}]",
			r"\u{00}",
		];

		for &pattern in PATTERNS.iter() {
			let regex: Regex = Regex::from_pattern(pattern).unwrap();
			let serialized: String = regex.to_pattern();
			let reparsed: Regex = Regex::from_pattern(&serialized)
				.unwrap_or_else(|e| panic!("{pattern:?} -> {serialized:?}: {e:?}"));
			assert_eq!(regex, reparsed, "{pattern:?} -> {serialized:?}");
		}
	}

	#[test]
	fn literal_caret_in_brackets_stays_literal() {
		let regex: Regex = Regex::from_pattern(r"[\^a]").unwrap();
		assert_eq!(regex.to_pattern(), r"[\^a]");
		std::assert_matches!(regex, Regex::BracketedRanges { negated: false, .. });
	}

	#[test]
	fn nested_repetitions_are_parenthesized() {
		const CASES: &[(&str, &str)] = &[
			("(a+)+", "(a+)+"),
			("(a+)?b", "(a+)?b"),
			("(a{2}){3}", "(a{2}){3}"),
			("(ab)+", "(ab)+"),
			("a+b", "a+b"),
		];
		for &(pattern, expected) in CASES.iter() {
			assert_eq!(Regex::from_pattern(pattern).unwrap().to_pattern(), expected);
		}
	}

	#[test]
	fn standard_escape_classes() {
		const CLASSES: &[(&str, &[(char, char)])] = &[
			(r"\d", &[('0', '9')]),
			(r"\w", &[('0', '9'), ('a', 'z'), ('A', 'Z')]),
			(
				r"\s",
				&[(' ', ' '), ('\t', '\t'), ('\r', '\r'), ('\n', '\n')],
			),
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
