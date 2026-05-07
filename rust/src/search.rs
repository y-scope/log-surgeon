#![allow(unused)]

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZero;

use crate::dfa::Tdfa;
use crate::dfa::TdfaExecution;
use crate::ffi::UncheckedCArray;
use crate::log_event::Match;
use crate::log_event::MatchFfiPointers;
use crate::nfa::PathComponent;
use crate::nfa::Tnfa;
use crate::regex::AnchoredRegex;
use crate::regex::Regex;
use crate::regex::SubRule;
use crate::schema::RootRule;
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
	pub rule_name: String,
	pub qualified_name: String,
	pub value: String,
	pub symbolic_value: Vec<SymbolicChar>,
	// pub is_all_wildcards: bool,
	// pub dfa: Tdfa,
}

impl Eq for SubQuery {}

impl Ord for SubQuery {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(&self.group, &self.rule_idx, &self.qualified_name, &self.value).cmp(&(
			&other.group,
			&other.rule_idx,
			&other.qualified_name,
			&other.value,
		))
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
		if let Some(rule_idx) = self.rule_idx {
			fmt.write_fmt(format_args!(
				"({}:?<{}:{}>{})",
				self.group, rule_idx, self.qualified_name, self.value,
			))
		} else {
			fmt.write_str(&self.value)
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

impl std::fmt::Display for SearchStringView<'_> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for ch in self.iter() {
			ch.fmt(fmt)?;
		}
		Ok(())
	}
}

impl Interpretation {
	fn dedup(interpretations: &mut Vec<Self>) {
		interpretations.iter_mut().for_each(Self::canonicalize);

		interpretations.sort();
		interpretations.dedup();

		let mut i: usize = 0;
		while i < interpretations.len() {
			let mut j: usize = i + 1;
			while j < interpretations.len() {
				if interpretations[i].sub_queries.len() != interpretations[j].sub_queries.len() {
					j += 1;
					continue;
				}
				if std::iter::zip(
					interpretations[i].sub_queries.iter(),
					interpretations[j].sub_queries.iter(),
				)
				.all(|(query1, query2)| query1.subsumes(query2))
				{
					interpretations.remove(j);
					continue;
				}
				if std::iter::zip(
					interpretations[i].sub_queries.iter(),
					interpretations[j].sub_queries.iter(),
				)
				.all(|(query1, query2)| query2.subsumes(query1))
				{
					interpretations.swap(i, j);
					interpretations.remove(j);
					j = i + 1;
					continue;
				}
				j += 1;
			}
			i += 1;
		}
	}

	fn canonicalize(&mut self) {
		let mut groups: BTreeMap<usize, usize> = BTreeMap::new();
		for sub_query in self.sub_queries.iter_mut() {
			let n: usize = groups.len();
			let new_group: usize = *groups.entry(sub_query.group).or_insert(n);
			sub_query.group = new_group;
		}
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
				// println!(
				// 	"== {start}..{end}, {sub_view}, {}",
				// 	sub_view.extend_with_greedy_wildcards()
				// );
				if (sub_view.as_str() != &[SymbolicChar::WildcardStar])
					&& ((sub_view.as_str().first() == Some(&SymbolicChar::WildcardStar))
						|| (sub_view.as_str().last() == Some(&SymbolicChar::WildcardStar)))
				{
					continue;
				}

				let mut single_token_interpretations: Vec<Interpretation> =
					sub_view.single_token_interpretations(schema, group);

				// for i in single_token_interpretations.iter() {
				// 	println!("- {i:?}");
				// }

				if single_token_interpretations.is_empty() {
					continue;
				}

				group += 1;

				if start == 0 {
					for suffix in single_token_interpretations.into_iter() {
						interpretations[end - 1].push(suffix);
					}
				} else {
					// Remark: `interpretations[start - 1]` and `interpretations[end - 1]` cannot alias
					// since `start < end`, but rustc doesn't know that.
					for prefix in interpretations[start - 1].clone().iter() {
						for suffix in single_token_interpretations.iter() {
							let mut combined: Interpretation = prefix.clone();
							combined.append_sub_query(suffix.clone());
							interpretations[end - 1].push(combined);
						}
					}
				}
			}

			Interpretation::dedup(&mut interpretations[end - 1]);
			// println!("interpretations up to {end} are {:?}", interpretations[end - 1]);
		}

		interpretations.pop().unwrap()
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
	fn single_token_interpretations(&self, schema: &Schema, group: usize) -> Vec<Interpretation> {
		assert!(!self.is_empty());

		let extended: Self = self.extend_with_greedy_wildcards();

		let mut interpretations: Vec<Interpretation> = Vec::new();

		if self.as_str() == &[SymbolicChar::WildcardStar] {
			interpretations.push(Interpretation {
				sub_queries: vec![SubQuery::new_static_text(&[SymbolicChar::WildcardStar], schema, group)],
			});
			return interpretations;
		}

		let has_wildcard: bool = extended.as_str().iter().any(SymbolicChar::is_wildcard);

		let potential_interpretations: Vec<Interpretation> =
			extended.interpretations_for_nfa(schema, &schema.main_nfa, group);

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

		for i in (0..self.start).rev() {
			if self.full_string.0[i] == SymbolicChar::WildcardStar {
				new_start = i;
			} else {
				break;
			}
		}
		for i in self.end..self.full_string.0.len() {
			if self.full_string.0[i] == SymbolicChar::WildcardStar {
				new_end = i + 1;
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

	fn as_regex(&self) -> Regex {
		Regex::Sequence(
			self.as_str()
				.iter()
				.map(|&ch| match ch {
					SymbolicChar::Literal(ch) => Regex::Literal(ch),
					SymbolicChar::WildcardStar => Regex::KleeneClosure(Box::new(Regex::AnyChar)),
					SymbolicChar::WildcardOne => Regex::BoundedRepetition {
						min: 0,
						max: 1,
						item: Box::new(Regex::AnyChar),
					},
				})
				.collect::<Vec<_>>(),
		)
	}

	fn interpretations_for_name_internal(
		&self,
		schema: &Schema,
		rows: &VariableOrCaptures<Regex>,
		group: usize,
	) -> Vec<Interpretation> {
		let mut interpretations: Vec<Interpretation> = Vec::new();

		let search_nfa: Tnfa = Tnfa::for_regex(&self.as_regex());

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
			let rule_nfa: Tnfa = Tnfa::for_single_rule(rule.idx, &regex);
			interpretations.extend(self.interpretations_for_nfa(schema, &rule_nfa, group).into_iter());
		}

		// Interpretation::dedup(&mut interpretations);
		interpretations
	}

	fn interpretations_for_nfa(&self, schema: &Schema, nfa: &Tnfa, group: usize) -> Vec<Interpretation> {
		let mut interpretations: Vec<Interpretation> = Vec::new();

		let search_nfa: Tnfa = Tnfa::for_regex(&self.as_regex());

		for path in nfa.intersect(&search_nfa).compute_paths() {
			let mut sub_queries: Vec<SubQuery> = Vec::new();

			// if self[0].is_wildcard() {
			// 	sub_queries.push(SubQuery::new_static_text(&self[0..1], schema, group));
			// }

			let mut last_pos: usize = 0;

			for token in path.iter() {
				match token {
					PathComponent::Literal(_) => {
						sub_queries.push(SubQuery {
							group,
							rule_idx: None,
							rule_name: String::new(),
							qualified_name: String::new(),
							value: token.to_query_string(),
							symbolic_value: token.to_symbolic_chars(),
						});
					},
					PathComponent::Capture {
						rule_idx,
						qualified_name,
						contents,
					} => {
						let rule: &RootRule = &schema[*rule_idx];
						sub_queries.push(SubQuery {
							group,
							rule_idx: Some(rule.idx.get()),
							rule_name: rule.name.clone(),
							qualified_name: rule.name.clone() + qualified_name,
							value: token.to_query_string(),
							symbolic_value: token.to_symbolic_chars(),
						});
					},
					PathComponent::QueryWildcard | PathComponent::PatternWildcard => {
						sub_queries.push(SubQuery {
							group,
							rule_idx: None,
							rule_name: String::new(),
							qualified_name: String::new(),
							value: "*".to_owned(),
							symbolic_value: vec![SymbolicChar::WildcardStar],
						});
					},
				}
			}
			interpretations.push(Interpretation { sub_queries });
		}

		interpretations.iter_mut().for_each(Interpretation::condense);
		Interpretation::dedup(&mut interpretations);

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

		false
		// std::iter::zip(self.sub_queries.iter(), input_parts.iter().copied())
		// 	.all(|(sub_query, input)| sub_query.execute(input))
	}

	pub fn any<'a>(mut interpretations: impl Iterator<Item = &'a Self>, input_parts: &[&str]) -> bool {
		interpretations.any(|interp| interp.execute(input_parts))
	}

	// TODO same as `Path::condense`
	fn condense(&mut self) {
		let mut i: usize = 1;
		while i < self.sub_queries.len() {
			let previous: &SubQuery = &self.sub_queries[i - 1];
			let current: &SubQuery = &self.sub_queries[i];
			if previous.is_static_text() && current.is_static_text() {
				let value: String = previous.value.clone() + &current.value;
				let mut symbolic_value: Vec<SymbolicChar> = previous.symbolic_value.clone();
				symbolic_value.extend(current.symbolic_value.iter().copied());
				self.sub_queries[i - 1].value = value;
				self.sub_queries[i - 1].symbolic_value = symbolic_value;
				self.sub_queries.remove(i);
			} else {
				i += 1;
			}
		}
	}

	fn append_sub_query(&mut self, mut suffix: Interpretation) {
		let Some(me_last): Option<&mut SubQuery> = self.sub_queries.last_mut() else {
			*self = suffix;
			return;
		};
		let Some(suffix_first): Option<&mut SubQuery> = suffix.sub_queries.first_mut() else {
			return;
		};

		if me_last.is_static_text() && suffix_first.is_static_text() {
			me_last.value.extend(suffix_first.value.drain(..));
			me_last.symbolic_value.extend(suffix_first.symbolic_value.drain(..));
			self.sub_queries.extend(suffix.sub_queries.drain(1..));
		} else {
			self.sub_queries.extend(suffix.sub_queries.into_iter());
		}
	}

	fn invariants(&self) {
		let mut last_was_static_text: bool = false;
		let mut i: usize = 0;
		for sub_query in self.sub_queries.iter() {
			if sub_query.is_static_text() {
				assert!(!last_was_static_text);
				last_was_static_text = true;
			} else {
				last_was_static_text = false;
			}
			i += 1;
		}
	}
}

impl SubQuery {
	fn new_static_text(symbols: &[SymbolicChar], schema: &Schema, group: usize) -> Self {
		let is_all_wildcards: bool = symbols.iter().all(|&ch| ch == SymbolicChar::WildcardStar);
		Self {
			group,
			rule_idx: None,
			rule_name: String::new(),
			qualified_name: String::new(),
			value: symbols.iter().fold(String::new(), |mut accum, &ch| {
				accum.push_str(&ch.to_string());
				accum
			}),
			symbolic_value: symbols.to_owned(),
		}
	}

	fn is_static_text(&self) -> bool {
		self.rule_idx.is_none()
	}

	fn subsumes(&self, other: &Self) -> bool {
		if (self.group, self.rule_idx, &self.qualified_name) != (other.group, other.rule_idx, &other.qualified_name) {
			return false;
		}
		// Remark: Always has 1 subslice.
		let mut other_parts: Vec<&[SymbolicChar]> = other
			.symbolic_value
			.split(|&ch| ch == SymbolicChar::WildcardStar)
			.collect::<Vec<_>>();
		if *other.symbolic_value.last().unwrap() == SymbolicChar::WildcardStar {
			if *self.symbolic_value.last().unwrap() != SymbolicChar::WildcardStar {
				return false;
			}
			let last: &[SymbolicChar] = other_parts.pop().unwrap();
			assert_eq!(last, &[]);
		}
		let mut i: usize = 0;
		if *other.symbolic_value.first().unwrap() == SymbolicChar::WildcardStar {
			if *self.symbolic_value.first().unwrap() != SymbolicChar::WildcardStar {
				return false;
			}
			assert_eq!(other_parts[0], &[]);
			i += 1;
		}
		for my_part in self.symbolic_value.split(|&ch| ch == SymbolicChar::WildcardStar) {
			if my_part.is_empty() {
				continue;
			}
			let Some(other_part): Option<&[SymbolicChar]> = other_parts.get(i).copied() else {
				return false;
			};
			if let Some(suffix) = other_part.strip_prefix(my_part) {
				if suffix.is_empty() {
					i += 1;
				} else {
					other_parts[i] = suffix;
				}
			} else {
				return false;
			}
		}
		i == other_parts.len()
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
			let interpretations: Vec<Interpretation> = search_by_name(&schema, "*a*@*mail*example*", "email");
			println!("===");

			for i in interpretations.iter() {
				println!("- {i:?}");
			}
		}
	}

	#[test]
	fn test_subsumes() {
		let a: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			rule_name: String::new(),
			qualified_name: String::new(),
			value: String::new(),
			symbolic_value: vec![SymbolicChar::Literal('a'), SymbolicChar::WildcardStar],
		};
		let b: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			rule_name: String::new(),
			qualified_name: String::new(),
			value: String::new(),
			symbolic_value: vec![SymbolicChar::Literal('a')],
		};
		let c: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			rule_name: String::new(),
			qualified_name: String::new(),
			value: String::new(),
			symbolic_value: vec![SymbolicChar::WildcardStar, SymbolicChar::Literal('a')],
		};
		let d: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			rule_name: String::new(),
			qualified_name: String::new(),
			value: String::new(),
			symbolic_value: vec![SymbolicChar::Literal('a')],
		};
		let e: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			rule_name: String::new(),
			qualified_name: String::new(),
			value: String::new(),
			symbolic_value: vec![SymbolicChar::WildcardStar],
		};

		assert!(a.subsumes(&b));
		assert!(!b.subsumes(&a));

		assert!(c.subsumes(&d));
		assert!(!d.subsumes(&e));

		assert!(!a.subsumes(&c));
		assert!(!c.subsumes(&a));

		assert!(a.subsumes(&a));
		assert!(b.subsumes(&b));
		assert!(c.subsumes(&c));
		assert!(d.subsumes(&d));
		assert!(e.subsumes(&e));
	}

	#[test]
	fn full_log_search() {
		let mut builder: SchemaBuilder = SchemaBuilder::new();
		builder
			.add_rule("email", r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)")
			.unwrap();

		let schema: Schema = builder.build();

		let search: SearchString = SearchString::parse("a@com*").unwrap();

		let interpretations: Vec<Interpretation> = search.interpretations_for_name(&schema, "");

		println!("=== Interpretations");
		for interpretation in interpretations.iter() {
			println!("- {interpretation:?}");
		}
	}

	#[test]
	fn search_single_token_interpretation() {
		let mut builder: SchemaBuilder = SchemaBuilder::new();
		builder
			.add_rule("email", r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)")
			.unwrap();

		let schema: Schema = builder.build();

		{
			let query: SearchString = SearchString::parse("a@com*").unwrap();
			let interpretations: Vec<Interpretation> = search_single_token(&schema, "a@com*");

			println!("=== Interpretations");
			for i in interpretations.iter() {
				println!("- {i:?}");
			}
			println!("=== Done");
		}
	}

	fn search_by_name(schema: &Schema, query: &str, name: &str) -> Vec<Interpretation> {
		let query: SearchString = SearchString::parse(query).unwrap();

		let interpretations: Vec<Interpretation> = query.interpretations_for_name(&schema, name);

		interpretations
	}

	fn search_single_token(schema: &Schema, query: &str) -> Vec<Interpretation> {
		let query: SearchString = SearchString::parse(query).unwrap();

		let interpretations: Vec<Interpretation> =
			query.view(0, query.0.len()).single_token_interpretations(&schema, 0);

		interpretations
	}
}
