use std::collections::BTreeMap;
use std::sync::Arc;

use crate::nfa::Path;
use crate::nfa::PathComponent;
use crate::nfa::Tnfa;
use crate::regex::Regex;
use crate::schema::RootRule;
use crate::schema::RuleIdx;
use crate::schema::RuleInfo;
use crate::schema::Schema;
use crate::schema::VariableOrCaptures;

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
	pub rule_idx: Option<RuleIdx>,
	pub fully_qualified_name: Arc<str>,
	pub symbolic_value: Vec<SymbolicChar>,
	pub string_value: String,
}

impl std::fmt::Debug for SubQuery {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if let Some(rule_idx) = self.rule_idx {
			fmt.write_fmt(format_args!(
				"({}:?<{}:{}>{})",
				self.group, rule_idx, self.fully_qualified_name, self.string_value,
			))
		} else {
			fmt.write_str(&self.string_value)
		}
	}
}

impl Eq for SubQuery {}

impl Ord for SubQuery {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(
			&self.group,
			&self.rule_idx,
			&self.fully_qualified_name,
			&self.symbolic_value,
		)
			.cmp(&(
				&other.group,
				&other.rule_idx,
				&other.fully_qualified_name,
				&other.symbolic_value,
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

#[derive(Debug, Clone)]
pub struct InterpretationPrefix {
	successors: BTreeMap<SubQuery, Self>,
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
	fn dedup_covered_interpretations(interpretations: &mut Vec<Self>) {
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

	fn dedup_non_greedy_interpretations(interpretations: &mut Vec<Self>) {
		let mut old_len: usize = 0;
		while old_len != interpretations.len() {
			let mut i: usize = 0;
			while i < interpretations.len() {
				debug!("== {i} / {}", interpretations.len());
				let mut j: usize = i + 1;
				while j < interpretations.len() {
					let interpretation1: &Interpretation = &interpretations[i];
					let interpretation2: &Interpretation = &interpretations[j];
					let mut next: usize = j + 1;
					for (k, (query1, query2)) in
						std::iter::zip(interpretation1.sub_queries.iter(), interpretation2.sub_queries.iter())
							.enumerate()
					{
						if query1.is_static_text() && query2.is_static_text() {
							if query1.symbolic_value == query2.symbolic_value {
								continue;
							} else {
								break;
							}
						}
						if query1.rule_idx != query2.rule_idx {
							break;
						}
						if query1.group != query2.group {
							break;
						}
						if query1.symbolic_value == query2.symbolic_value {
							if k == interpretation1.sub_queries.len() {
								debug!("- removing same interp");
								interpretations.remove(j);
								next = j;
								break;
							}
							continue;
						}
						if query1.symbolic_value.starts_with(&query2.symbolic_value) {
							assert!(query1.symbolic_value.len() > query2.symbolic_value.len());
							debug!("removing j < i:");
							debug!("- {interpretation1:?}");
							debug!("- {interpretation2:?}");
							interpretations.remove(j);
							next = j;
							break;
						}
						if query2.symbolic_value.starts_with(&query1.symbolic_value) {
							assert!(query2.symbolic_value.len() > query1.symbolic_value.len());
							debug!("removing i < j:");
							debug!("- {interpretation1:?}");
							debug!("- {interpretation2:?}");
							interpretations.swap(i, j);
							interpretations.remove(j);
							next = i + 1;
							break;
						}
					}
					j = next;
				}
				i += 1;
			}
			old_len = interpretations.len();
		}
	}

	/// Re-numbers groups consecutively starting from `0`.
	fn canonicalize(&mut self) {
		let mut groups: BTreeMap<usize, usize> = BTreeMap::from([(0, 0)]);
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

	pub fn get_interpretations(&self, schema: &Schema, name: &str) -> Vec<Interpretation> {
		let Some(rows): Option<VariableOrCaptures<Regex>> = schema.regexes_for_name(name) else {
			return Vec::new();
		};

		if !name.is_empty() {
			return self.view(0, self.0.len()).interpretations_for_name(schema, &rows, 0);
		}

		self.full_log_interpretations(schema)
	}

	fn full_log_interpretations(&self, schema: &Schema) -> Vec<Interpretation> {
		if self.0.is_empty() {
			return Vec::new();
		}

		let mut can_start_at_position: Vec<bool> = vec![false; self.0.len()];
		can_start_at_position[0] = true;
		let mut last_boundary: usize = 0;
		for i in 1..self.0.len() {
			if self.0[i - 1] == SymbolicChar::WildcardStar {
				can_start_at_position[i - 1] = true;
				can_start_at_position[i] = true;
				last_boundary = i;
				continue;
			}
			// TODO careful about newlines...
			if let SymbolicChar::Literal(ch) = self.0[i - 1]
				&& schema.delimiters.contains(ch)
			{
				if let SymbolicChar::Literal(ch) = self.0[i]
					&& !schema.delimiters.contains(ch)
				{
					can_start_at_position[i] = true;
					last_boundary = i;
					continue;
				}
			}
			let fragment: &[SymbolicChar] = &self.0[last_boundary..=i];
			let regex: Regex = Regex::Sequence(
				fragment
					.iter()
					.map(|&ch| match ch {
						SymbolicChar::Literal(ch) => Regex::Literal(ch),
						SymbolicChar::WildcardOne => Regex::BoundedRepetition {
							min: 0,
							max: 1,
							item: Box::new(Regex::AnyChar),
						},
						SymbolicChar::WildcardStar => Regex::KleeneClosure(Box::new(Regex::AnyChar)),
					})
					.collect::<Vec<_>>(),
			);
			let search_nfa: Tnfa = Tnfa::for_regex(&regex);
			let can_end_here: bool = schema.main_nfa.intersect(&search_nfa).can_accept();
			can_start_at_position[i] = can_end_here;
		}

		let mut interpretations_up_to_position: Vec<Vec<Interpretation>> = vec![Vec::new(); self.0.len()];
		// Group `0` for static text.
		let mut group: usize = 1;

		for end in 1..=self.0.len() {
			for start in 0..end {
				if !can_start_at_position[start] {
					continue;
				}

				let sub_view: SearchStringView<'_> = self.view(start, end);
				// println!("== {start}..{end} /{}: {sub_view:?}", self.0.len());

				if (sub_view.as_str().len() > 1)
					&& ((sub_view.as_str().first() == Some(&SymbolicChar::WildcardStar))
						|| (sub_view.as_str().last() == Some(&SymbolicChar::WildcardStar)))
				{
					continue;
				}

				// println!(
				// 	"== Interpretations ({start}..{end}): {sub_view}, {}, {}, {}, {:?}, {:?}",
				// 	sub_view.as_str() != &[SymbolicChar::WildcardStar],
				// 	sub_view.as_str().first() == Some(&SymbolicChar::WildcardStar),
				// 	sub_view.as_str().last() == Some(&SymbolicChar::WildcardStar),
				// 	sub_view.as_str().first(),
				// 	sub_view.as_str().last(),
				// );
				let single_token_interpretations: Vec<Interpretation> =
					sub_view.single_token_interpretations(schema, group);

				if single_token_interpretations.is_empty() {
					continue;
				}

				group += 1;

				if start == 0 {
					for suffix in single_token_interpretations.into_iter() {
						interpretations_up_to_position[end - 1].push(suffix);
					}
				} else {
					// Remark: `interpretations[start - 1]` and `interpretations[end - 1]` cannot alias
					// since `start < end`, but rustc doesn't know that.
					for prefix in interpretations_up_to_position[start - 1].clone().iter() {
						for suffix in single_token_interpretations.iter() {
							let mut combined: Interpretation = prefix.clone();
							combined.append_sub_query(suffix.clone());
							interpretations_up_to_position[end - 1].push(combined);
						}
					}
				}
			}

			interpretations_up_to_position[end - 1]
				.iter()
				.for_each(Interpretation::invariants);
		}

		let mut interpretations: Vec<Interpretation> = interpretations_up_to_position.pop().unwrap();
		interpretations.iter_mut().for_each(Interpretation::canonicalize);
		// Interpretation::dedup(&mut interpretations);
		Interpretation::dedup_non_greedy_interpretations(&mut interpretations);

		// println!("=== done {}", interpretations.len());

		interpretations
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

		let extended: Self = self.extend_to_greedy_wildcards();

		// println!("- {self:?} extends to {extended:?}");

		let mut interpretations: Vec<Interpretation> = Vec::new();

		if self.as_str() == &[SymbolicChar::WildcardStar] {
			interpretations.push(Interpretation {
				sub_queries: vec![SubQuery::new_static_text(vec![SymbolicChar::WildcardStar])],
			});
			return interpretations;
		}

		let has_wildcard: bool = extended.as_str().iter().any(SymbolicChar::is_wildcard);

		let potential_interpretations: Vec<Interpretation> =
			extended.interpretations_for_nfa(schema, &schema.main_nfa, group);

		if has_wildcard || potential_interpretations.is_empty() {
			if extended.ends_with_delimiter(schema) {
				interpretations.push(Interpretation {
					sub_queries: vec![SubQuery::new_static_text(extended.as_str().to_owned())],
				});
			}
		}

		for interpretation in potential_interpretations.into_iter() {
			if interpretation.sub_queries.iter().any(|sub_query| {
				sub_query.rule_idx.is_some() && (sub_query.symbolic_value != &[SymbolicChar::WildcardStar])
			}) {
				interpretations.push(interpretation);
			}
			// TODO more careful?
		}
		// interpretations.extend(potential_interpretations.into_iter());

		interpretations
	}

	fn extend_to_greedy_wildcards(&self) -> Self {
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

	fn surround_with_wildcards(&self) -> Vec<SymbolicChar> {
		let mut symbols: Vec<SymbolicChar> = Vec::with_capacity(self.len() + 2);
		if !self.starts_with(&[SymbolicChar::WildcardStar]) {
			symbols.push(SymbolicChar::WildcardStar);
		}
		symbols.extend(self.iter().copied());
		if !self.ends_with(&[SymbolicChar::WildcardStar]) {
			symbols.push(SymbolicChar::WildcardStar);
		}
		symbols
	}

	// TODO refactor with full log search
	fn to_regex(&self) -> Regex {
		Regex::Sequence(self.as_str().iter().map(SymbolicChar::to_regex).collect::<Vec<_>>())
	}

	fn interpretations_for_name(
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
			let rule_nfa: Tnfa = Tnfa::for_single_rule(rule.idx, &regex);
			interpretations.extend(self.interpretations_for_nfa(schema, &rule_nfa, group).into_iter());
		}

		interpretations.sort();
		interpretations.dedup();

		// Interpretation::dedup(&mut interpretations);
		interpretations
	}

	fn interpretations_for_nfa(&self, schema: &Schema, nfa: &Tnfa, group: usize) -> Vec<Interpretation> {
		assert!(self.as_str() != &[SymbolicChar::WildcardStar]);
		let mut interpretations: Vec<Interpretation> = Vec::new();

		let search_nfa: Tnfa = Tnfa::for_regex(&self.to_regex());

		// println!(
		// 	"== query ({}..{}..{}) {:?}, {:?}",
		// 	self.start,
		// 	self.end,
		// 	extra,
		// 	self,
		// 	self.to_regex(extra)
		// );
		let intersection: Tnfa = nfa.intersect(&search_nfa);
		let (paths, rules_potentially_without_captures): (Vec<Path>, Vec<RuleIdx>) = intersection.compute_paths();
		for path in paths.iter() {
			let mut sub_queries: Vec<SubQuery> = Vec::new();

			for token in path.iter() {
				match token {
					PathComponent::Literal(contents) => {
						sub_queries.push(SubQuery::new_static_text(contents.clone()));
					},
					PathComponent::Capture {
						rule_idx,
						maybe_sub_rule_id,
						contents,
					} => {
						let rule: &RootRule = &schema[*rule_idx];
						let rule_info: &RuleInfo = &rule[*maybe_sub_rule_id];
						sub_queries.push(SubQuery::new(group, rule_info, contents.clone()));
					},
				}
			}
			interpretations.push(Interpretation { sub_queries });
		}
		for &rule_idx in rules_potentially_without_captures.iter() {
			let rule: &RootRule = &schema[rule_idx];
			let rule_info: &RuleInfo = &rule[None];
			interpretations.push(Interpretation {
				sub_queries: vec![SubQuery::new(group, rule_info, self.as_str().to_owned())],
			});
		}

		interpretations.iter_mut().for_each(Interpretation::condense);

		Interpretation::dedup_covered_interpretations(&mut interpretations);
		interpretations
	}

	fn ends_with_delimiter(&self, schema: &Schema) -> bool {
		let SymbolicChar::Literal(ch): SymbolicChar = *self.as_str().last().unwrap() else {
			return true;
		};
		schema.delimiters.contains(ch)
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

	fn to_regex(&self) -> Regex {
		match *self {
			SymbolicChar::Literal(ch) => Regex::Literal(ch),
			SymbolicChar::WildcardStar => Regex::KleeneClosure(Box::new(Regex::AnyChar)),
			SymbolicChar::WildcardOne => Regex::BoundedRepetition {
				min: 0,
				max: 1,
				item: Box::new(Regex::AnyChar),
			},
		}
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
	// TODO same as `Path::condense`
	fn condense(&mut self) {
		let mut i: usize = 1;
		while i < self.sub_queries.len() {
			let previous: &SubQuery = &self.sub_queries[i - 1];
			let current: &SubQuery = &self.sub_queries[i];
			if previous.is_static_text() && current.is_static_text() {
				let mut symbolic_value: Vec<SymbolicChar> = previous.symbolic_value.clone();
				symbolic_value.extend(current.symbolic_value.iter().copied());
				let string_value: String = previous.string_value.clone() + &current.string_value;
				self.sub_queries[i - 1].symbolic_value = symbolic_value;
				self.sub_queries[i - 1].string_value = string_value;
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
			if (*me_last.symbolic_value.last().unwrap() == SymbolicChar::WildcardStar)
				&& (*suffix_first.symbolic_value.first().unwrap() == SymbolicChar::WildcardStar)
			{
				me_last.symbolic_value.pop().unwrap();
				me_last.string_value.pop().unwrap();
			}
			me_last.symbolic_value.extend(suffix_first.symbolic_value.drain(..));
			me_last.string_value.extend(suffix_first.string_value.drain(..));
			self.sub_queries.extend(suffix.sub_queries.drain(1..));
		} else {
			self.sub_queries.extend(suffix.sub_queries.into_iter());
		}
	}

	fn invariants(&self) {
		let mut last_was_static_text: bool = false;
		for sub_query in self.sub_queries.iter() {
			if sub_query.is_static_text() {
				assert!(!last_was_static_text);
				last_was_static_text = true;
			} else {
				last_was_static_text = false;
			}
		}
	}
}

impl SubQuery {
	fn new_static_text(symbolic_value: Vec<SymbolicChar>) -> Self {
		let string_value: String = symbolic_value.iter().fold(String::new(), |mut accum, &ch| {
			accum.push_str(&ch.to_string());
			accum
		});
		Self {
			group: 0,
			rule_idx: None,
			fully_qualified_name: Arc::from(""),
			symbolic_value,
			string_value,
		}
	}

	fn new(group: usize, rule_info: &RuleInfo, symbolic_value: Vec<SymbolicChar>) -> Self {
		// TODO duplicated above
		let string_value: String = symbolic_value.iter().fold(String::new(), |mut accum, &ch| {
			accum.push_str(&ch.to_string());
			accum
		});
		Self {
			group,
			rule_idx: Some(rule_info.root_idx),
			fully_qualified_name: rule_info.fully_qualified_name.clone(),
			symbolic_value,
			string_value,
		}
	}

	fn is_static_text(&self) -> bool {
		self.rule_idx.is_none()
	}

	fn subsumes(&self, other: &Self) -> bool {
		if (self.group, self.rule_idx, &self.fully_qualified_name)
			!= (other.group, other.rule_idx, &other.fully_qualified_name)
		{
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

impl InterpretationPrefix {
	pub fn from_interpretations(interpretations: &[Interpretation]) -> Self {
		let mut this: Self = Self::new();
		for interpretation in interpretations.iter() {
			this.add_interpretation(&interpretation.sub_queries);
		}
		this
	}

	pub fn len(&self) -> usize {
		self.successors.len()
	}

	pub fn total_len(&self) -> usize {
		if self.successors.is_empty() {
			return 1;
		}
		self.successors
			.values()
			.fold(0, |accum, successor| accum + successor.total_len())
	}

	fn new() -> Self {
		Self {
			successors: BTreeMap::new(),
		}
	}

	fn add_interpretation(&mut self, sub_queries: &[SubQuery]) {
		let Some(first): Option<&SubQuery> = sub_queries.first() else {
			return;
		};
		self.successors
			.entry(first.clone())
			.or_insert_with(Self::new)
			.add_interpretation(&sub_queries[1..]);
	}

	pub fn print(&self, indent: usize) {
		for (sub_query, successors) in self.successors.iter() {
			println!(
				"{:\t>indent$}- {sub_query:?} ({} -> {})",
				"",
				successors.len(),
				successors.total_len()
			);
			successors.print(indent + 1);
		}
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
			fully_qualified_name: Arc::from(""),
			symbolic_value: vec![SymbolicChar::Literal('a'), SymbolicChar::WildcardStar],
			string_value: String::new(),
		};
		let b: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			fully_qualified_name: Arc::from(""),
			symbolic_value: vec![SymbolicChar::Literal('a')],
			string_value: String::new(),
		};
		let c: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			fully_qualified_name: Arc::from(""),
			symbolic_value: vec![SymbolicChar::WildcardStar, SymbolicChar::Literal('a')],
			string_value: String::new(),
		};
		let d: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			fully_qualified_name: Arc::from(""),
			symbolic_value: vec![SymbolicChar::Literal('a')],
			string_value: String::new(),
		};
		let e: SubQuery = SubQuery {
			group: 0,
			rule_idx: None,
			fully_qualified_name: Arc::from(""),
			symbolic_value: vec![SymbolicChar::WildcardStar],
			string_value: String::new(),
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

		let interpretations: Vec<Interpretation> = search.get_interpretations(&schema, "");

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

		let interpretations: Vec<Interpretation> = query.get_interpretations(&schema, name);

		interpretations
	}

	fn search_single_token(schema: &Schema, query: &str) -> Vec<Interpretation> {
		let query: SearchString = SearchString::parse(query).unwrap();

		let interpretations: Vec<Interpretation> =
			query.view(0, query.0.len()).single_token_interpretations(&schema, 0);

		interpretations
	}
}
