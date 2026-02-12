use crate::dfa::Dfa;
use crate::dfa::MatchedRule;
use crate::nfa::CaptureInfo;
use crate::schema::Schema;

#[derive(Debug)]
pub struct Lexer {
	schema: Schema,
	dfa: Dfa,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Token<'schema, 'input> {
	Variable {
		rule: usize,
		name: &'schema str,
		lexeme: &'input str,
		// fragments: Vec<Fragment<'schema, 'input>>,
	},
	StaticText(&'input str),
	EndOfInput,
}

impl Lexer {
	pub fn new(schema: Schema) -> Self {
		let dfa: Dfa = schema.build_dfa();
		Self { schema, dfa }
	}

	pub fn next_token<'input, F>(&self, input: &'input str, pos: &mut usize, mut on_capture: F) -> Token<'_, 'input>
	where
		F: FnMut(usize, usize, usize),
	{
		if *pos == input.len() {
			return Token::EndOfInput;
		}

		let start: usize = *pos;

		match self
			.dfa
			.simulate_with_captures(&input[*pos..], |_, t, _, i, j| on_capture(t, i, j))
		{
			Ok(MatchedRule { rule, lexeme }) => {
				*pos += lexeme.len();
				Token::Variable {
					rule,
					name: &self.schema.rules()[rule].name,
					lexeme,
				}
			},
			Err(consumed) => {
				*pos += consumed;
				self.glob_static_text(input, pos);
				Token::StaticText(&input[start..*pos])
			},
		}
	}

	pub fn rule_name(&self, i: usize) -> &str {
		&self.schema.rules()[i].name
	}

	pub fn capture_name(&self, i: usize) -> &str {
		self.dfa.capture_name(i)
	}

	fn glob_static_text(&self, input: &str, pos: &mut usize) {
		for ch in input[*pos..].chars() {
			if ch == '\n' {
				break;
			}
			*pos += ch.len_utf8();
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
				break;
			}
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

		let mut lexer: Lexer = Lexer::new(schema);
		let input: &str = "hello world goodbye hello world  goodbye  ";
		let mut pos: usize = 0;

		/*
		// TODO use assert_matches once stabilized
		assert_eq!(
			lexer.next_token(input, &mut pos),
			Ok(Variable {
				rule: 1,
				name: "hello",
				lexeme: "hello world",
				fragments: Vec::new(),
			})
		);

		assert_eq!(lexer.next_token(input, &mut pos), Err(Some(" ")));

		assert_eq!(
			lexer.next_token(input, &mut pos),
			Ok(Variable {
				rule: 2,
				name: "bye",
				lexeme: "goodbye",
				fragments: Vec::new(),
			})
		);

		assert_eq!(lexer.next_token(input, &mut pos), Err(Some(" ")));

		assert_eq!(
			lexer.next_token(input, &mut pos),
			Ok(Variable {
				rule: 1,
				name: "hello",
				lexeme: "hello world",
				fragments: Vec::new(),
			})
		);

		assert_eq!(lexer.next_token(input, &mut pos), Err(Some("  ")));

		assert_eq!(
			lexer.next_token(input, &mut pos),
			Ok(Variable {
				rule: 2,
				name: "bye",
				lexeme: "goodbye",
				fragments: Vec::new(),
			})
		);

		assert_eq!(lexer.next_token(input, &mut pos), Err(Some("  ")));
		assert_eq!(lexer.next_token(input, &mut pos), Err(None));
		*/
	}

	fn ignore(_: &CaptureInfo, _: &str) {}
}
