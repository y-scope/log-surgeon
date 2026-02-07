use crate::regex::Regex;

#[derive(Debug)]
pub struct Schema {
	rules: Vec<Rule>,
	delimiters: String,
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
			delimiters: Self::DEFAULT_DELIMITERS.to_owned(),
		}
	}

	pub fn set_delimiters(&mut self, delimiters: &str) {
		self.rules[0].regex = Self::pattern_for_delimiters(delimiters);
		self.delimiters = delimiters.to_owned();
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

	pub fn delimiters(&self) -> &str {
		&self.delimiters
	}

	pub fn pattern_for_delimiters(delimiters: &str) -> Regex {
		let mut escaped: String = String::new();
		for ch in delimiters.chars() {
			escaped += &format!("\\u{{{:02x}}}", u32::from(ch));
		}
		let pattern: String = format!("[{escaped}]");
		Regex::from_pattern(&pattern).unwrap()
	}
}
