#![allow(unused)]

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZero;

use crate::dfa::Tdfa;
use crate::dfa::TdfaExecution;
use crate::ffi::UncheckedCArray;
use crate::lexer::Lexer;
use crate::log_event::Capture;
use crate::log_event::CaptureFfiPointers;
use crate::regex::Regex;
use crate::regex::RegexCapture;
use crate::regex::TopLevelRegex;
use crate::schema::Rule;
use crate::schema::RuleIdx;
use crate::schema::Schema;
use crate::schema::VariableOrCaptures;
use crate::utils::Range;

#[derive(Debug)]
pub struct SearchString(Vec<SymbolicChar>);

#[derive(Debug)]
pub enum SearchStringError<'input> {
	InvalidEscape { before: &'input str, after: &'input str },
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum SymbolicChar {
	Literal(char),
	WildcardStar,
	WildcardOne,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Interpretation {
	pub sub_queries: Vec<SubQuery>,
	pub stringified: String,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SubQuery {
	pub group: usize,
	pub rule_idx: Option<NonZero<u16>>,
	pub name: String,
	pub value: String,
}

#[derive(Clone, Copy)]
struct SearchStringView<'a> {
	full_string: &'a SearchString,
	start: usize,
	end: usize,
}

impl std::fmt::Debug for SearchStringView<'_> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.debug_tuple("SearchStringView")
			.field(&self.as_str().iter().map(SymbolicChar::to_string).collect::<String>())
			.finish()
	}
}

impl SearchString {
	pub fn parse(input: &str) -> Result<Self, SearchStringError<'_>> {
		let mut chars: Vec<SymbolicChar> = Vec::new();
		let mut last_was_escape: bool = false;
		for (i, ch) in input.char_indices() {
			match ch {
				'*' | '?' | '\\' => {
					chars.push(if last_was_escape {
						SymbolicChar::Literal(ch)
					} else {
						match ch {
							'*' => SymbolicChar::WildcardStar,
							'?' => SymbolicChar::WildcardOne,
							'\\' => {
								last_was_escape = true;
								continue;
							},
							_ => {
								unreachable!();
							},
						}
					});
				},
				_ => {
					if last_was_escape {
						let (before, after): (&str, &str) = input.split_at(i);
						return Err(SearchStringError::InvalidEscape { before, after });
					} else {
						chars.push(SymbolicChar::Literal(ch));
					}
				},
			}
			last_was_escape = false;
		}
		if last_was_escape {
			return Err(SearchStringError::InvalidEscape {
				before: input,
				after: "",
			});
		}
		Ok(Self(chars))
	}

	pub fn as_slice(&self) -> &[SymbolicChar] {
		&self.0
	}

	pub fn interpretations_for_name(&self, schema: &Schema, name: &str) -> Vec<Interpretation> {
		let Some(rows): Option<VariableOrCaptures<Regex>> = schema.regexes_for_name(name) else {
			return Vec::new();
		};

		if !name.is_empty() {
			return self
				.view(0, self.0.len())
				.interpretations_for_name_internal(schema, &rows);
		}

		if self.0.is_empty() {
			return Vec::new();
		}

		let mut interpretations: Vec<Vec<Interpretation>> = vec![Vec::new(); self.0.len()];
		let mut group: usize = 0;

		for end in 1..=self.0.len() {
			for start in 0..end {
				let sub_view: SearchStringView<'_> = self.view(start, end);
				if (sub_view.as_str() != &[SymbolicChar::WildcardStar])
					&& ((sub_view.as_str().first() == Some(&SymbolicChar::WildcardStar))
						|| (sub_view.as_str().last() == Some(&SymbolicChar::WildcardStar)))
				{
					continue;
				}

				let single_token_interpretations: Vec<Interpretation> =
					sub_view.single_token_interpretations(schema, &rows);

				if single_token_interpretations.is_empty() {
					continue;
				}

				if start == 0 {
					for suffix in single_token_interpretations.into_iter() {
						interpretations[end - 1].push(suffix);
					}
				} else {
					// TODO explain clone
					for prefix in interpretations[start - 1].clone().iter() {
						for suffix in single_token_interpretations.iter() {
							let mut combined: Interpretation = prefix.clone();
							combined.append_sub_query(suffix.clone());
							interpretations[end - 1].push(combined);
						}
					}
				}

				println!("interpretations up to {end} are {:?}", interpretations[end - 1]);
			}
		}

		interpretations.pop().unwrap().into_iter().collect::<Vec<_>>()
	}

	fn iter(&self) -> impl Iterator<Item = SymbolicChar> {
		self.0.iter().copied()
	}

	fn view(&self, start: usize, end: usize) -> SearchStringView<'_> {
		SearchStringView {
			full_string: &self,
			start,
			end,
		}
	}
}

impl std::fmt::Display for SearchString {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for ch in self.0.iter() {
			ch.fmt(fmt)?;
		}
		Ok(())
	}
}

impl<'a> SearchStringView<'a> {
	fn single_token_interpretations(&self, schema: &Schema, rows: &VariableOrCaptures<Regex>) -> Vec<Interpretation> {
		assert!(!self.is_empty());

		let extended: Self = self.extend();

		let mut interpretations: Vec<Interpretation> = Vec::new();

		if self.as_str() == &[SymbolicChar::WildcardStar] {
			interpretations.push(Interpretation {
				sub_queries: vec![SubQuery::new_static_text(self.as_str())],
				stringified: String::new(),
			});
		}

		let has_wildcard: bool = self.as_str().iter().find(|ch| ch.is_wildcard()).is_some();

		let potential_interpretations: Vec<Interpretation> = extended.interpretations_for_name_internal(schema, rows);

		if has_wildcard || potential_interpretations.is_empty() {
			interpretations.push(Interpretation {
				sub_queries: vec![SubQuery::new_static_text(extended.as_str())],
				stringified: String::new(),
			});
		}

		interpretations.extend(potential_interpretations.into_iter());

		interpretations
	}

	fn extend(&self) -> Self {
		let mut new_start: usize = self.start;
		let mut new_end: usize = self.end;
		while new_start > 0 {
			if self.full_string.0[new_start] == SymbolicChar::WildcardStar {
				new_start -= 1;
			} else {
				break;
			}
		}
		while new_end < self.full_string.0.len() {
			if self.full_string.0[new_end] == SymbolicChar::WildcardStar {
				new_end += 1;
			} else {
				break;
			}
		}
		Self {
			full_string: self.full_string,
			start: new_start,
			end: new_end,
		}
	}

	fn as_str(&self) -> &[SymbolicChar] {
		&self.full_string.0[self.start..self.end]
	}

	fn interpretations_for_name_internal(
		&self,
		schema: &Schema,
		rows: &VariableOrCaptures<Regex>,
	) -> Vec<Interpretation> {
		let mut interpretations: Vec<Interpretation> = Vec::new();

		let rows = match rows {
			VariableOrCaptures::Variable(rows) => rows
				.iter()
				.map(|(rule_idx, regex)| (&schema[*rule_idx], regex))
				.collect::<Vec<_>>(),
			VariableOrCaptures::Captures(rows) => rows
				.into_iter()
				.map(|(rule_idx, _info, regex)| (&schema[*rule_idx], regex))
				.collect::<Vec<_>>(),
		};

		for (rule, regex) in rows.into_iter() {
			debug!("- simulating regex {regex:?}");
			for path in regex.simulate(self.as_str()) {
				let mut sub_queries: Vec<SubQuery> = Vec::new();

				if self[0].is_wildcard() {
					sub_queries.push(SubQuery {
						group: 0,
						rule_idx: None,
						name: String::new(),
						value: "*".to_owned(),
					});
				}

				let mut last_pos: usize = 0;

				for token in path.into_iter() {
					if let Some(capture) = &token.maybe_capture {
						sub_queries.push(SubQuery {
							group: 0,
							rule_idx: Some(rule.idx.index),
							name: format!("{}{}", rule.name, capture.qualified_name),
							value: token
								.value
								.iter()
								.map(SymbolicChar::escape_for_search_string)
								.collect::<String>(),
						});
					} else {
						sub_queries.push(SubQuery::new_static_text(&token.value));
					}
					// if last_pos < start {
					// 	sub_queries.push(SubQuery::new_static_text(&self[last_pos..start]));
					// }
					// let mut value: String = String::new();
					// for &ch in self[start..end].iter() {
					// 	value.push_str(&ch.escape_for_search_string());
					// }
					// sub_queries.push(SubQuery {
					// 	group: 0,
					// 	rule_idx: Some(rule.idx.index),
					// 	name: format!("{}{}", rule.name, capture.qualified_name),
					// 	value: value.clone(),
					// });
					// last_pos = end;
				}
				// let end: usize = self.len() - 1;
				// if last_pos < end {
				// 	sub_queries.push(SubQuery::new_static_text(&self[last_pos..end]));
				// }
				interpretations.push(Interpretation {
					sub_queries,
					stringified: String::new(),
				});
			}
		}

		interpretations
	}
}

impl std::ops::Deref for SearchStringView<'_> {
	type Target = [SymbolicChar];

	fn deref(&self) -> &Self::Target {
		self.as_str()
	}
}

impl SymbolicChar {
	pub fn is_wildcard(&self) -> bool {
		matches!(self, Self::WildcardStar | Self::WildcardOne)
	}

	pub fn maybe_delimiter(&self, lexer: &Lexer) -> bool {
		match *self {
			Self::Literal(ch) => lexer.schema.delimiters.contains(ch),
			Self::WildcardStar => true,
			Self::WildcardOne => true,
		}
	}

	pub fn escape_for_search_string(&self) -> String {
		(match self {
			Self::Literal('*') => "\\*",
			Self::Literal('?') => "\\?",
			Self::Literal('\\') => "\\\\",
			Self::Literal('<') => "\\<",
			Self::Literal('>') => "\\>",
			Self::Literal('(') => "\\(",
			Self::Literal(')') => "\\)",
			Self::Literal(ch) => {
				return format!("{ch}");
			},
			Self::WildcardStar => "*",
			Self::WildcardOne => "?",
		})
		.to_owned()
	}
}

impl std::fmt::Display for SymbolicChar {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.write_str(match self {
			Self::Literal('*') => "\\*",
			Self::Literal('?') => "\\?",
			Self::Literal('\\') => "\\\\",
			Self::Literal(ch) => {
				return ch.fmt(fmt);
			},
			Self::WildcardStar => "*",
			Self::WildcardOne => "?",
		})
	}
}

impl Interpretation {
	pub fn stringify(&self) -> String {
		let mut buf: String = String::new();
		for sub_query in self.sub_queries.iter() {
			if sub_query.is_static_text() {
				buf += &sub_query.value;
			} else {
				buf += &format!("<{}:{}>(", sub_query.rule_idx.map_or(0, NonZero::get), sub_query.name);
				buf += &sub_query.value;
				buf += ")";
			}
		}
		buf
	}

	fn append_sub_query(&mut self, suffix: Interpretation) {
		assert!(!suffix.sub_queries.is_empty());

		let Some(last): Option<&mut SubQuery> = self.sub_queries.last_mut() else {
			panic!("`Interpretation` should not be empty");
		};

		self.sub_queries.extend(suffix.sub_queries.into_iter());
		// if last.is_static_text() && suffix.is_static_text() {
		// 	last.symbols.extend(&suffix.symbols[..]);
		// } else {
		// 	self.sub_queries.push(suffix);
		// }
	}
}

impl SubQuery {
	fn new_static_text(symbols: &[SymbolicChar]) -> Self {
		Self {
			group: 0,
			rule_idx: None,
			name: String::new(),
			value: symbols
				.iter()
				.map(SymbolicChar::escape_for_search_string)
				.collect::<String>(),
		}
	}

	fn is_static_text(&self) -> bool {
		self.rule_idx.is_none()
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::schema::SchemaBuilder;

	#[test]
	fn basic() {
		let mut builder: SchemaBuilder = SchemaBuilder::new();
		builder
			.add_rule("email", r"(?<user>\w+)@(?<parts>\w+\.)+(?<tld>\w+)")
			.unwrap();

		let schema: Schema = builder.build();

		{
			println!("===");
			let search: SearchString = SearchString::parse("a*c").unwrap();
			let search: SearchString = SearchString::parse("a*@*com").unwrap();

			for tokens in search.interpretations_for_name(&schema, "email") {
				println!("- {tokens:?}");
			}
		}

		// panic!();
	}
}
