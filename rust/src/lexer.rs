use crate::dfa::MatchedRule;
use crate::dfa::Tdfa;
use crate::nfa::AutomataCapture;
use crate::schema::Schema;

#[derive(Debug, Clone)]
pub struct Lexer {
	pub schema: Schema,
	pub dfa: Tdfa,
	dfa_per_rule: Vec<Tdfa>,
	delimiter_ascii_cache: [bool; 0x80],
	non_ascii_delimiters: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Token<'input> {
	Variable { rule: usize, lexeme: &'input str },
	StaticText(&'input str),
	EndOfInput,
}

impl Lexer {
	pub fn new(schema: Schema) -> Self {
		let dfa: Tdfa = schema.build_dfa();
		let dfa_per_rule: Vec<Tdfa> = schema
			.rules()
			.chunks(1)
			.map(|rule| Tdfa::for_rules(rule, schema.delimiters.to_owned()))
			.collect::<Vec<_>>();
		let mut delimiter_ascii_cache: [bool; 0x80] = [false; 0x80];
		let mut non_ascii_delimiters: String = String::new();
		for ch in schema.delimiters.chars() {
			if let Ok(i) = u8::try_from(ch)
				&& let Some(entry) = delimiter_ascii_cache.get_mut(usize::from(i))
			{
				*entry = true;
			} else {
				non_ascii_delimiters.push(ch);
			}
		}
		Self {
			schema,
			dfa,
			dfa_per_rule,
			delimiter_ascii_cache,
			non_ascii_delimiters,
		}
	}

	pub fn next_token<'input, F>(
		&self,
		input: &'input str,
		pos: &mut usize,
		last_was_delimited: u32,
		on_capture: F,
	) -> Token<'input>
	where
		F: FnMut(usize, &'input str, usize, usize),
	{
		if *pos == input.len() {
			return Token::EndOfInput;
		}

		let start: usize = *pos;

		if let Some(MatchedRule { rule, lexeme }) =
			self.dfa.execute_without_captures(&input[*pos..], last_was_delimited)
		{
			self.dfa_per_rule[rule].execute_with_captures(lexeme, last_was_delimited, on_capture);
			*pos += lexeme.len();
			Token::Variable { rule, lexeme }
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
			*pos += ch.len_utf8();
			if self.is_delimiter(ch) {
				break;
			}
		}
	}

	fn is_delimiter(&self, ch: char) -> bool {
		if let Ok(i) = u8::try_from(ch)
			&& let Some(ch_is_delimiter) = self.delimiter_ascii_cache.get(usize::from(i))
		{
			*ch_is_delimiter
		} else {
			self.non_ascii_delimiters.contains(ch)
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn variables() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("hello", "hello world").unwrap();
		schema.add_rule("bye", "goodbye").unwrap();

		let _lexer: Lexer = Lexer::new(schema);
		let _input: &str = "hello world goodbye hello world  goodbye  ";
		let _pos: usize = 0;
	}
}
