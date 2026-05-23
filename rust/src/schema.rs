mod schema_file;

use std::collections::BTreeMap;
use std::num::NonZero;
use std::sync::Arc;

use crate::dfa::Tdfa;
use crate::nfa::Tnfa;
use crate::regex::AnchoredRegex;
use crate::regex::IntoRegex;
use crate::regex::Regex;
use crate::regex::SubRule;

#[derive(Debug, Clone)]
pub struct SchemaBuilder {
	rules_by_priority: BTreeMap<i32, Vec<(Arc<str>, AnchoredRegex)>>,
	delimiters: String,
	anchor_ch: char,
}

/// A `Schema` is conceptually a list of rules and a set of delimiter characters.
///
/// [`Rule`]s may be added with a specific integer priority;
/// larger integer value means higher priority.
/// Within a priority level, rules are prioritized by insertion order.
///
#[derive(Debug, Clone)]
pub struct Schema {
	pub rules: Vec<RootRule>,

	pub delimiters: String,

	/// TDFA used for lexing/parsing.
	pub main_dfa: Tdfa,
	/// TNFA used for search.
	pub main_nfa: Tnfa,

	/// Derived from `delimiters`;
	/// used to insert "phantom characters" for start/end anchors.
	pub anchor_ch: char,
	pub ascii_delimiters: [bool; 0x80],
	pub non_ascii_delimiters: String,
}

impl Eq for Schema {}

impl PartialEq for Schema {
	fn eq(&self, other: &Self) -> bool {
		(&self.rules, &self.delimiters).eq(&(&other.rules, &other.delimiters))
	}
}

#[derive(Debug, Clone)]
pub struct RootRule {
	pub idx: RuleIdx,
	pub name: Arc<str>,
	/// Priority level given by the user.
	pub priority: i32,

	pub regex: AnchoredRegex,
	pub rule_info: Vec<RuleInfo>,

	pub dfa: Tdfa,
}

impl Eq for RootRule {}

impl PartialEq for RootRule {
	fn eq(&self, other: &Self) -> bool {
		(self.idx, &self.name, self.priority, &self.regex)
			.cmp(&(other.idx, &other.name, other.priority, &other.regex))
			.is_eq()
	}
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuleInfo {
	pub root_idx: RuleIdx,
	pub root_name: Arc<str>,

	/// If this is not a root rule, additional sub-rule info.
	pub maybe_sub_rule: Option<SubRule>,

	pub fully_qualified_name: Arc<str>,
}

/// Index in the schema, offset by 1.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RuleIdx(NonZero<u16>);

#[derive(Debug)]
pub enum VariableOrCaptures<T> {
	Variable(Vec<(RuleIdx, T)>),
	Captures(Vec<(RuleIdx, SubRule, T)>),
}

impl SchemaBuilder {
	pub fn new() -> Self {
		Self {
			rules_by_priority: BTreeMap::new(),
			delimiters: Schema::DEFAULT_DELIMITERS.to_owned(),
			anchor_ch: '\n',
		}
	}

	/// Panics if `delimiters` is empty.
	pub fn set_delimiters<LikeString>(&mut self, delimiters: LikeString) -> &mut Self
	where
		LikeString: Into<String>,
	{
		let delimiters: String = delimiters.into();
		assert!(!delimiters.is_empty());
		self.anchor_ch = delimiters.chars().next().unwrap();
		self.delimiters = delimiters;
		self
	}

	/// Adds a rule with the default priority `0`.
	///
	/// Panics if `name` is empty or one of the reserved words:
	///
	/// - `"delimiters"`
	///
	pub fn add_rule<LikeString, RegexOrPattern>(
		&mut self,
		name: LikeString,
		regex: RegexOrPattern,
	) -> Result<&mut Self, RegexOrPattern::Error>
	where
		LikeString: Into<Arc<str>>,
		RegexOrPattern: IntoRegex,
	{
		self.add_rule_with_priority(0, name, regex)
	}

	/// Adds a rule with the given priority; larger integer value has higher priority.
	/// Within a priority level, rules are prioritized by insertion order.
	///
	/// Panics if `name` is empty or one of the reserved words:
	///
	/// - `"delimiters"`
	///
	pub fn add_rule_with_priority<LikeString, RegexOrPattern>(
		&mut self,
		priority: i32,
		name: LikeString,
		regex: RegexOrPattern,
	) -> Result<&mut Self, RegexOrPattern::Error>
	where
		LikeString: Into<Arc<str>>,
		RegexOrPattern: IntoRegex,
	{
		let name: Arc<str> = name.into();
		assert!(!name.is_empty());
		assert_ne!(&*name, "delimiters");

		let regex: AnchoredRegex = regex.into()?;

		let rules: &mut Vec<(Arc<str>, AnchoredRegex)> =
			self.rules_by_priority.entry(priority).or_insert_with(Vec::new);

		rules.push((name, regex));

		Ok(self)
	}

	pub fn build(self) -> Schema {
		let mut rules: Vec<RootRule> = Vec::new();
		let mut index: NonZero<u16> = NonZero::<u16>::MIN;
		for (priority, rules_at_priority) in self.rules_by_priority.into_iter().rev() {
			for (name, regex) in rules_at_priority.into_iter() {
				rules.push(RootRule::new(RuleIdx(index), name, priority, regex));

				let Some(next): Option<NonZero<u16>> = index.checked_add(1) else {
					panic!("more than u16::MAX rules (not supported)");
				};
				index = next;
			}
		}

		let main_nfa: Tnfa = Tnfa::for_rules::<true, _>(rules.iter(), self.delimiters.clone());
		// let main_dfa: Tdfa = Tdfa::determinization(&main_nfa);
		let main_dfa: Tdfa = Tdfa::for_rules(rules.iter(), self.delimiters.clone());

		let mut ascii_delimiters: [bool; 0x80] = [false; 0x80];
		let mut non_ascii_delimiters: String = String::new();
		for ch in self.delimiters.chars() {
			if let Ok(i) = u8::try_from(ch)
				&& let Some(entry) = ascii_delimiters.get_mut(usize::from(i))
			{
				*entry = true;
			} else {
				non_ascii_delimiters.push(ch);
			}
		}

		Schema {
			rules,
			delimiters: self.delimiters,
			main_nfa,
			main_dfa,
			anchor_ch: self.anchor_ch,
			ascii_delimiters,
			non_ascii_delimiters,
		}
	}
}

impl Schema {
	pub const DEFAULT_DELIMITERS: &str = " \t\r\n:,!;%";

	pub fn build_dfa(&self) -> Tdfa {
		Tdfa::for_rules(&self.rules, self.delimiters.clone())
	}

	pub fn regexes_for_name(&self, name: &str) -> Option<VariableOrCaptures<Regex>> {
		let parts: Vec<&str> = name.split('.').collect::<Vec<_>>();
		let rule_name: &str = parts.first().copied()?;
		let capture_names: &[&str] = &parts[1..];

		if let Some(first) = capture_names.first().copied() {
			let mut possibilities: Vec<(RuleIdx, SubRule, Regex)> = Vec::new();
			for rule in self.rules.iter() {
				if &*rule.name != rule_name {
					continue;
				}
				Self::find_capture(&rule.regex.inner, first, &capture_names[1..], &mut |info, regex| {
					possibilities.push((rule.idx, info.clone(), regex.clone()));
				});
			}
			Some(VariableOrCaptures::Captures(possibilities))
		} else {
			Some(VariableOrCaptures::Variable(
				self.rules
					.iter()
					.filter(|rule| &*rule.name == rule_name)
					.map(|rule| (rule.idx, rule.regex.inner.clone()))
					.collect::<Vec<_>>(),
			))
		}
	}

	fn find_capture<F>(regex: &Regex, first: &str, rest: &[&str], func: &mut F)
	where
		F: FnMut(&SubRule, &Regex),
	{
		match regex {
			Regex::AnyChar | Regex::Literal(..) | Regex::Group { .. } => (),
			Regex::Capture(sub_rule) => {
				if sub_rule.name == first {
					if let Some(first) = rest.first().copied() {
						Self::find_capture(&sub_rule.regex, first, &rest[1..], func);
					} else {
						func(sub_rule, &sub_rule.regex);
					}
				}
			},
			Regex::KleeneClosure(item) | Regex::KleenePlus(item) | Regex::BoundedRepetition { item, .. } => {
				Self::find_capture(item, first, rest, func);
			},
			Regex::Sequence(items) | Regex::Alternation(items) => {
				for item in items.iter() {
					Self::find_capture(item, first, rest, func);
				}
			},
		}
	}
}

impl std::ops::Index<RuleIdx> for Schema {
	type Output = RootRule;

	fn index(&self, idx: RuleIdx) -> &Self::Output {
		&self.rules[idx.as_usize()]
	}
}

impl RootRule {
	pub fn new(idx: RuleIdx, name: Arc<str>, priority: i32, regex: AnchoredRegex) -> Self {
		let mut rule_info: Vec<RuleInfo> = Vec::with_capacity(1 + regex.inner.count_captures());
		rule_info.push(RuleInfo {
			root_idx: idx,
			root_name: name.clone(),
			maybe_sub_rule: None,
			fully_qualified_name: name.clone(),
		});

		let mut stack: Vec<&Regex> = vec![&regex.inner];
		while let Some(regex) = stack.pop() {
			match regex {
				Regex::AnyChar | Regex::Literal(..) | Regex::Group { .. } => (),
				Regex::Capture(sub_rule) => {
					let i: usize = sub_rule.id_as_usize();
					assert_eq!(rule_info.len(), i);
					rule_info.push(RuleInfo {
						root_idx: idx,
						root_name: name.clone(),
						maybe_sub_rule: Some(sub_rule.clone()),
						fully_qualified_name: Arc::from(format!("{}{}", name, sub_rule.qualified_name)),
					});
					stack.push(&sub_rule.regex);
				},
				Regex::KleeneClosure(item) | Regex::KleenePlus(item) | Regex::BoundedRepetition { item, .. } => {
					stack.push(item);
				},
				Regex::Sequence(items) | Regex::Alternation(items) => {
					// Push on to stack in reverse to mirror DFS.
					for sub_item in items.iter().rev() {
						stack.push(sub_item);
					}
				},
			}
		}

		let dfa: Tdfa = Tdfa::for_single_rule(idx, &regex.inner);

		Self {
			idx,
			name,
			priority,
			regex,
			rule_info,
			dfa,
		}
	}
}

impl std::ops::Index<Option<NonZero<u16>>> for RootRule {
	type Output = RuleInfo;

	fn index(&self, i: Option<NonZero<u16>>) -> &Self::Output {
		let i: usize = usize::from(i.map_or(0, NonZero::get));
		&self.rule_info[i]
	}
}

impl RuleIdx {
	/// cbindgen:ignore
	pub const NIL: Self = Self(NonZero::<u16>::MAX);

	pub fn get(&self) -> NonZero<u16> {
		self.0
	}

	pub fn as_usize(&self) -> usize {
		usize::from(self.0.get() - 1)
	}
}

impl std::fmt::Display for RuleIdx {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(fmt)
	}
}

impl RuleInfo {
	pub fn sub_rule_name(&self) -> &str {
		if let Some(sub_rule) = &self.maybe_sub_rule {
			&sub_rule.name
		} else {
			""
		}
	}
}
