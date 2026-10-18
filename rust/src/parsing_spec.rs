mod encoding;
mod rule;
mod spec_file;
#[cfg(test)]
mod test;

use std::collections::BTreeMap;
use std::num::NonZero;
use std::sync::Arc;

pub use encoding::Encoding;
pub use encoding::EncodingIdx;
pub use rule::CaptureRef;
pub use rule::ResolvedCapture;
pub use rule::RootRule;
pub use rule::RuleIdx;
pub use rule::RuleInfo;

use crate::dfa::FlattenedDfa;
use crate::dfa::Tdfa;
use crate::nfa::Tnfa;
use crate::parser::Parser;
use crate::regex::AnchoredRegex;
use crate::regex::Regex;
use crate::regex::RegexError;
use crate::regex::RegexPlaceholderLookup;
use crate::search::decompose::ShapeModelCache;

#[derive(Debug, Clone)]
pub struct ParsingSpecBuilder {
	rules_by_priority: BTreeMap<i32, Vec<(Arc<str>, AnchoredRegex)>>,
	placeholders: BTreeMap<String, Regex>,
	encodings: Vec<Arc<Encoding>>,

	/// Cached (canonicalized) DFA for [`ParsingSpec::dfa_for_parsing`].
	maybe_cached_dfa: Option<FlattenedDfa>,

	delimiters: String,
}

/// A `ParsingSpec` is conceptually a list of rules and a set of delimiter characters.
///
/// [`Rule`]s may be added with a specific integer priority;
/// larger integer value means higher priority.
/// Within a priority level, rules are prioritized by insertion order.
///
#[derive(Debug, Clone)]
pub struct ParsingSpec {
	pub rules: Vec<RootRule>,
	pub placeholders: BTreeMap<String, Regex>,

	pub delimiters: String,

	pub encodings: Vec<Arc<Encoding>>,

	/// DFA used for lexing/parsing:
	/// determine which root rule matched, without executing tags for matching sub-rules.
	/// Should be constructed and executed with anchor/lookaround characters.
	pub dfa_for_parsing: FlattenedDfa,

	/// Prefilter models for log-shape search, built on demand and shared across
	/// [`Parser`]s created from this spec.
	///
	/// A spec is long-lived and typically searched repeatedly against the same set of shapes,
	/// so this turns a per-query cost into a per-shape one. See [`ShapeModelCache`].
	shape_models: ShapeModelCache,
}

impl Eq for ParsingSpec {}

impl PartialEq for ParsingSpec {
	fn eq(&self, other: &Self) -> bool {
		(
			&self.rules,
			&self.delimiters,
			&self.placeholders,
			&self.encodings,
		)
			.eq(&(
				&other.rules,
				&other.delimiters,
				&other.placeholders,
				&other.encodings,
			))
	}
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogShapeFragment {
	Text(String),
	Rule(String),
}

impl ParsingSpecBuilder {
	pub fn new() -> Self {
		Self {
			rules_by_priority: BTreeMap::new(),
			placeholders: BTreeMap::new(),
			encodings: Vec::new(),
			maybe_cached_dfa: None,
			delimiters: ParsingSpec::DEFAULT_DELIMITERS.to_owned(),
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
		self.delimiters = delimiters;
		self
	}

	/// Adds a rule with the default priority `0`.
	///
	/// Panics if `name` is empty or one of the reserved words:
	///
	/// - `"delimiters"`
	///
	pub fn add_rule<LikeString>(
		&mut self,
		name: LikeString,
		pattern: &str,
	) -> Result<&mut Self, RegexError>
	where
		LikeString: Into<Arc<str>>,
	{
		self.add_rule_with_priority(0, name, pattern)
	}

	/// Adds a rule with the given priority; larger integer value has higher priority.
	/// Within a priority level, rules are prioritized by insertion order.
	///
	/// Panics if `name` is empty or one of the reserved words:
	///
	/// - `"delimiters"`
	///
	pub fn add_rule_with_priority<LikeString>(
		&mut self,
		priority: i32,
		name: LikeString,
		pattern: &str,
	) -> Result<&mut Self, RegexError>
	where
		LikeString: Into<Arc<str>>,
	{
		let name: Arc<str> = name.into();
		assert!(!name.is_empty());
		assert_ne!(&*name, "delimiters");

		let regex: AnchoredRegex =
			AnchoredRegex::from_pattern_with_placeholders(pattern, name.clone(), self)?;
		Ok(self.add_rule_parsed(priority, name, regex))
	}

	/// Adds an already-parsed rule; see [`ParsingSpecBuilder::add_rule_with_priority`].
	pub fn add_rule_parsed<LikeString>(
		&mut self,
		priority: i32,
		name: LikeString,
		regex: AnchoredRegex,
	) -> &mut Self
	where
		LikeString: Into<Arc<str>>,
	{
		let name: Arc<str> = name.into();
		assert!(!name.is_empty());
		assert_ne!(&*name, "delimiters");

		let rules: &mut Vec<(Arc<str>, AnchoredRegex)> = self
			.rules_by_priority
			.entry(priority)
			.or_insert_with(Vec::new);

		rules.push((name, regex));

		self
	}

	/// Panics if `name` is empty or one of the reserved words:
	///
	/// - `"delimiters"`
	///
	pub fn add_placeholder<LikeString>(
		&mut self,
		name: LikeString,
		regex: Regex,
	) -> Result<&mut Self, Regex>
	where
		LikeString: Into<String>,
	{
		let name: String = name.into();
		assert!(!name.is_empty());
		assert_ne!(name, "delimiters");

		let maybe_old: Option<Regex> = self.placeholders.insert(name, regex);
		if let Some(old) = maybe_old {
			return Err(old);
		}

		Ok(self)
	}

	/// Panics if `name` is empty.
	pub fn add_encoding<LikeString>(
		&mut self,
		name: LikeString,
		regex: Regex,
	) -> Result<&mut Self, Arc<Encoding>>
	where
		LikeString: Into<String>,
	{
		let name: String = name.into();
		assert!(!name.is_empty());

		if let Some(other) = self.encodings.iter().find(|other| other.name == name) {
			return Err(other.clone());
		}

		let idx: NonZero<u16> = u16::try_from(self.encodings.len())
			.ok()
			.and_then(|n| NonZero::<u16>::MIN.checked_add(n))
			.expect("too many encodings");
		let idx: EncodingIdx = EncodingIdx::from(idx);
		let nfa: Tnfa = Tnfa::for_regex(&regex);
		self.encodings.push(Arc::new(Encoding {
			idx,
			name,
			regex,
			nfa,
		}));

		Ok(self)
	}

	pub fn set_cached_dfa(&mut self, cached: FlattenedDfa) -> &mut Self {
		// cached.initialize_ascii_cache();
		self.maybe_cached_dfa = Some(cached);
		self
	}

	pub fn build(self) -> ParsingSpec {
		let mut rules: Vec<RootRule> = Vec::new();

		let mut index: NonZero<u16> = NonZero::<u16>::MIN;
		for (priority, rules_at_priority) in self.rules_by_priority.into_iter().rev() {
			for (rule_name, rule_regex) in rules_at_priority.into_iter() {
				let rule_idx: RuleIdx = RuleIdx::new(index);

				rules.push(RootRule::new(
					rule_idx, rule_name, priority, rule_regex, &self.encodings,
				));

				index = index
					.checked_add(1)
					.expect("more than `u16::MAX` rules (not supported)");
			}
		}

		let dfa_for_parsing: FlattenedDfa = self.maybe_cached_dfa.unwrap_or_else(|| {
			debug!("[dfa] determinizing main dfa for parsing...");
			now!(t0);
			let main_dfa: Tdfa = Tdfa::for_rules(&rules, &self.delimiters, &self.encodings);
			now!(t1);
			debug!(
				"[dfa] determinizing took {} ms. canonicalizing...",
				millis!(t0, t1)
			);
			let minimized: Tdfa = main_dfa.canonicalize();
			now!(t2);
			debug!("[dfa] canonicalizing took {} ms.", millis!(t1, t2));
			now!(t3);
			let dfa_for_parsing: FlattenedDfa = minimized.flatten();
			now!(t4);
			debug!(
				"[dfa] compressing main dfa for parsing took {} ms.",
				millis!(t3, t4)
			);
			dfa_for_parsing
		});

		ParsingSpec {
			rules,
			placeholders: self.placeholders,
			delimiters: self.delimiters,
			encodings: self.encodings,
			dfa_for_parsing,
			shape_models: ShapeModelCache::new(),
		}
	}
}

impl RegexPlaceholderLookup for ParsingSpecBuilder {
	fn lookup(&mut self, name: &str) -> Option<Regex> {
		self.placeholders.get(name).cloned()
	}
}

/// A blank [`ParsingSpec`], used for default values that require a `'static` spec reference.
///
/// A `static` rather than an associated `const` because the caches are interior-mutable, so a
/// borrow of a `const` temporary (as in [`crate::log_event::LogEvent::BLANK`]) is not allowed.
pub static BLANK: ParsingSpec = ParsingSpec {
	rules: Vec::new(),
	placeholders: BTreeMap::new(),
	delimiters: String::new(),
	encodings: Vec::new(),
	dfa_for_parsing: FlattenedDfa::BLANK,
	shape_models: ShapeModelCache::new(),
};

impl ParsingSpec {
	/// The shape-model cache backing
	/// [`crate::search::SearchString::search_by_log_shapes`].
	pub fn shape_models(&self) -> &ShapeModelCache {
		&self.shape_models
	}

	/// Create a new [`Parser`] that shares this spec.
	///
	/// Parsers are cheap to create once the spec and its DFA have been built,
	/// and each caller owns a mutable parser; there may be many parsers per spec.
	pub fn create_parser(self: &Arc<Self>) -> Parser {
		Parser::new(self.clone())
	}

	pub const DEFAULT_DELIMITERS: &str = " \t\r\n:,!;%";

	/// (Sub)rules with the exact fully qualified name match.
	pub fn rules_for_name(&self, name: &str) -> Vec<(&RuleInfo, &Regex)> {
		let parts: Vec<&str> = name.split('.').collect::<Vec<_>>();
		let rule_name: &str = parts.first().copied().expect("name fragment is empty");
		let capture_names: &[&str] = &parts[1..];

		if let Some(first) = capture_names.first().copied() {
			let mut possibilities: Vec<(&RuleInfo, &Regex)> = Vec::new();
			for root_rule in self.rules.iter() {
				if &*root_rule.name != rule_name {
					continue;
				}
				root_rule.find_capture(
					root_rule.regex.root_item(),
					first,
					&capture_names[1..],
					&mut possibilities,
				);
			}
			possibilities
		} else {
			// Just a root name; no trailing parts.
			self.rules
				.iter()
				.filter(|root_rule| &*root_rule.name == rule_name)
				.map(|root_rule| (&root_rule[None], &root_rule.regex.regex))
				.collect::<Vec<_>>()
		}
	}

	/// Converts a shape string to a regular expression.
	/// References to rules (by name) should be enclosed with percent symbols as `%foo.bar%`.
	/// Returns `Err(name)` if a name is not found.
	pub fn automata_for_shape(&self, shape: &str) -> Result<Tnfa, String> {
		self.automata_for_fragments(&self.split_log_shape(shape))
	}

	/// Converts a sequence of shape fragments to an automaton.
	///
	/// Split out from [`Self::automata_for_shape`] so a caller can build a **prefix** of a shape.
	/// A query that is not anchored at the end finishes with a wildcard
	/// that consumes everything past its last literal run,
	/// so shape parts beyond that run only ever match `.*` and need never be built.
	/// Real shapes carry tens of thousands of characters of trailing static text,
	/// and one state is emitted per character,
	/// so not building it is the difference between a ~20-state intersection
	/// and a ~20 000-state one.
	///
	/// Note the elided tail is dropped outright rather than replaced by `.*`:
	/// a wildcard is a *superset* of the text it stands for,
	/// and would let a run straddle into the tail in ways the real text forbids,
	/// inventing interpretations the full shape does not have.
	///
	/// Returns `Err(name)` if a name is not found.
	pub fn automata_for_fragments(&self, fragments: &[LogShapeFragment]) -> Result<Tnfa, String> {
		let mut sequence: Vec<Tnfa> = Vec::new();

		for fragment in fragments.iter() {
			match fragment {
				LogShapeFragment::Text(text) => {
					let regex: Regex =
						Regex::Sequence(text.chars().map(Regex::Literal).collect::<Vec<_>>());
					sequence.push(Tnfa::for_regex(&regex));
				},
				LogShapeFragment::Rule(rule_name) => {
					let rules: Vec<(&RuleInfo, &Regex)> = self.rules_for_name(rule_name);

					let branches: Tnfa = rules
						.iter()
						.map(|&(info, regex)| {
							// Capture tags need the actual root rule (to resolve capture names),
							// but the fragment's accepting states must be `NIL` for `concat`.
							Tnfa::for_single_rule(info.root_idx, regex, &[])
								.with_accepting_rule(RuleIdx::NIL)
						})
						.reduce(|lhs, rhs| lhs.alternate(&rhs))
						.ok_or_else(|| rule_name.clone())?;

					sequence.push(branches);
				},
			}
		}

		Ok(sequence
			.into_iter()
			.fold(Tnfa::epsilon(), |accum, x| accum.concat(&x)))
	}

	/// Parses/splits a log shape string into fragments.
	/// Panics on invalid input.
	pub fn split_log_shape(&self, shape: &str) -> Vec<LogShapeFragment> {
		const SEPARATOR: char = '%';

		enum Kind {
			Text(String),
			Rule(String),
		}

		let mut sequence: Vec<LogShapeFragment> = Vec::new();

		let mut current: Kind = Kind::Text(String::new());

		for ch in shape.chars() {
			match current {
				Kind::Text(mut buffer) => {
					if ch == SEPARATOR {
						// Append static text.
						if !buffer.is_empty() {
							sequence.push(LogShapeFragment::Text(buffer));
						}

						// Switch to parsing rule name.
						current = Kind::Rule(String::new());
					} else {
						buffer.push(ch);
						current = Kind::Text(buffer);
					}
				},
				Kind::Rule(mut rule_name) => {
					if ch == SEPARATOR {
						// Check for `%%` escape.
						if rule_name.is_empty() {
							// Switch back to static text.
							current = Kind::Text("%".to_owned());
							continue;
						}

						// Append rule regexes.
						sequence.push(LogShapeFragment::Rule(rule_name));

						// Switch to static text.
						current = Kind::Text(String::new());
					} else {
						rule_name.push(ch);
						current = Kind::Rule(rule_name);
					}
				},
			}
		}

		let Kind::Text(buffer): Kind = current else {
			panic!("malformed log shape '{}'", shape.escape_default());
		};
		if !buffer.is_empty() {
			sequence.push(LogShapeFragment::Text(buffer));
		}

		sequence
	}
}

impl std::ops::Index<RuleIdx> for ParsingSpec {
	type Output = RootRule;

	fn index(&self, idx: RuleIdx) -> &Self::Output {
		&self.rules[usize::from(u16::from(idx)) - 1]
	}
}

impl ParsingSpec {
	/// The root rule for `idx`, or `None` if `idx` is out of range
	/// (e.g. [`RuleIdx::NIL`], used for search/encoding automata).
	pub fn maybe_root_rule(&self, idx: RuleIdx) -> Option<&RootRule> {
		let i: usize = usize::from(u16::from(idx)).checked_sub(1)?;
		self.rules.get(i)
	}

	/// Resolve a [`CaptureRef`] (whose tag carried its root rule) against this spec.
	pub fn resolve_capture(&self, capture: CaptureRef) -> Option<(&RuleInfo, Arc<str>)> {
		let rule: &RootRule = self.maybe_root_rule(capture.rule_idx)?;
		let info: &RuleInfo = rule.rule_info.get(usize::from(capture.capture_id))?;
		Some((info, info.fully_qualified_name.clone()))
	}
}

impl std::ops::Index<EncodingIdx> for ParsingSpec {
	type Output = Arc<Encoding>;

	fn index(&self, idx: EncodingIdx) -> &Self::Output {
		&self.encodings[usize::from(u16::from(idx) - 1)]
	}
}
