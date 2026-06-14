mod rule;
mod schema_file;

use std::collections::BTreeMap;
use std::num::NonZero;
use std::sync::Arc;

pub use rule::RootRule;
pub use rule::RuleIdx;
pub use rule::RuleInfo;
pub use rule::SubRule;

use crate::dfa::CompressedDfa;
use crate::dfa::Tdfa;
use crate::nfa::Tnfa;
use crate::regex::AnchoredRegex;
use crate::regex::IntoRegex;
use crate::regex::Regex;
use crate::regex::RegexPlaceholderLookup;

#[derive(Debug, Clone)]
pub struct SchemaBuilder {
	rules_by_priority: BTreeMap<i32, Vec<(Arc<str>, AnchoredRegex)>>,
	placeholders: BTreeMap<String, Regex>,
	encodings: Vec<(String, Regex)>,

	maybe_cached_dfa: Option<Tdfa>,

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
	pub placeholders: BTreeMap<String, Regex>,

	pub delimiters: String,

	/// TDFA used for lexing/parsing.
	pub main_dfa: Tdfa,
	/// TNFA used for search.
	pub main_nfa: Tnfa,
	/// TODO
	pub optimized_dfa: CompressedDfa,

	pub encodings: Vec<Vec<String>>,

	/// Derived from `delimiters`;
	/// used to insert "phantom characters" to implement start/end anchors.
	pub anchor_ch: char,
	pub ascii_delimiters: [bool; 0x80],
	pub non_ascii_delimiters: String,
}

impl Eq for Schema {}

impl PartialEq for Schema {
	fn eq(&self, other: &Self) -> bool {
		(&self.rules, &self.delimiters, &self.encodings).eq(&(&other.rules, &other.delimiters, &other.encodings))
	}
}

impl SchemaBuilder {
	pub fn new() -> Self {
		Self {
			rules_by_priority: BTreeMap::new(),
			placeholders: BTreeMap::new(),
			encodings: Vec::new(),
			maybe_cached_dfa: None,
			delimiters: Schema::DEFAULT_DELIMITERS.to_owned(),
			anchor_ch: '\n',
		}
	}

	/// Panics if `delimiters` is empty.
	pub fn set_delimiters<LikeString>(&mut self, delimiters: LikeString) -> &mut Self
	where
		LikeString: Into<String>,
	{
		let mut delimiters: String = delimiters.into();
		assert!(!delimiters.is_empty());
		if !delimiters.contains('\n') {
			delimiters.push('\n');
		}
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

	pub fn add_placeholder(&mut self, name: String, regex: Regex) -> Result<&mut Self, Regex> {
		assert!(!name.is_empty());
		assert_ne!(name, "delimiters");

		let maybe_old: Option<Regex> = self.placeholders.insert(name, regex);
		if let Some(old) = maybe_old {
			return Err(old);
		}

		Ok(self)
	}

	pub fn add_encoding<LikeString>(&mut self, name: LikeString, regex: Regex) -> Result<&mut Self, Regex>
	where
		LikeString: Into<String>,
	{
		let name: String = name.into();
		assert!(!name.is_empty());

		if let Some((_, other_regex)) = self.encodings.iter().find(|(other_name, _)| *other_name == name) {
			return Err(other_regex.clone());
		}

		self.encodings.push((name, regex));

		Ok(self)
	}

	pub fn set_cached_dfa(&mut self, mut cached: Tdfa) -> &mut Self {
		cached.initialize_ascii_cache();
		self.maybe_cached_dfa = Some(cached);
		self
	}

	pub fn build(self) -> Schema {
		let mut rules: Vec<RootRule> = Vec::new();

		let mut encodings: Vec<Vec<String>> = vec![Vec::new()];
		let mut encodings_lookup: BTreeMap<Vec<String>, usize> = BTreeMap::from([(Vec::new(), 0)]);

		let mut index: NonZero<u16> = NonZero::<u16>::MIN;
		for (priority, rules_at_priority) in self.rules_by_priority.into_iter().rev() {
			for (name, regex) in rules_at_priority.into_iter() {
				let rule_idx: RuleIdx = RuleIdx::new(index);

				rules.push(RootRule::new(rule_idx, name, priority, regex, |regex| {
					let rule_nfa: Tnfa = Tnfa::for_single_rule(rule_idx, regex);

					let mut possible_encodings: Vec<String> = Vec::new();
					for (encoding_name, encoding_regex) in self.encodings.iter() {
						let encoding_nfa: Tnfa = Tnfa::for_single_rule(RuleIdx::NIL, encoding_regex);
						let intersection: Tnfa = rule_nfa.intersect::<false>(&encoding_nfa);
						if intersection.can_accept() {
							possible_encodings.push(encoding_name.clone());
						}
					}
					let encoding_idx: usize =
						*encodings_lookup
							.entry(possible_encodings)
							.or_insert_with_key(|possible_encodings| {
								let n: usize = encodings.len();
								encodings.push(possible_encodings.clone());
								n
							});

					let Some(encoding_idx): Option<u16> = u16::try_from(encoding_idx).ok() else {
						panic!("more than u16::MAX encodings (not supported)");
					};

					let encoding_idx: Option<NonZero<u16>> = NonZero::new(encoding_idx);
					encoding_idx
				}));

				let Some(next): Option<NonZero<u16>> = index.checked_add(1) else {
					panic!("more than u16::MAX rules (not supported)");
				};
				index = next;
			}
		}

		let main_nfa: Tnfa = Tnfa::for_rules::<true, _>(rules.iter(), &self.delimiters);

		let main_dfa: Tdfa = self.maybe_cached_dfa.unwrap_or_else(|| {
			now!(t0);
			let main_dfa: Tdfa = Tdfa::for_rules(rules.iter(), self.delimiters.clone());
			now!(t1);
			let minimized: Tdfa = main_dfa.minimize();
			now!(t2);
			debug!(
				"[minimizing dfa] took ({:?}, {:?})",
				t1.duration_since(t0),
				t2.duration_since(t1)
			);
			minimized
		});

		let optimized_dfa: CompressedDfa = main_dfa.compress();

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
			placeholders: self.placeholders,
			delimiters: self.delimiters,
			main_nfa,
			main_dfa,
			optimized_dfa,
			encodings,
			anchor_ch: self.anchor_ch,
			ascii_delimiters,
			non_ascii_delimiters,
		}
	}
}

impl RegexPlaceholderLookup for SchemaBuilder {
	fn lookup(&mut self, name: &str) -> Option<Regex> {
		self.placeholders.get(name).cloned()
	}
}

impl Schema {
	pub const DEFAULT_DELIMITERS: &str = " \t\r\n:,!;%";

	pub fn build_dfa(&self) -> Tdfa {
		Tdfa::for_rules(&self.rules, self.delimiters.clone())
	}

	pub fn rules_for_name(&self, name: &str) -> Option<Vec<(&RuleInfo, &Regex)>> {
		let parts: Vec<&str> = name.split('.').collect::<Vec<_>>();
		let rule_name: &str = parts.first().copied()?;
		let capture_names: &[&str] = &parts[1..];

		if let Some(first) = capture_names.first().copied() {
			let mut possibilities: Vec<(&RuleInfo, &Regex)> = Vec::new();
			for root_rule in self.rules.iter() {
				if &*root_rule.name != rule_name {
					continue;
				}
				Self::find_capture(
					root_rule,
					&root_rule.regex.inner,
					first,
					&capture_names[1..],
					&mut possibilities,
				);
			}
			Some(possibilities)
		} else {
			Some(
				self.rules
					.iter()
					.filter(|root_rule| &*root_rule.name == rule_name)
					.map(|root_rule| (&root_rule[None], &root_rule.regex.inner))
					.collect::<Vec<_>>(),
			)
		}
	}

	fn find_capture<'a>(
		root_rule: &'a RootRule,
		regex: &'a Regex,
		first: &str,
		rest: &[&str],
		collect: &mut Vec<(&'a RuleInfo, &'a Regex)>,
	) {
		match regex {
			Regex::AnyChar | Regex::Literal(..) | Regex::BracketedRanges { .. } => (),
			Regex::Capture(sub_rule) => {
				if sub_rule.name == first {
					if let Some(first) = rest.first().copied() {
						Self::find_capture(root_rule, &sub_rule.regex, first, &rest[1..], collect);
					} else {
						collect.push((&root_rule[Some(sub_rule.id)], regex));
					}
				}
			},
			Regex::KleeneClosure(item)
			| Regex::KleenePlus(item)
			| Regex::BoundedRepetition { item, .. }
			| Regex::Placeholder { item, .. } => {
				Self::find_capture(root_rule, item, first, rest, collect);
			},
			Regex::Sequence(items) | Regex::Alternation(items) => {
				for item in items.iter() {
					Self::find_capture(root_rule, item, first, rest, collect);
				}
			},
		}
	}
}

impl std::ops::Index<RuleIdx> for Schema {
	type Output = RootRule;

	fn index(&self, idx: RuleIdx) -> &Self::Output {
		&self.rules[usize::from(u16::from(idx)) - 1]
	}
}

impl RootRule {
	pub fn new<F>(idx: RuleIdx, name: Arc<str>, priority: i32, regex: AnchoredRegex, mut lookup_encoding: F) -> Self
	where
		F: FnMut(&Regex) -> Option<NonZero<u16>>,
	{
		let mut rule_info: Vec<RuleInfo> = Vec::with_capacity(1 + regex.inner.count_captures());
		rule_info.push(RuleInfo {
			root_idx: idx,
			root_name: name.clone(),
			maybe_sub_rule: None,
			fully_qualified_name: name.clone(),
			encoding_idx: lookup_encoding(&regex.inner),
		});

		let mut stack: Vec<&Regex> = vec![&regex.inner];
		while let Some(regex) = stack.pop() {
			match regex {
				Regex::AnyChar | Regex::Literal(..) | Regex::BracketedRanges { .. } => (),
				Regex::Capture(sub_rule) => {
					let i: usize = sub_rule.id_as_usize();
					assert_eq!(rule_info.len(), i);
					rule_info.push(RuleInfo {
						root_idx: idx,
						root_name: name.clone(),
						maybe_sub_rule: Some(sub_rule.clone()),
						fully_qualified_name: Arc::from(format!("{}{}", name, sub_rule.qualified_name)),
						encoding_idx: if sub_rule.is_leaf() {
							lookup_encoding(&sub_rule.regex)
						} else {
							None
						},
					});
					stack.push(&sub_rule.regex);
				},
				Regex::KleeneClosure(item)
				| Regex::KleenePlus(item)
				| Regex::BoundedRepetition { item, .. }
				| Regex::Placeholder { item, .. } => {
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

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn number_encoding() {
		let mut builder: SchemaBuilder = SchemaBuilder::new();
		builder
			.add_rule("has_number", r"\w*\d\w*")
			.unwrap()
			.add_rule("ip_address", r"\d(\.\d){3}")
			.unwrap()
			.add_encoding("int", Regex::from_pattern(r"\d+").unwrap().inner)
			.unwrap();

		let schema: Schema = builder.build();

		assert_eq!(schema.rules[0][None].encoding_idx, Some(NonZero::<u16>::MIN));
		assert_eq!(schema.rules[1][None].encoding_idx, None);
	}
}
