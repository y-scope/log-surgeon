use crate::dfa::Tdfa;
use crate::log_event::Capture;
use crate::regex::IntoRegex;
use crate::regex::Regex;
use crate::regex::RegexCapture;
use std::collections::BTreeMap;
use std::num::NonZero;

#[derive(Debug, Clone)]
pub struct SchemaBuilder {
	rules_by_priority: BTreeMap<i32, Vec<(String, Regex)>>,
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
	pub rules: Vec<Rule>,
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
	pub capture_info: Vec<RegexCapture>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct RuleIdx {
	/// The original priority level given by the user.
	pub priority: i32,
	/// Insertion order of the rule at the given priority level.
	pub position: u16,
	/// Index in the schema.
	pub index: NonZero<u16>,
}

#[derive(Debug)]
pub enum VariableOrCaptures<T> {
	Variable(Vec<(RuleIdx, T)>),
	Captures(Vec<(RuleIdx, RegexCapture, T)>),
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
		assert_ne!(name, "delimiters");

		let regex: Regex = regex.into()?;

		let rules: &mut Vec<(String, Regex)> = self.rules_by_priority.entry(priority).or_insert_with(Vec::new);

		rules.push((name, regex));

		Ok(())
	}

	pub fn build(self) -> Schema {
		let mut rules: Vec<Rule> = Vec::new();
		for (priority, rules_at_priority) in self.rules_by_priority.into_iter().rev() {
			for (position, (name, regex)) in rules_at_priority.into_iter().enumerate() {
				let Some(index): Option<NonZero<u16>> = u16::try_from(rules.len())
					.ok()
					.and_then(|index| NonZero::<u16>::MIN.checked_add(index))
				else {
					panic!("more than u16::MAX rules (not supported)");
				};

				// Don't need to check; necessarily `position <= index`.
				let position: u16 = position as u16;

				rules.push(Rule::new(
					RuleIdx {
						priority,
						position,
						index,
					},
					name,
					regex,
				));
			}
		}
		Schema {
			rules,
			delimiters: self.delimiters,
			anchor_ch: self.anchor_ch,
		}
	}
}

impl Schema {
	pub const DEFAULT_DELIMITERS: &str = " \t\r\n:,!;%";

	pub fn build_dfa(&self) -> Tdfa {
		Tdfa::for_rules(&self.rules, self.delimiters.clone())
	}

	pub fn names(&self, capture: &Capture) -> (&'_ str, &'_ str) {
		let rule: &Rule = &self[capture.rule_idx];
		let capture_id: usize = capture.capture_id.map_or(0, NonZero::get) as usize;
		(&rule.name, &rule.capture_info[capture_id].name)
	}

	pub fn regexes_for_name(&self, name: &str) -> Option<VariableOrCaptures<Regex>> {
		let parts: Vec<&str> = name.split('.').collect::<Vec<_>>();
		let rule_name: &str = parts.first().copied()?;
		let capture_names: &[&str] = &parts[1..];

		if let Some(first) = capture_names.first().copied() {
			let mut possibilities: Vec<(RuleIdx, RegexCapture, Regex)> = Vec::new();
			for rule in self.rules.iter() {
				if rule.name != rule_name {
					continue;
				}
				Self::find_capture(&rule.regex, first, &capture_names[1..], &mut |info, regex| {
					possibilities.push((rule.idx, info.clone(), regex.clone()));
				});
			}
			Some(VariableOrCaptures::Captures(possibilities))
		} else {
			Some(VariableOrCaptures::Variable(
				self.rules
					.iter()
					.filter(|rule| rule.name == rule_name)
					.map(|rule| (rule.idx, rule.regex.clone()))
					.collect::<Vec<_>>(),
			))
		}
	}

	fn find_capture<F>(regex: &Regex, first: &str, rest: &[&str], func: &mut F)
	where
		F: FnMut(&RegexCapture, &Regex),
	{
		match regex {
			Regex::Anchor(_) | Regex::AnyChar | Regex::Literal(..) | Regex::Group { .. } => (),
			Regex::Capture { info, item } => {
				if info.name == first {
					if let Some(first) = rest.first().copied() {
						Self::find_capture(item, first, &rest[1..], func);
					} else {
						func(info, item);
					}
				}
			},
			Regex::KleeneClosure(item) | Regex::BoundedRepetition { item, .. } => {
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
	type Output = Rule;

	fn index(&self, idx: RuleIdx) -> &Self::Output {
		&self.rules[usize::from(idx.index.get() - 1)]
		// let Some(rules): Option<&Vec<Rule>> = self.rules_by_priority.get(&idx.priority) else {
		// 	panic!("schema rule idx out of range: no rules with priority {}", idx.priority);
		// };
		// let Some(rule): Option<&Rule> = rules.get(idx.position as usize) else {
		// 	panic!(
		// 		"schema rule idx out of range: position {} for {} rules at priority {}",
		// 		idx.position,
		// 		rules.len(),
		// 		idx.priority,
		// 	);
		// };
		// rule
	}
}

impl Rule {
	pub fn new(idx: RuleIdx, name: String, regex: Regex) -> Self {
		let mut capture_info: Vec<RegexCapture> = vec![RegexCapture::NULL; 1 + regex.count_captures()];
		regex.populate_capture_info(&mut capture_info);

		Self {
			idx,
			name,
			regex,
			capture_info,
		}
	}

	pub fn capture_info(&self, i: Option<NonZero<u32>>) -> &RegexCapture {
		&self.capture_info[i.map_or(0, NonZero::get) as usize]
	}
}

impl RuleIdx {
	pub fn as_index(&self) -> usize {
		usize::from(self.index.get() - 1)
	}
}
