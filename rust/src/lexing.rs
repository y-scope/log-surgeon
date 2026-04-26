use crate::dfa::MatchedRule;
use crate::dfa::TdfaExecution;
use crate::schema::RootRule;
use crate::schema::Schema;
use std::str::Chars;

// #[derive(Debug, Clone)]
// pub struct Lexer {
// 	pub schema: Schema,
// 	pub dfa: Tdfa,
// 	pub dfa_per_rule: Vec<Tdfa>,
// 	delimiter_ascii_cache: [bool; 0x80],
// 	non_ascii_delimiters: String,
// }

#[derive(Debug, Eq, PartialEq)]
pub enum Token<'schema, 'input> {
	Variable {
		rule: &'schema RootRule,
		lexeme: &'input str,
		has_captures: bool,
	},
	Newline,
	StaticText(&'input str),
	EndOfInput,
}

// impl Lexer {
// 	pub fn new(schema: Schema) -> Self {
// 		let dfa: Tdfa = schema.build_dfa();
// 		let dfa_per_rule: Vec<Tdfa> = schema
// 			.rules
// 			.iter()
// 			.map(|rule| Tdfa::for_single_rule(rule.idx, &rule.name, &rule.regex.inner))
// 			.collect::<Vec<_>>();
// 		let mut delimiter_ascii_cache: [bool; 0x80] = [false; 0x80];
// 		let mut non_ascii_delimiters: String = String::new();
// 		for ch in schema.delimiters.chars() {
// 			if let Ok(i) = u8::try_from(ch)
// 				&& let Some(entry) = delimiter_ascii_cache.get_mut(usize::from(i))
// 			{
// 				*entry = true;
// 			} else {
// 				non_ascii_delimiters.push(ch);
// 			}
// 		}
// 		Self {
// 			schema,
// 			dfa,
// 			dfa_per_rule,
// 			delimiter_ascii_cache,
// 			non_ascii_delimiters,
// 		}
// 	}
// }

impl Schema {
	pub fn next_token<'schema, 'input>(
		&'schema self,
		input: &'input str,
		pos: &mut usize,
		last_was_delimited: u32,
		data: &mut TdfaExecution,
	) -> Token<'schema, 'input> {
		let start: usize = *pos;

		if start == input.len() {
			return Token::EndOfInput;
		}

		if let Some(MatchedRule { rule_idx, lexeme }) = self
			.main_dfa
			.execute_without_captures(&input[start..], last_was_delimited)
		{
			let rule: &RootRule = &self[rule_idx];
			// First entry is for the root rule.
			let has_captures: bool = rule.capture_info.len() > 1;
			data.clear();
			if has_captures {
				let matched: bool = rule.dfa.execute_with_captures(lexeme, data, rule.idx);
				assert!(matched);
			}
			*pos += lexeme.len();
			Token::Variable {
				rule,
				lexeme,
				has_captures,
			}
		} else {
			let mut chars: Chars<'_> = input[start..].chars();
			// We checked for `start == input.len()` above.
			let first: char = chars.next().unwrap();
			*pos += first.len_utf8();
			if first == '\n' {
				return Token::Newline;
			} else if !self.is_delimiter(first) {
				self.glob_static_text(input, pos);
			}
			Token::StaticText(&input[start..*pos])
		}
	}

	fn glob_static_text(&self, input: &str, pos: &mut usize) {
		for ch in input[*pos..].chars() {
			if ch == '\n' {
				break;
			}
			*pos += ch.len_utf8();
			if self.is_delimiter(ch) {
				break;
			}
		}
	}

	fn is_delimiter(&self, ch: char) -> bool {
		if let Ok(i) = u8::try_from(ch)
			&& let Some(ch_is_delimiter) = self.ascii_delimiters.get(usize::from(i))
		{
			*ch_is_delimiter
		} else {
			self.non_ascii_delimiters.contains(ch)
		}
	}
}
