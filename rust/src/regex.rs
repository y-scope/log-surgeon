mod pattern_parsing;

use std::num::NonZero;
use std::sync::Arc;

pub use pattern_parsing::RegexError;
pub use pattern_parsing::RegexPlaceholderLookup;

use crate::parsing_spec::SubRule;
use crate::utils::Escaped;
use crate::utils::LocalTryInto;

const SPECIAL_CHARACTERS: &str = r"\()[]{}*+?.|^$";

const SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS: &str = r"\[]";

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct AnchoredRegex {
	pub anchor_before: bool,
	pub anchor_after: bool,
	pub regex: Regex,
	/// Total "captures" in the regex - total [`Regex::Capture`] **plus 1** for the implicit full capture behaviour.
	pub total_captures: NonZero<u16>,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum Regex {
	AnyChar,
	Literal(char),
	Capture(Arc<SubRule>),
	BracketedRanges {
		negated: bool,
		items: Vec<(char, char)>,
	},
	KleeneClosure(Box<Regex>),
	/// In terms of matching (e.g. when building the NFA),
	/// this variant is equivalent to ("desugars as") a sequence of
	/// the inner item and the Kleene closure of the item
	/// (see [`Regex::wrap_as_desugared_kleene_plus`]).
	///
	/// However, when parsing patterns, we store `(item)+` explicitly so we can (re)serialize it
	/// without the "desguaring".
	KleenePlus(Box<Regex>),
	BoundedRepetition {
		min: u32,
		max: u32,
		item: Box<Regex>,
	},
	Sequence(Vec<Regex>),
	Alternation(Vec<Regex>),
	/// No effect on string matching;
	/// this variant is for serializing a [`crate::parsing_spec::ParsingSpec`].
	Placeholder {
		name: String,
		item: Box<Regex>,
	},
}

impl std::fmt::Debug for Regex {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.to_pattern().escape_default(), fmt)
	}
}

impl std::fmt::Display for Regex {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.to_pattern().escape_default().fmt(fmt)
	}
}

impl LocalTryInto<AnchoredRegex> for &str {
	type Error = RegexError;

	fn try_into(self) -> Result<AnchoredRegex, Self::Error> {
		Regex::from_pattern(self)
	}
}

impl AnchoredRegex {
	pub fn to_pattern(&self) -> String {
		let pattern: String = self.regex.to_pattern();
		let anchor_before: &str = if self.anchor_before { "^" } else { "" };
		let anchor_after: &str = if self.anchor_after { "$" } else { "" };

		format!("{anchor_before}{pattern}{anchor_after}")
	}
}

impl Regex {
	/// An "invalid" `Regex` value; conceptually (and literally) it matches no strings,
	/// so it's not meaningful in an actual parsing spec.
	///
	/// Currently, only used during pattern parsing for [`Regex::Placeholder`],
	/// where the actual placeholder value will be filled in later.
	pub const NIL: Self = Self::Alternation(Vec::new());

	/// String representation/"to pattern" conversion of this regex,
	/// replacing any leading or trailing space literal with `[ ]` for explicitness.
	pub fn to_pattern(&self) -> String {
		let mut pattern: String = self.to_pattern_internal();

		if let Some(suffix) = pattern.strip_prefix(' ') {
			pattern = format!("[ ]{suffix}");
		}
		if let Some(prefix) = pattern.strip_suffix(' ') {
			pattern = format!("{prefix}[ ]");
		}

		// Other whitespace should always be escaped.
		assert!(!pattern.starts_with(|ch: char| ch.is_whitespace()));
		assert!(!pattern.ends_with(|ch: char| ch.is_whitespace()));

		pattern
	}

	/// Direct string representation/"to pattern" conversion of this regex.
	fn to_pattern_internal(&self) -> String {
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
				let serialized: String = items.iter().fold(String::new(), |mut accumulated, &(lo, hi)| {
					escape(lo, &mut accumulated);
					if lo != hi {
						accumulated.push('-');
						escape(hi, &mut accumulated);
					}
					accumulated
				});
				format!("[{negation}{serialized}]")
			},
			Self::Capture(sub_rule) => {
				format!("(?<{}>{})", sub_rule.name, sub_rule.regex.to_pattern_internal())
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
				let item_pattern: String = self.surround(item);
				if (*min, *max) == (0, 1) {
					format!("{item_pattern}?")
				} else if min == max {
					format!("{item_pattern}{{{min}}}")
				} else {
					format!("{item_pattern}{{{min},{max}}}")
				}
			},
			Self::Sequence(items) => items.iter().fold(String::new(), |mut accumulated, item| {
				accumulated.push_str(&self.surround(item));
				accumulated
			}),
			Self::Alternation(items) => {
				// There should be at least one alternative.
				let first: &Self = items.first().unwrap();
				items[1..]
					.iter()
					.fold(first.to_pattern_internal(), |mut accumulated, item| {
						accumulated.push('|');
						accumulated.push_str(&self.surround(item));
						accumulated
					})
			},
		}
	}

	/// Parenthesizes a subexpression if necessary; see [`Regex::precedence`].
	fn surround(&self, item: &Self) -> String {
		let sub_pattern: String = item.to_pattern_internal();
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
	/// "Desugars" a pattern `(self)+` as `(self)(self)*`;
	/// see [`Regex::KleenePlus`] for more details.
	pub fn wrap_as_desugared_kleene_plus(&self) -> Self {
		Self::Sequence(vec![self.clone(), Self::KleeneClosure(Box::new(self.clone()))])
	}

	/// Whether this regex accepts an empty string;
	/// return the first (minimal) child that does (for diagnostics).
	///
	/// An empty sequence should be not-parsable,
	/// but repetition suffixes allow for empty matches,
	/// e.g. `a*` or `a{0,3}`.
	fn is_nullable(&self) -> Option<&Self> {
		match self {
			Self::AnyChar | Self::Literal(..) | Self::BracketedRanges { .. } => None,
			Self::Capture(sub_rule) => sub_rule.regex.is_nullable(),
			Self::KleeneClosure(item) => Some(item.is_nullable().unwrap_or(self)),
			Self::KleenePlus(item) => item.is_nullable(),
			Self::BoundedRepetition { min, item, .. } => {
				if *min > 0 {
					item.is_nullable()
				} else {
					Some(item.is_nullable().unwrap_or(self))
				}
			},
			Self::Placeholder { item, .. } => item.is_nullable(),
			Self::Sequence(items) => {
				if items.iter().all(|item| item.is_nullable().is_some()) {
					Some(self)
				} else {
					None
				}
			},
			Self::Alternation(items) => {
				for item in items.iter() {
					if let Some(child) = item.is_nullable() {
						return Some(child);
					}
				}
				None
			},
		}
	}
}
