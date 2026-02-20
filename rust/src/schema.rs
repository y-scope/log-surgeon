use crate::dfa::Dfa;
use crate::nfa::Nfa;
use crate::regex::IntoRegex;
use crate::regex::Regex;

#[derive(Debug, Clone)]
pub struct Schema {
	rules: Vec<Rule>,
	delimiters: String,
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
				regex: Regex::Literal('\n'),
			}],
			delimiters: Self::DEFAULT_DELIMITERS.to_owned(),
		}
	}

	pub fn set_delimiters<LikeString>(&mut self, delimiters: LikeString)
	where
		LikeString: Into<String>,
	{
		self.delimiters = delimiters.into();
	}

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
		assert_ne!(name, "newline");
		assert_ne!(name, "static");
		assert_ne!(name, "delimiters");

		let regex: Regex = regex.into()?;

		let idx: usize = self.rules.len();
		self.rules.push(Rule { idx, name, regex });
		Ok(())
	}
}

impl Schema {
	pub fn build_dfa(&self) -> Dfa {
		let nfa: Nfa = Nfa::for_rules(&self.rules);
		let dfa: Dfa = Dfa::determinization(&nfa);
		dfa
	}
}

// Accessors
impl Schema {
	pub fn rules(&self) -> &[Rule] {
		&self.rules
	}

	pub fn delimiters(&self) -> &str {
		&self.delimiters
	}
}

// Nolonger needed?
// TODO
#[allow(unused)]
impl Schema {
	fn pattern_for_delimiters(delimiters: &str) -> Regex {
		let escaped: String = delimiters
			.chars()
			.fold(String::new(), |buf, ch| buf + &format!("\\u{{{:x}}}", u32::from(ch)));
		let pattern: String = format!("[{escaped}]");
		Regex::from_pattern(&pattern).unwrap()
	}

	fn pattern_for_static_text(delimiters: &str) -> Regex {
		let mut escaped: String = String::new();
		for ch in delimiters.chars() {
			escaped += &format!("\\u{{{:x}}}", u32::from(ch));
		}
		let pattern: String = format!("[^{escaped}]+|[{escaped}]+");
		Regex::from_pattern(&pattern).unwrap()
	}
}
