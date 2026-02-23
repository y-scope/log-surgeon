use crate::dfa::MatchedRule;
use crate::dfa::Tdfa;
use crate::nfa::AutomataCapture;
use crate::schema::Schema;

#[derive(Debug, Clone)]
pub struct Lexer {
	schema: Schema,
	dfa: Tdfa,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Token<'schema, 'input> {
	Variable {
		rule: usize,
		name: &'schema str,
		lexeme: &'input str,
	},
	StaticText(&'input str),
	EndOfInput,
}

impl Lexer {
	pub fn new(schema: Schema) -> Self {
		let dfa: Tdfa = schema.build_dfa();
		Self { schema, dfa }
	}

	pub fn next_token<'input, F>(&self, input: &'input str, pos: &mut usize, on_capture: F) -> Token<'_, 'input>
	where
		F: FnMut(usize, &'input str, usize, usize),
	{
		if *pos == input.len() {
			return Token::EndOfInput;
		}

		let start: usize = *pos;

		if let Some(MatchedRule { rule, lexeme }) = self.dfa.execute_with_captures(&input[*pos..], on_capture) {
			*pos += lexeme.len();
			Token::Variable {
				rule,
				name: &self.schema.rules()[rule].name,
				lexeme,
			}
		} else {
			self.glob_static_text(input, pos);
			Token::StaticText(&input[start..*pos])
		}
	}

	pub fn rule_name(&self, i: usize) -> &str {
		&self.schema.rules()[i].name
	}

	pub fn capture_info(&self, i: usize) -> &AutomataCapture {
		self.dfa.capture_info(i)
	}

	fn glob_static_text(&self, input: &str, pos: &mut usize) {
		for ch in input[*pos..].chars() {
			if ch == '\n' {
				break;
			}
			if self.is_delimiter(ch) {
				for ch in input[*pos..].chars() {
					if ch == '\n' {
						break;
					} else if self.is_delimiter(ch) {
						*pos += ch.len_utf8();
					} else {
						break;
					}
				}
				return;
			}
			*pos += ch.len_utf8();
		}
	}

	fn is_delimiter(&self, ch: char) -> bool {
		// XXX: Needs benchmarking, but given a small set of delimiter characters,
		// a linear search may be faster than a theoretically $O(1)$ hash lookup.
		self.schema.delimiters().contains(ch)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::regex::Regex;

	/*
	#[test]
	fn basic() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("hello", Regex::from_pattern("hello world").unwrap());
		schema.add_rule("bye", Regex::from_pattern("goodbye").unwrap());

		let mut lexer: Lexer = Lexer::new(schema);
		let input: &str = "hello world goodbye hello world  goodbye  ";
		let mut pos: usize = 0;
		assert_eq!(
			lexer.next_fragment(input, &mut pos, ignore).projection(),
			(1, "hello world")
		);
		assert_eq!(
			lexer.next_fragment(input, &mut pos, ignore).projection(),
			(2, "goodbye")
		);
		assert_eq!(
			lexer.next_fragment(input, &mut pos, ignore).projection(),
			(1, "hello world")
		);
		assert_eq!(
			lexer.next_fragment(input, &mut pos, ignore).projection(),
			(2, "goodbye")
		);
		assert_eq!(lexer.next_fragment(input, &mut pos, ignore).projection(), (0, ""));
		assert_eq!(pos, input.len());
		assert_eq!(lexer.next_fragment(input, &mut pos, ignore).projection(), (0, ""));
	}
	*/

	#[test]
	fn variables() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("hello", Regex::from_pattern("hello world").unwrap());
		schema.add_rule("bye", Regex::from_pattern("goodbye").unwrap());

		let _lexer: Lexer = Lexer::new(schema);
		let _input: &str = "hello world goodbye hello world  goodbye  ";
		let _pos: usize = 0;
	}
}
