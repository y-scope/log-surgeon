mod pattern_parsing;
pub use pattern_parsing::*;

use std::num::NonZero;

use crate::schema::RuleInfo;
use crate::utils::Escaped;

// TODO: relax need to escape `<>`?
const SPECIAL_CHARACTERS: &str = r"\()[]{}*+?.|^$<>";

const SPECIAL_CHARACTERS_IN_BRACKETED_EXPRESSIONS: &str = r"\[]";

/// This is morally just `TryInto<AnchoredRegex>`
/// since we can't have `impl<'a> From<&'a str> for Result<AnchoredRegex, RegexError<'a>>`,
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
	Capture(SubRule),
	Group { negated: bool, items: Vec<(char, char)> },
	KleeneClosure(Box<Regex>),
	KleenePlus(Box<Regex>),
	BoundedRepetition { min: u32, max: u32, item: Box<Regex> },
	Sequence(Vec<Regex>),
	Alternation(Vec<Regex>),
}

impl std::fmt::Debug for Regex {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(&self.to_pattern())
	}
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SubRule {
	pub name: String,
	pub regex: Box<Regex>,

	/// ID statically assigned left-to-right based on the regex pattern.
	/// For example, the pattern `(?<start>[a-z]+(?<rest>\.[a-z]+)*)|(?<start>[0-9]+)` has three non-zero capture IDs.
	/// When the pattern is actually matched,
	/// there may be multiple instances of capture ID 2 (corresponding to `"rest"`).
	/// The capture ID also differentiates between different capture groups given the same name,
	/// e.g. the two instances of `"start"` in the pattern.
	pub id: NonZero<u16>,
	/// ID of the parent capture, if any.
	pub parent_id: Option<NonZero<u16>>,
	/// Total number of nested captures (recursively/arbitrarily deep);
	/// `0` iff this is a "leaf" capture.
	pub descendents: usize,

	/// Qualified name w.r.t captures including the leading dot;
	/// a top-level capture is ".a", a second-level capture is ".a.b".
	pub qualified_name: String,
}

impl IntoRegex for AnchoredRegex {
	type Error = std::convert::Infallible;

	fn into(self) -> Result<AnchoredRegex, Self::Error> {
		Ok(self)
	}
}

impl<'a> IntoRegex for &'a str {
	type Error = RegexError<'a>;

	fn into(self) -> Result<AnchoredRegex, Self::Error> {
		Regex::from_pattern(self)
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
					Escaped::escape(ch).to_string()
				}
			},
			Self::Group { negated, items } => {
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
			Self::AnyChar | Self::Literal(_) | Self::Group { .. } | Self::Capture { .. } => 0,
			Self::KleeneClosure(_) | Self::KleenePlus(_) | Self::BoundedRepetition { .. } => -1,
			Self::Sequence(_) => -2,
			Self::Alternation(_) => -3,
		}
	}
}

impl Regex {
	pub fn count_captures(&self) -> usize {
		match self {
			Self::AnyChar | Self::Literal(..) | Self::Group { .. } => 0,
			Self::Capture(sub_rule) => 1 + sub_rule.descendents,
			Self::KleeneClosure(item) | Self::KleenePlus(item) | Self::BoundedRepetition { item, .. } => {
				item.count_captures()
			},
			Self::Sequence(items) | Self::Alternation(items) => {
				items.iter().fold(0, |total, item| total + item.count_captures())
			},
		}
	}

	pub fn populate_sub_rule_info(&self, capture_info: &mut Vec<RuleInfo>) {
		match self {
			Self::AnyChar | Self::Literal(..) | Self::Group { .. } => (),
			Self::Capture(sub_rule) => {
				let i: usize = sub_rule.id_as_usize();
				assert_eq!(capture_info.len(), i);
				capture_info.push(RuleInfo::Sub(sub_rule.clone()));
				sub_rule.regex.populate_sub_rule_info(capture_info);
			},
			Self::KleeneClosure(item) | Self::KleenePlus(item) | Self::BoundedRepetition { item, .. } => {
				item.populate_sub_rule_info(capture_info);
			},
			Self::Sequence(items) | Self::Alternation(items) => {
				for sub_item in items.iter() {
					sub_item.populate_sub_rule_info(capture_info);
				}
			},
		}
	}

	/// [`RegexCapture::id`] defaults to [`NonZero::<u16>::MAX`];
	/// if we actually reach this, `next_id` will overflow,
	/// so it naturally works as a placeholder/invalid value.
	///
	/// Invariant: `parent_id < id`.
	fn number_captures(&mut self, id: &mut NonZero<u16>, stack: &mut Vec<(NonZero<u16>, String)>) -> Option<usize> {
		let mut bread: usize = 0;
		match self {
			Self::AnyChar | Self::Literal(..) | Self::Group { .. } => (),
			Self::Capture(sub_rule) => {
				let maybe_parent: Option<&(NonZero<u16>, String)> = stack.last();
				sub_rule.parent_id = maybe_parent.map(|(id, _)| *id);
				sub_rule.id = *id;
				sub_rule.qualified_name = format!("{}.{}", maybe_parent.map_or("", |(_, name)| name), sub_rule.name);
				stack.push((sub_rule.id, sub_rule.qualified_name.clone()));
				// `id` is `u16`.
				*id = id.checked_add(1)?;
				sub_rule.descendents = sub_rule.regex.number_captures(id, stack)?;
				// `bread` is `usize`.
				bread = 1 + sub_rule.descendents;
				stack.pop();
			},
			Self::KleeneClosure(item) | Self::KleenePlus(item) | Self::BoundedRepetition { item, .. } => {
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

impl Regex {
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
