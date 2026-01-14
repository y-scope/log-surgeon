use crate::regex::Regex;

#[derive(Debug)]
pub struct Schema {
	pub rules: Vec<Rule>,
	pub delimiters: String,
}

#[derive(Debug)]
pub struct Rule {
	pub idx: usize,
	pub name: String,
	pub regex: Regex,
}

impl Schema {
	pub fn new() -> Self {
		Self {
			rules: Vec::new(),
			delimiters: String::new(),
		}
	}

	pub fn add_rule<LikeString>(&mut self, name: LikeString, regex: Regex)
	where
		LikeString: Into<String>,
	{
		self.rules.push(Rule {
			idx: self.rules.len(),
			name: name.into(),
			regex,
		});
	}
}
