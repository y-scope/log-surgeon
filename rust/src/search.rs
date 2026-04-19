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

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum SymbolicChar {
	Literal(char),
	WildcardStar,
	WildcardOne,
}

impl std::fmt::Debug for SymbolicChar {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			Self::Literal('*') => r"\*".fmt(fmt),
			Self::Literal('?') => r"\?".fmt(fmt),
			Self::Literal('\\') => r"\\".fmt(fmt),
			Self::Literal(ch) => ch.fmt(fmt),
			Self::WildcardStar => fmt.write_str("*"),
			Self::WildcardOne => fmt.write_str("?"),
		}
	}
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Interpretation {
	pub sub_queries: Vec<SubQuery>,
}

impl std::fmt::Debug for Interpretation {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let Some(first): Option<&SubQuery> = self.sub_queries.first() else {
			return fmt.write_str("[]");
		};
		fmt.write_str("[")?;
		first.fmt(fmt)?;
		for sub_query in self.sub_queries[1..].iter() {
			fmt.write_str(", ")?;
			sub_query.fmt(fmt)?;
		}
		fmt.write_str("]")?;
		Ok(())
	}
}

#[derive(Clone)]
pub struct SubQuery {
	pub group: usize,
	pub rule_idx: Option<NonZero<u16>>,
	pub name: String,
	pub value: Vec<SymbolicChar>,
	pub is_all_wildcards: bool,
	pub dfa: Tdfa,
}

impl Eq for SubQuery {}

impl Ord for SubQuery {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(&self.group, &self.rule_idx, &self.name, &self.value)
			.cmp(&(&other.group, &other.rule_idx, &other.name, &other.value))
	}
}

impl PartialOrd for SubQuery {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialEq for SubQuery {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other).is_eq()
	}
}

impl std::fmt::Debug for SubQuery {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let value: String = self
			.value
			.iter()
			.map(SymbolicChar::escape_for_search_string)
			.collect::<String>();
		if self.is_static_text() {
			fmt.write_fmt(format_args!("({}:{})", self.group, value))
		} else {
			fmt.write_fmt(format_args!(
				"({}:?<{}:{}>{})",
				self.group,
				self.rule_idx.map_or(0, NonZero::get),
				self.name,
				value,
			))
		}
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
				.interpretations_for_name_internal(schema, &rows, 0);
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

				let mut single_token_interpretations: Vec<Interpretation> =
					sub_view.single_token_interpretations(schema, &rows, group);

				if single_token_interpretations.is_empty() {
					continue;
				}

				group += 1;

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
	fn single_token_interpretations(
		&self,
		schema: &Schema,
		rows: &VariableOrCaptures<Regex>,
		group: usize,
	) -> Vec<Interpretation> {
		assert!(!self.is_empty());

		let extended: Self = self.extend_with_greedy_wildcards();

		let mut interpretations: Vec<Interpretation> = Vec::new();

		if self.as_str() == &[SymbolicChar::WildcardStar] {
			interpretations.push(Interpretation {
				sub_queries: vec![SubQuery::new_static_text(self.as_str(), schema, group)],
			});
		}

		let has_wildcard: bool = self.as_str().iter().any(SymbolicChar::is_wildcard);

		let potential_interpretations: Vec<Interpretation> =
			extended.interpretations_for_name_internal(schema, rows, group);

		if has_wildcard || potential_interpretations.is_empty() {
			interpretations.push(Interpretation {
				sub_queries: vec![SubQuery::new_static_text(extended.as_str(), schema, group)],
			});
		}

		interpretations.extend(potential_interpretations.into_iter());

		interpretations
	}

	fn extend_with_greedy_wildcards(&self) -> Self {
		let mut new_start: usize = self.start;
		let mut new_end: usize = self.end;
		while (new_start > 0) && (self.full_string.0[new_start] == SymbolicChar::WildcardStar) {
			new_start -= 1;
		}
		while (new_end < self.full_string.0.len()) && (self.full_string.0[new_end] == SymbolicChar::WildcardStar) {
			new_end += 1;
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
		group: usize,
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
					sub_queries.push(SubQuery::new_static_text(&self[0..1], schema, group));
				}

				let mut last_pos: usize = 0;

				for token in path.into_iter() {
					if let Some(capture) = token.maybe_capture {
						if token.value.iter().all(|&ch| ch == SymbolicChar::WildcardStar) {
							continue;
						}
						let name: String = format!("{}{}", rule.name, capture.qualified_name);
						sub_queries.push(SubQuery::new_leaf(rule, capture, name, token.value, schema, group));
					} else {
						sub_queries.push(SubQuery::new_static_text(&token.value, schema, group));
					}
				}
				interpretations.push(Interpretation { sub_queries });
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
	pub fn execute(&self, input_parts: &[&str]) -> bool {
		assert_eq!(self.sub_queries.len(), input_parts.len());

		std::iter::zip(self.sub_queries.iter(), input_parts.iter().copied())
			.all(|(sub_query, input)| sub_query.execute(input))
	}

	pub fn any<'a>(mut interpretations: impl Iterator<Item = &'a Self>, input_parts: &[&str]) -> bool {
		interpretations.any(|interp| interp.execute(input_parts))
	}

	// 	pub fn all<'a>(mut interpretations: impl Iterator<Item = &'a Self>, input_parts: &[&str]) -> bool {
	// 		interpretations.all(|interp| interp.execute(input_parts))
	// 	}

	fn append_sub_query(&mut self, mut suffix: Interpretation) {
		let Some(suffix_first): Option<&mut SubQuery> = suffix.sub_queries.first_mut() else {
			return;
		};
		let Some(me_last): Option<&mut SubQuery> = self.sub_queries.last_mut() else {
			return;
		};

		if me_last.is_static_text() && suffix_first.is_static_text() {
			me_last.value.extend(suffix_first.value.drain(..));
			self.sub_queries.extend(suffix.sub_queries.drain(1..));
		} else {
			self.sub_queries.extend(suffix.sub_queries.into_iter());
		}
	}
}

impl SubQuery {
	#[tracing::instrument(skip_all, level = "trace")]
	pub fn execute(&self, input: &str) -> bool {
		if self.is_all_wildcards {
			return true;
		}
		self.dfa
			.execute_without_captures(input, u32::from('\n'), &mut self.dfa.execution_data())
			.map_or(false, |matched_rule| matched_rule.lexeme.len() == input.len())
	}

	fn new_static_text(symbols: &[SymbolicChar], schema: &Schema, group: usize) -> Self {
		let is_all_wildcards: bool = symbols.iter().all(|&ch| ch == SymbolicChar::WildcardStar);
		Self {
			group,
			rule_idx: None,
			name: String::new(),
			value: symbols.to_vec(),
			is_all_wildcards,
			dfa: Self::dfa_for(symbols, &schema.delimiters),
		}
	}

	fn new_leaf(
		rule: &Rule,
		capture: RegexCapture,
		name: String,
		symbols: Vec<SymbolicChar>,
		schema: &Schema,
		group: usize,
	) -> Self {
		let is_all_wildcards: bool = symbols.iter().all(|&ch| ch == SymbolicChar::WildcardStar);
		let dfa: Tdfa = Self::dfa_for(&symbols, &schema.delimiters);
		Self {
			group,
			rule_idx: Some(rule.idx.index),
			name,
			value: symbols,
			is_all_wildcards,
			dfa,
		}
	}

	fn is_static_text(&self) -> bool {
		self.rule_idx.is_none()
	}

	#[tracing::instrument(skip_all, level = "trace")]
	fn dfa_for(symbols: &[SymbolicChar], delimiters: &str) -> Tdfa {
		let mut sequence: Vec<Regex> = Vec::new();
		for &ch in symbols.iter() {
			match ch {
				SymbolicChar::Literal(ch) => {
					sequence.push(Regex::Literal(ch));
				},
				SymbolicChar::WildcardOne => {
					sequence.push(Regex::BoundedRepetition {
						min: 0,
						max: 1,
						item: Box::new(Regex::AnyChar),
					});
				},
				SymbolicChar::WildcardStar => {
					sequence.push(Regex::KleeneClosure(Box::new(Regex::AnyChar)));
				},
			}
		}
		let regex: Regex = Regex::Sequence(sequence);
		Tdfa::for_rules(
			&[Rule {
				idx: RuleIdx {
					priority: 0,
					position: 0,
					index: NonZero::<u16>::MAX,
				},
				name: String::new(),
				regex: TopLevelRegex {
					anchor_before: false,
					anchor_after: false,
					inner: regex,
				},
				capture_info: Vec::new(),
			}],
			delimiters.to_owned(),
		)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::schema::SchemaBuilder;

	#[test]
	fn search_email() {
		let mut builder: SchemaBuilder = SchemaBuilder::new();
		builder
			.add_rule("email", r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)")
			.unwrap();

		let schema: Schema = builder.build();

		{
			let interpretations: Vec<Interpretation> = search(&schema, "a*@*com", "email");

			assert!(interpretations.iter().any(execute(&["a", "@", "example", "com"])));
			assert!(interpretations.iter().any(execute(&["aa", "@", "example", "com"])));
			assert!(interpretations.iter().any(execute(&["aa", "@", "", "com"])));

			assert!(!interpretations.iter().any(execute(&["", "@", "example", "com"])));
			assert!(!interpretations.iter().any(execute(&["aa", "@@", "example", "com"])));
			assert!(!interpretations.iter().any(execute(&["a", "@", "example", "org"])));
		}

		{
			println!("===");
			let interpretations: Vec<Interpretation> = search(&schema, "*a@foo.*", "email");

			for i in interpretations.iter() {
				println!("- {i:?}");
			}
		}
	}

	fn search(schema: &Schema, query: &str, name: &str) -> Vec<Interpretation> {
		let query: SearchString = SearchString::parse(query).unwrap();

		let interpretations: Vec<Interpretation> = query.interpretations_for_name(&schema, name);

		interpretations
	}

	fn execute(parts: &[&str]) -> impl Fn(&Interpretation) -> bool {
		|interp| interp.execute(parts)
	}

	// #[test]
	// fn full_log_search() {
	// 	let mut builder: SchemaBuilder = SchemaBuilder::new();
	// 	builder
	// 		.add_rule("email", r"(?<user>\w+)@(?<parts>\w+\.)+(?<tld>\w+)")
	// 		.unwrap();

	// 	let schema: Schema = builder.build();

	// 	let search: SearchString = SearchString::parse("a@com*").unwrap();

	// 	println!("=== Interpretations");
	// 	for interpretation in search.interpretations_for_name(&schema, "") {
	// 		println!("- {interpretation:?}");
	// 	}
	// }
}
