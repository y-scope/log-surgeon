use crate::dfa::Tdfa;
use crate::regex::IntoRegex;
use crate::regex::Regex;
use std::collections::BTreeMap;

/// A `Schema` is conceptually a list of rules and a set of delimiter characters.
///
/// [`Rule`]s may be added with a specific integer priority;
/// larger integer value means higher priority.
/// Within a priority level, rules are prioritized by insertion order.
///
/// Before automata construction, rules are "flattened", ordered by priority (highest first).
/// A special `0`th rule internally represents a "newline" token.
/// Rules are "ID"ed by their index in this flattened priority list.
///
#[derive(Debug, Clone)]
pub struct Schema {
	rules_by_priority: BTreeMap<i32, Vec<(String, Regex)>>,
	rules_flattened: Vec<Rule>,
	pub delimiters: String,
	/// Derived from `delimiters`;
	/// used to insert "phantom characters" for start/end anchors.
	pub anchor_ch: char,
}

#[derive(Debug, Clone)]
pub struct Rule {
	pub idx: RuleIdx,
	pub name: String,
	pub regex: Regex,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuleIdx {
	/// Index in the [`Schema`]'s flattened priority list.
	pub index: u32,
	/// The original priority level given by the user.
	pub priority: i32,
}

impl Schema {
	pub const DEFAULT_DELIMITERS: &str = " \t\r\n:,!;%";

	pub fn new() -> Self {
		Self {
			rules_by_priority: BTreeMap::new(),
			rules_flattened: Vec::new(),
			delimiters: Self::DEFAULT_DELIMITERS.to_owned(),
			anchor_ch: ' ',
		}
	}

	/// Panics if `delimiters` is empty.
	pub fn set_delimiters<LikeString>(&mut self, delimiters: LikeString)
	where
		LikeString: Into<String>,
	{
		let delimiters: String = delimiters.into();
		assert!(!delimiters.is_empty());
		self.anchor_ch = delimiters.chars().next().unwrap();
		self.delimiters = delimiters;
	}

	/// Adds a rule with the default priority `0`.
	///
	/// Panics if `name` is empty or one of the reserved words:
	///
	/// - `"newline"`
	/// - `"delimiters"`
	///
	pub fn add_rule<LikeString, RegexOrPattern>(
		&mut self,
		name: LikeString,
		regex: RegexOrPattern,
	) -> Result<(), RegexOrPattern::Error>
	where
		LikeString: Into<String>,
		RegexOrPattern: IntoRegex,
	{
		self.add_rule_with_priority(0, name, regex)
	}

	/// Adds a rule with the given priority; larger integer value has higher priority.
	/// Within a priority level, rules are prioritized by insertion order.
	///
	/// Panics if `name` is empty or one of the reserved words:
	///
	/// - `"newline"`
	/// - `"delimiters"`
	///
	pub fn add_rule_with_priority<LikeString, RegexOrPattern>(
		&mut self,
		priority: i32,
		name: LikeString,
		regex: RegexOrPattern,
	) -> Result<(), RegexOrPattern::Error>
	where
		LikeString: Into<String>,
		RegexOrPattern: IntoRegex,
	{
		let name: String = name.into();
		assert!(!name.is_empty());
		assert_ne!(name, "newline");
		assert_ne!(name, "delimiters");

		let regex: Regex = regex.into()?;

		let rules: &mut Vec<(String, Regex)> = self.rules_by_priority.entry(priority).or_insert_with(Vec::new);

		rules.push((name, regex));

		Ok(())
	}
}

impl Schema {
	pub fn build_dfa(&mut self) -> Tdfa {
		self.flatten_rules();
		Tdfa::for_rules(&self.rules_flattened, self.delimiters.clone())
	}

	fn flatten_rules(&mut self) {
		self.rules_flattened.clear();
		self.rules_flattened.push(Rule {
			idx: RuleIdx {
				index: 0,
				priority: i32::MAX,
			},
			name: "newline".to_owned(),
			regex: Regex::Sequence(vec![Regex::AnyChar, Regex::Literal('\n')]),
		});
		for (&priority, rules) in self.rules_by_priority.iter().rev() {
			for (name, regex) in rules.iter() {
				// This can only fail on 32-bit platforms; i.e. (as far as rustc supports, 16-bit ones).
				let index: u32 = u32::try_from(self.rules_flattened.len()).unwrap();
				self.rules_flattened.push(Rule {
					idx: RuleIdx { index, priority },
					name: name.clone(),
					regex: regex.clone(),
				});
			}
		}
	}
}

impl Schema {
	pub fn rules(&self) -> &[Rule] {
		&self.rules_flattened
	}
}

impl RuleIdx {
	pub fn index(&self) -> usize {
		// This can only fail on 32-bit platforms; i.e. (as far as rustc supports, 16-bit ones).
		usize::try_from(self.index).unwrap()
	}
}
