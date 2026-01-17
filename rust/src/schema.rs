use crate::regex::Regex;

#[derive(Debug)]
pub struct Schema {
	rules: Vec<Rule>,
}

#[derive(Debug)]
pub struct Rule {
	pub idx: usize,
	pub name: String,
	pub regex: Regex,
}

impl Schema {
	pub const DEFAULT_DELIMITERS: &str = r" \t\r\n:,!;%";

	pub fn new() -> Self {
		Self {
			rules: vec![Rule {
				idx: 0,
				name: "static".to_owned(),
				regex: Self::pattern_for_delimiters(Self::DEFAULT_DELIMITERS),
			}],
		}
	}

	pub fn set_delimiters(&mut self, delimiters: &str) {
		self.rules[0].regex = Self::pattern_for_delimiters(delimiters);
	}

	pub fn add_rule<LikeString>(&mut self, name: LikeString, regex: Regex)
	where
		LikeString: Into<String>,
	{
		let idx: usize = self.rules.len();
		self.rules.push(Rule {
			idx,
			name: name.into(),
			regex,
		});
	}

	pub fn rules(&self) -> &[Rule] {
		&self.rules
	}

	// TODO use generalized escapes
	fn pattern_for_delimiters(delimiters: &str) -> Regex {
		let pattern: String = format!("[^{delimiters}]+|delimiters");
		Regex::from_pattern(&pattern).unwrap()
	}
}
