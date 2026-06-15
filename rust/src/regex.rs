mod pattern_parsing;

use std::num::NonZero;
use std::sync::Arc;

pub use pattern_parsing::RegexError;
pub use pattern_parsing::RegexPlaceholderLookup;

use crate::parsing_spec::SubRule;
use crate::utils::Escaped;

// TODO: relax need to escape `<>`?
const SPECIAL_CHARACTERS: &str = r"\()[]{}*+?.|^$<>";

const SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS: &str = r"\[]";

/// Morally, this is just `TryInto<AnchoredRegex>`,
/// since we can't have `impl<'a> TryFrom<&'a str> for Result<AnchoredRegex, RegexError<'a>>`,
/// because of Rust's forsaken orphan rules.
pub trait IntoRegex {
	type Error;

	fn into(self) -> Result<AnchoredRegex, Self::Error>;
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct AnchoredRegex {
	pub anchor_before: bool,
	pub anchor_after: bool,
	pub inner: Regex,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum Regex {
	AnyChar,
	Literal(char),
	Capture(Box<SubRule>),
	BracketedRanges { negated: bool, items: Vec<(char, char)> },
	KleeneClosure(Box<Regex>),
	KleenePlus(Box<Regex>),
	BoundedRepetition { min: u32, max: u32, item: Box<Regex> },
	Sequence(Vec<Regex>),
	Alternation(Vec<Regex>),
	Placeholder { name: String, item: Box<Regex> },
}

impl std::fmt::Debug for Regex {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(&self.to_pattern())
	}
}

impl IntoRegex for AnchoredRegex {
	type Error = std::convert::Infallible;

	fn into(self) -> Result<AnchoredRegex, Self::Error> {
		Ok(self)
	}
}

impl IntoRegex for &str {
	type Error = RegexError;

	fn into(self) -> Result<AnchoredRegex, Self::Error> {
		Regex::from_pattern(self)
	}
}

impl AnchoredRegex {
	pub fn unanchored(inner: Regex) -> Self {
		Self {
			anchor_before: false,
			anchor_after: false,
			inner,
		}
	}

	pub fn to_pattern(&self) -> String {
		let mut pattern: String = self.inner.to_pattern();

		// We do this replacement before the anchors for consistency.
		if let Some(suffix) = pattern.strip_prefix(' ') {
			pattern = format!("[ ]{suffix}");
		}
		if let Some(prefix) = pattern.strip_suffix(' ') {
			pattern = format!("{prefix}[ ]");
		}

		let anchor_before: &str = if self.anchor_before { "^" } else { "" };
		let anchor_after: &str = if self.anchor_after { "$" } else { "" };

		let pattern: String = format!("{anchor_before}{pattern}{anchor_after}");

		assert!(!pattern.starts_with(|ch: char| ch.is_whitespace()));
		assert!(!pattern.ends_with(|ch: char| ch.is_whitespace()));

		pattern
	}
}

impl Regex {
	pub fn to_pattern(&self) -> String {
		match self {
			Self::AnyChar => ".".to_owned(),
			&Self::Literal(ch) => {
				if SPECIAL_CHARACTERS.contains(ch) {
					format!("\\{ch}")
				} else {
					Escaped::escape(ch).escape_space(false).to_string()
				}
			},
			Self::BracketedRanges { negated, items } => {
				fn escape(ch: char, buffer: &mut String) {
					if SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS.contains(ch) {
						buffer.push('\\');
						buffer.push(ch);
					} else if ch == '-' {
						// This is needed, for example, for `[a\-z]` as 3 characters, but not `[a-]`.
						// However, we always escape it for simplicity and clarity.
						buffer.push_str("\\-");
					} else {
						buffer.push_str(&Escaped::escape(ch).escape_space(false).to_string());
					}
				}

				let negation: &str = if *negated { "^" } else { "" };
				let items: String = items.iter().fold(String::new(), |mut accumulated, &(lo, hi)| {
					escape(lo, &mut accumulated);
					if lo != hi {
						accumulated.push('-');
						escape(hi, &mut accumulated);
					}
					accumulated
				});
				format!("[{negation}{items}]")
			},
			Self::Capture(sub_rule) => {
				format!("(?<{}>{})", sub_rule.name, sub_rule.regex.to_pattern())
			},
			Self::Placeholder { name, .. } => {
				format!("(?<{}>)", name)
			},
			Self::KleeneClosure(item) => {
				format!("{}*", self.surround(item))
			},
			Self::KleenePlus(item) => {
				format!("{}+", self.surround(item))
			},
			Self::BoundedRepetition { min, max, item } => {
				let sub_pattern: String = self.surround(item);
				if (*min, *max) == (0, 1) {
					format!("{sub_pattern}?")
				} else if min == max {
					format!("{sub_pattern}{{{min}}}")
				} else {
					format!("{sub_pattern}{{{min},{max}}}")
				}
			},
			Self::Sequence(items) => items.iter().fold(String::new(), |mut accumulated, item| {
				accumulated.push_str(&self.surround(item));
				accumulated
			}),
			Self::Alternation(items) => {
				// There should be at least one alternative.
				let first: &Self = items.first().unwrap();
				// Alternation is already the lowest precedence (and associative),
				// so no need to parenthesize subexpressions.
				let mut buffer: String = first.to_pattern();
				for item in items[1..].iter() {
					buffer.push('|');
					buffer.push_str(&item.to_pattern());
				}
				buffer
			},
		}
	}

	/// Parenthesizes a subexpression if necessary; see [`Regex::precedence`].
	fn surround(&self, item: &Self) -> String {
		let sub_pattern: String = item.to_pattern();
		if item.precedence() < self.precedence() {
			format!("({sub_pattern})")
		} else {
			sub_pattern
		}
	}

	/// Lower value is lower precedence;
	/// if an expression contains a subexpression with strictly lower precedence,
	/// the subexpression must be parenthesized,
	/// except for a capture, which is "already" parenthesized.
	fn precedence(&self) -> isize {
		match self {
			Self::AnyChar | Self::Literal(_) | Self::BracketedRanges { .. } => 0,
			Self::Capture { .. } | Self::Placeholder { .. } => 0,
			Self::KleeneClosure(_) | Self::KleenePlus(_) | Self::BoundedRepetition { .. } => -1,
			Self::Sequence(_) => -2,
			Self::Alternation(_) => -3,
		}
	}
}

impl Regex {
	pub fn count_captures(&self) -> usize {
		match self {
			Self::AnyChar | Self::Literal(..) | Self::BracketedRanges { .. } => 0,
			Self::Capture(sub_rule) => 1 + sub_rule.descendents,
			Self::KleeneClosure(item)
			| Self::KleenePlus(item)
			| Self::BoundedRepetition { item, .. }
			| Self::Placeholder { item, .. } => item.count_captures(),
			Self::Sequence(items) | Self::Alternation(items) => {
				items.iter().fold(0, |total, item| total + item.count_captures())
			},
		}
	}
}

impl Regex {
	/// "Desugars" a pattern `(self)+` as `(self)(self)*`.
	pub fn into_kleene_plus(&self) -> Self {
		Self::Sequence(vec![self.clone(), Regex::KleeneClosure(Box::new(self.clone()))])
	}
}

impl SubRule {
	pub fn id_as_usize(&self) -> usize {
		usize::from(self.id.get())
	}

	pub fn is_leaf(&self) -> bool {
		self.descendents == 0
	}
}
