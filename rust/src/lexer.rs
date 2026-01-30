use crate::dfa::Dfa;
use crate::dfa::GotRule;
use crate::nfa::NfaError;
use crate::nfa::Variable;
use crate::schema::Schema;

#[derive(Debug)]
pub struct Lexer {
	schema: Schema,
	dfa: Dfa,
}

#[derive(Debug)]
pub struct Fragment<'input> {
	pub static_text: &'input str,
	pub rule: usize,
	pub lexeme: &'input str,
}

impl Lexer {
	pub fn new(schema: &Schema) -> Result<Self, NfaError> {
		let dfa: Dfa = Dfa::for_schema(&schema)?;
		Ok(Self {
			schema: schema.clone(),
			dfa,
		})
	}

	pub fn next_fragment<'input, F>(
		&mut self,
		input: &'input str,
		pos: &mut usize,
		mut on_capture: F,
	) -> Fragment<'input>
	where
		F: FnMut(&Variable, &str),
	{
		let old_pos: usize = *pos;
		while *pos < input.len() {
			match self.dfa.simulate_with_captures(&input[*pos..], &mut on_capture) {
				Ok(GotRule { rule, lexeme }) => {
					let static_text_end: usize = *pos;
					*pos += lexeme.len();
					if let Some(next) = input[*pos..].chars().next() {
						if !self.is_delimiter(next) {
							*pos += next.len_utf8();
							self.glob_static_text(input, pos);
							continue;
						}
					}
					return Fragment {
						static_text: str::from_utf8(&input.as_bytes()[old_pos..static_text_end]).unwrap(),
						rule,
						lexeme,
					};
				},
				Err(consumed) => {
					*pos += consumed;
					self.glob_static_text(input, pos);
				},
			}
		}

		Fragment {
			static_text: str::from_utf8(&input.as_bytes()[old_pos..*pos]).unwrap(),
			rule: 0,
			lexeme: &input[*pos..],
		}
	}

	fn glob_static_text(&self, input: &str, pos: &mut usize) {
		for ch in input[*pos..].chars() {
			*pos += ch.len_utf8();
			if self.is_delimiter(ch) {
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

	#[test]
	fn basic() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("hello", Regex::from_pattern("hello world").unwrap());
		schema.add_rule("bye", Regex::from_pattern("goodbye").unwrap());

		let mut lexer: Lexer = Lexer::new(&schema).unwrap();
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

	impl Fragment<'_> {
		fn projection(&self) -> (usize, &str) {
			(self.rule, self.lexeme)
		}
	}

	fn ignore(_: &Variable, _: &str) {}
}
