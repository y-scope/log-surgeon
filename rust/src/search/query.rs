//! The query language: [`SearchString`] and its symbols.
//!
//! A query is literal characters and `*`, with `\` escaping either. It is parsed once into
//! canonical [`SymbolicChar`]s (no adjacent wildcards), from which anchoring is read off.

use crate::regex::Regex;

#[derive(Debug)]
pub struct SearchString {
	pub(super) symbols: Vec<SymbolicChar>,
}

impl std::fmt::Display for SearchString {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for ch in self.symbols.iter() {
			ch.fmt(fmt)?;
		}
		Ok(())
	}
}

#[derive(Debug)]
pub enum SearchStringError<'input> {
	InvalidEscape {
		before: &'input str,
		after: &'input str,
	},
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum SymbolicChar {
	Literal(char),
	GlobStar,
}

impl std::fmt::Debug for SymbolicChar {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self, fmt)
	}
}

impl std::fmt::Display for SymbolicChar {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Literal(ch) => {
				if matches!(ch, '*' | '\\') {
					fmt.write_str("\\")?;
				}
				ch.fmt(fmt)
			},
			Self::GlobStar => fmt.write_str("*"),
		}
	}
}

impl SearchString {
	/// Parses a query.
	///
	/// `*` and `\` are the only metacharacters; `\*` and `\\` write them literally.
	///
	/// The result is canonical: adjacent wildcards are collapsed, since `**` says exactly what
	/// `*` does. Everything downstream relies on this -- anchoring is read off the first and
	/// last symbol, and a wildcard is a symbol like any other that the automata consume --
	/// so it is established once, here, rather than at every entry point.
	pub fn parse(input: &str) -> Result<Self, SearchStringError<'_>> {
		let mut symbols: Vec<SymbolicChar> = Vec::new();
		let mut last_was_escape: bool = false;
		for (i, ch) in input.char_indices() {
			match (last_was_escape, ch) {
				(true, '*' | '\\') => {
					symbols.push(SymbolicChar::Literal(ch));
					last_was_escape = false;
				},
				(true, _) => {
					let (before, after): (&str, &str) = input.split_at(i);
					return Err(SearchStringError::InvalidEscape { before, after });
				},
				(false, '\\') => last_was_escape = true,
				(false, '*') => {
					if !symbols.last().is_some_and(SymbolicChar::is_wildcard) {
						symbols.push(SymbolicChar::GlobStar);
					}
				},
				(false, _) => symbols.push(SymbolicChar::Literal(ch)),
			}
		}
		if last_was_escape {
			return Err(SearchStringError::InvalidEscape {
				before: input,
				after: "",
			});
		}
		Ok(Self { symbols })
	}

	pub fn as_slice(&self) -> &[SymbolicChar] {
		self.symbols.as_slice()
	}

	/// Whether a match must begin at the start of the message.
	///
	/// Anchoring is one uniform rule, read off both ends the same way:
	/// a query is anchored at a boundary exactly when it does not have a wildcard there.
	/// So `foo*` is anchored at the start, `*foo` at the end,
	/// `foo` at both (an exact match), and `*foo*` at neither.
	/// The empty query is anchored at both, and matches only an empty message.
	///
	/// Nothing is derived from this beyond the predicate itself:
	/// a wildcard is a symbol the automata consume like any other,
	/// so every match runs from the start of the message to its end,
	/// and "unanchored" just means a `*` is there to absorb the slack.
	#[must_use]
	pub fn anchored_start(&self) -> bool {
		!self.symbols.first().is_some_and(SymbolicChar::is_wildcard)
	}

	/// Whether a match must run through to the end of the message.
	///
	/// See [`Self::anchored_start`].
	#[must_use]
	pub fn anchored_end(&self) -> bool {
		!self.symbols.last().is_some_and(SymbolicChar::is_wildcard)
	}
}

impl SymbolicChar {
	pub fn is_wildcard(&self) -> bool {
		*self == Self::GlobStar
	}

	pub(super) fn to_regex(&self) -> Regex {
		match *self {
			SymbolicChar::Literal(ch) => Regex::Literal(ch),
			SymbolicChar::GlobStar => Regex::KleeneClosure(Box::new(Regex::AnyChar)),
		}
	}
}
