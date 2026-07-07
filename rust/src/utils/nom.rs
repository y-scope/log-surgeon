use nom::Err as NomErr;
use nom::IResult;
use nom::Parser;
use nom::error::ParseError;

pub struct NomUtils;

impl NomUtils {
	/// Parse and return a single `CHAR`,
	/// returning a [`NomErr::Error`] of `<E as ParseError>::from_char` otherwise.
	pub fn parse_char<'a, const CHAR: char, E>(input: &'a str) -> IResult<&'a str, char, E>
	where
		E: ParseError<&'a str>,
	{
		use std::str::Chars;

		let mut chars: Chars<'_> = input.chars();

		if let Some(ch) = chars.next() {
			if ch == CHAR {
				return Ok((chars.as_str(), ch));
			} else {
				return Err(NomErr::Error(E::from_char(input, CHAR)));
			}
		}

		Err(NomErr::Error(E::from_char(input, CHAR)))
	}

	/// Parse `inside` between `OPEN` and `CLOSE` characters;
	/// "cut" (commit to this parse) after seeing the opening character;
	/// i.e. transform [`NomErr::Error`] to [`NomErr::Failure`].
	///
	/// See also:
	/// - [`nom::combinator::cut`].
	pub fn surrounded_cut<'a, const OPEN: char, const CLOSE: char, O, F, G, E>(
		inside: F,
		expected_close: G,
	) -> impl Parser<&'a str, Output = O, Error = E>
	where
		F: Parser<&'a str, Output = O, Error = E>,
		G: Parser<&'a str, Output = char, Error = E>,
		E: ParseError<&'a str>,
	{
		use nom::combinator::cut;

		// Since `cut` takes its argument by value,
		// `cut`ting the parsers inside the closure makes it `FnOnce`
		// (it can only be moved once inside the closure,
		// where it can only be moved once to `cut`).
		//
		// Constructing the `cut` parsers outside,
		// the closure is `FnMut` is `Parser::parse` takes `&mut self`.
		//
		// `cut` returns `impl Parser`, so we can't annotate with an explicit type.
		let mut cut_inside: _ = cut(inside);
		let mut cut_close: _ = cut(NomUtils::parse_char::<CLOSE, E>.or(expected_close));

		move |input| {
			let (input, _): (&str, char) = NomUtils::parse_char::<OPEN, E>(input)?;

			// At this point, we've seen/parsed the opening character.
			// "Cut" (require) `inside` to parse necessarily,
			// as well as the closing character.
			let (input, output): (&str, O) = cut_inside.parse(input)?;
			let (input, _): (&str, char) = cut_close.parse(input)?;

			Ok((input, output))
		}
	}
}
