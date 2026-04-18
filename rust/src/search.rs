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
use crate::regex::TopLevelRegex;
use crate::schema::Rule;
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
	sub_queries: Vec<SubQuery>,
	pub stringified: String,
}

#[derive(Debug, Clone)]
pub struct Interpretation2 {
	// pub tokens: Vec<SymbolicToken>,
	pub leaf_captures: Vec<Capture>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct SymbolicToken {
	rule_idx: Option<NonZero<u16>>,
	start: usize,
	end: usize,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SubQuery {
	name: String,
	symbols: Vec<SymbolicChar>,
}

impl std::fmt::Debug for SubQuery {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		fmt.debug_struct("SubQuery")
			.field("name", &self.name)
			.field(
				"symbols",
				&self.symbols.iter().map(|ch| ch.to_string()).collect::<String>(),
			)
			.finish()
	}
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
			.field(&self.as_str().iter().map(|ch| ch.to_string()).collect::<String>())
			.finish()
	}
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct SimulationState {
	position: usize,
	dfa_state: usize,
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
		// Anchor
		chars.push(SymbolicChar::Literal('\0'));
		Ok(Self(chars))
	}

	pub fn as_slice(&self) -> &[SymbolicChar] {
		&self.0
	}

	pub fn interpretations_for_name(&self, schema: &Schema, name: &str) -> Vec<Interpretation2> {
		let mut interpretations: Vec<Interpretation2> = Vec::new();

		let Some(rows): Option<VariableOrCaptures<Regex>> = schema.regexes_for_name(name) else {
			return interpretations;
		};

		match rows {
			VariableOrCaptures::Variable(rows) => {
				for (rule_idx, _regex) in rows.into_iter() {
					let rule: &Rule = &schema[rule_idx];
					let dfa: Tdfa = Tdfa::for_rules(std::iter::once(rule), schema.delimiters.clone());
					let mut data: TdfaExecution = dfa.execution_data();
					for (_rule, captures) in dfa.simulate(&self.0) {
						let mut leaf_captures: Vec<Capture> = Vec::new();
						for capture in captures.iter() {
							if capture.is_leaf {
								leaf_captures.push(Capture {
									rule_idx: rule.idx,
									capture_id: Some(capture.capture_id),
									parent_index: capture.parent_index,
									parent_id: capture.parent_id,
									range: capture.range,
									is_leaf: true,
									ffi_pointers: CaptureFfiPointers {
										parent: std::ptr::null(),
										lexeme: UncheckedCArray::NULL,
										variable_name: UncheckedCArray::NULL,
										capture_name: UncheckedCArray::NULL,
									},
								});
							}
						}
						if !leaf_captures.is_empty() {
							interpretations.push(Interpretation2 { leaf_captures });
						}
					}
				}
			},
			VariableOrCaptures::Captures(rows) => {
				for (rule_idx, _info, regex) in rows.into_iter() {
					let rule: &Rule = &schema[rule_idx];
					let dfa: Tdfa = Tdfa::for_rules(std::iter::once(rule), schema.delimiters.clone());
					let regex: Regex = Regex::Sequence(vec![Regex::AnyChar, regex]);
					let dfa: Tdfa = Tdfa::for_rules(
						std::iter::once(&Rule::new(
							rule.idx,
							rule.name.clone(),
							TopLevelRegex {
								anchor_before: false,
								anchor_after: false,
								inner: regex,
							},
						)),
						schema.delimiters.clone(),
					);
					for (_rule, captures) in dfa.simulate(&self.0) {
						let mut leaf_captures: Vec<Capture> = Vec::new();
						for capture in captures.iter() {
							if capture.is_leaf {
								leaf_captures.push(Capture {
									rule_idx: rule.idx,
									capture_id: Some(capture.capture_id),
									parent_index: capture.parent_index,
									parent_id: capture.parent_id,
									range: capture.range,
									is_leaf: true,
									ffi_pointers: CaptureFfiPointers {
										parent: std::ptr::null(),
										lexeme: UncheckedCArray::NULL,
										variable_name: UncheckedCArray::NULL,
										capture_name: UncheckedCArray::NULL,
									},
								});
							}
						}
						if !leaf_captures.is_empty() {
							interpretations.push(Interpretation2 { leaf_captures });
						}
					}
				}
			},
		}

		interpretations
	}

	pub fn interpretations(&self, lexer: &Lexer) -> Vec<Interpretation> {
		let mut tokens_at_index: Vec<Option<BTreeSet<(usize, usize)>>> = vec![None; self.0.len()];

		let mut queue: Vec<(usize, Vec<(usize, usize, usize)>)> = vec![(0, vec![])];
		let mut done: Vec<(usize, Vec<(usize, usize, usize)>)> = Vec::new();

		while !queue.is_empty() {
			let mut new_queue: Vec<(usize, Vec<(usize, usize, usize)>)> = Vec::new();
			for (start, string) in queue.iter() {
				if *start == self.0.len() {
					done.push((*start, string.clone()));
					continue;
				}
				let tokens: &BTreeSet<(usize, usize)> =
					// tokens_at_index[*start].get_or_insert_with(|| lexer.dfa.simulate2(&self.0[*start..]));
					&Default::default();

				if tokens.is_empty() {
					if let Some(delimiter) = self.0[*start..].iter().position(|ch| ch.maybe_delimiter(lexer)) {
						new_queue.push((*start + delimiter + 1, string.clone()));
					} else {
						done.push((*start, string.clone()));
					}
				} else {
					for &(end, rule) in tokens.iter() {
						let mut new_string: Vec<(usize, usize, usize)> = string.clone();
						new_string.push((*start, start + end + 1, rule));
						new_queue.push((start + end + 1, new_string));
					}
				}
			}

			queue = new_queue;
		}

		done.into_iter()
			.map(|(end, string)| {
				let mut previous: usize = 0;
				let mut sub_queries: Vec<SubQuery> = Vec::new();
				for (start, end, rule) in string.into_iter() {
					if previous < start {
						sub_queries.push(SubQuery {
							name: String::new(),
							symbols: self.0[previous..start].to_owned(),
						});
					}
					sub_queries.push(SubQuery {
						name: lexer.rule_name(rule).to_owned(),
						symbols: self.0[start..end].to_owned(),
					});
					previous = end;
				}
				if previous < end {
					sub_queries.push(SubQuery {
						name: String::new(),
						symbols: self.0[previous..end].to_owned(),
					});
				}
				let mut tmp: Interpretation = Interpretation {
					sub_queries,
					stringified: String::new(),
				};
				let stringified: String = tmp.stringify();
				Interpretation {
					sub_queries: tmp.sub_queries,
					stringified,
				}
			})
			.collect::<Vec<_>>()
	}

	fn multi_token_interpretations(&self, lexer: &Lexer) -> Vec<Interpretation> {
		if self.0.is_empty() {
			return vec![];
		}

		let mut interpretations: Vec<BTreeSet<Interpretation>> = vec![BTreeSet::new(); self.0.len()];

		for end in 1..=self.0.len() {
			for start in 0..end {
				let sub_view: SearchStringView<'_> = SearchStringView {
					full_string: &self,
					start,
					end,
				};
				if (sub_view.as_str() != &[SymbolicChar::WildcardStar])
					&& ((sub_view.as_str().first() == Some(&SymbolicChar::WildcardStar))
						|| (sub_view.as_str().last() == Some(&SymbolicChar::WildcardStar)))
				{
					continue;
				}

				let single_token_interpretations: Vec<SubQuery> = sub_view.single_token_interpretations(lexer);

				println!(
					"single token interpreations for {start}..{end}: {sub_view:?} are: {single_token_interpretations:?}"
				);

				if single_token_interpretations.is_empty() {
					continue;
				}

				if start == 0 {
					for suffix in single_token_interpretations.into_iter() {
						// interpretations[end - 1].insert(Interpretation(vec![suffix]));
					}
				} else {
					// TODO explain clone
					for prefix in interpretations[start - 1].clone().iter() {
						for suffix in single_token_interpretations.iter() {
							let mut combined: Interpretation = prefix.clone();
							combined.append_sub_query(suffix.clone());
							interpretations[end - 1].insert(combined);
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
	fn single_token_interpretations(&self, lexer: &Lexer) -> Vec<SubQuery> {
		assert!(!self.as_str().is_empty());

		if (self.start, self.end) == (0, self.full_string.0.len()) {
			return vec![SubQuery::from(self.as_str())];
		}

		let extended: Self = self.extend();

		if (self.start > 0) && self.full_string.0[self.start - 1].is_wildcard() {
			if (self.end < self.full_string.0.len()) && self.full_string.0[self.end + 1].is_wildcard() {
				return vec![SubQuery::from(extended.as_str())];
			}
		}

		let mut interpretations: Vec<SubQuery> = Vec::new();

		let has_wildcard: bool = self.as_str().iter().find(|ch| ch.is_wildcard()).is_some();

		// let potential_rules: BTreeSet<usize> = lexer.dfa.simulate(extended.as_str());
		let potential_rules: BTreeSet<usize> = Default::default();

		if has_wildcard || potential_rules.is_empty() {
			interpretations.push(SubQuery::from(extended.as_str()));
		}

		for &rule in potential_rules.iter() {
			interpretations.push(SubQuery {
				name: lexer.rule_name(rule).to_owned(),
				symbols: extended.as_str().to_owned(),
			});
		}

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
				for ch in sub_query.symbols.iter() {
					buf += &ch.escape_for_search_string();
				}
			} else {
				buf += &format!("<{}>(", sub_query.name);
				for ch in sub_query.symbols.iter() {
					buf += &ch.escape_for_search_string();
				}
				buf += ")";
			}
		}
		buf
	}

	fn append_sub_query(&mut self, suffix: SubQuery) {
		assert!(!suffix.symbols.is_empty());

		let Some(last): Option<&mut SubQuery> = self.sub_queries.last_mut() else {
			panic!("`Interpretation` should not be empty");
		};

		if last.is_static_text() && suffix.is_static_text() {
			last.symbols.extend(&suffix.symbols[..]);
		} else {
			self.sub_queries.push(suffix);
		}
	}
}

impl SubQuery {
	fn is_static_text(&self) -> bool {
		self.name.is_empty()
	}
}

impl From<&[SymbolicChar]> for SubQuery {
	fn from(static_text: &[SymbolicChar]) -> Self {
		SubQuery {
			name: String::new(),
			symbols: static_text.to_owned(),
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
}
