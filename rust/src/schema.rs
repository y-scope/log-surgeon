use crate::dfa::Tdfa;
use crate::regex::IntoRegex;
use crate::regex::Regex;

#[derive(Debug, Clone)]
pub struct Schema {
	pub rules: Vec<Rule>,
	pub delimiters: String,
	pub anchor_ch: char,
}

#[derive(Debug, Clone)]
pub struct Rule {
	pub idx: usize,
	pub name: String,
	pub regex: Regex,
}

impl Schema {
	pub const DEFAULT_DELIMITERS: &str = " \t\r\n:,!;%";

	pub fn new() -> Self {
		// TODO special first rule
		Self {
			rules: vec![Rule {
				idx: 0,
				name: "newline".to_owned(),
				regex: Regex::Sequence(vec![Regex::AnyChar, Regex::Literal('\n')]),
			}],
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
		let name: String = name.into();
		assert!(!name.is_empty());
		assert_ne!(name, "newline");
		assert_ne!(name, "delimiters");

		let regex: Regex = regex.into()?;

		let idx: usize = self.rules.len();
		self.rules.push(Rule { idx, name, regex });
		Ok(())
	}
}

impl Schema {
	pub fn build_dfa(&self) -> Tdfa {
		Tdfa::for_rules(&self.rules, self.delimiters.clone())
	}
}

impl Schema {
	pub fn rules(&self) -> &[Rule] {
		&self.rules
	}
}
