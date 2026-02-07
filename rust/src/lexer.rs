use crate::c_interface::CStringView;
use crate::dfa::Dfa;
use crate::dfa::GotRule;
use crate::nfa::Nfa;
use crate::nfa::NfaError;
use crate::schema::Schema;

pub struct Lexer<'schema, 'input> {
	schema: &'schema Schema,
	dfa: Dfa<'schema>,
	captures_buffer: Vec<Capture<'schema, 'input>>,
}

pub struct Fragment<'schema, 'input, 'buffer> {
	pub rule: usize,
	pub lexeme: &'input str,
	pub captures: &'buffer [Capture<'schema, 'input>],
	pub is_event_start: bool,
}

#[repr(C)]
pub struct Capture<'schema, 'input> {
	pub name: CStringView<'schema>,
	pub lexeme: CStringView<'input>,
}

impl<'schema, 'input> Lexer<'schema, 'input> {
	pub fn new(schema: &'schema Schema) -> Result<Self, NfaError> {
		let nfa: Nfa<'_> = Nfa::for_schema(schema)?;
		let dfa: Dfa<'_> = Dfa::determinization(&nfa);
		Ok(Self {
			schema,
			dfa,
			captures_buffer: Vec::new(),
		})
	}

	pub fn next_fragment(&mut self, input: &'input str, pos: &mut usize) -> Fragment<'schema, 'input, '_> {
		self.captures_buffer.clear();

		while *pos < input.len() {
			let static_text_end: usize = *pos;
			match self.dfa.simulate_with_captures(&input[*pos..], |name, capture| {
				self.captures_buffer.push(Capture {
					name: CStringView::from_utf8(name),
					lexeme: CStringView::from_utf8(capture),
				});
			}) {
				Ok(GotRule { rule, lexeme }) => {
					*pos += lexeme.len();
					if let Some(next) = input[*pos..].chars().next() {
						*pos += next.len_utf8();
						if !self.is_delimiter(next) {
							self.glob_static_text(input, pos);
							continue;
						}
					}
					let at_line_start = static_text_end == 0
						|| input.as_bytes().get(static_text_end - 1) == Some(&b'\n');
					let is_event_start = at_line_start && self.schema.rules()[rule].is_timestamp;
					return Fragment {
						rule,
						lexeme,
						captures: &self.captures_buffer,
						is_event_start,
					};
				},
				Err(consumed) => {
					*pos += consumed;
					self.glob_static_text(input, pos);
				},
			}
		}

		Fragment {
			rule: 0,
			lexeme: &input[*pos..],
			captures: &[],
			is_event_start: false,
		}
	}

	fn glob_static_text(&self, input: &'input str, pos: &mut usize) {
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

		let mut lexer: Lexer<'_, '_> = Lexer::new(&schema).unwrap();
		let input: &str = "hello world goodbye hello world  goodbye  ";
		let mut pos: usize = 0;
		assert_eq!(lexer.next_fragment(input, &mut pos).projection(), (1, "hello world"));
		assert_eq!(lexer.next_fragment(input, &mut pos).projection(), (2, "goodbye"));
		assert_eq!(lexer.next_fragment(input, &mut pos).projection(), (1, "hello world"));
		assert_eq!(lexer.next_fragment(input, &mut pos).projection(), (2, "goodbye"));
		assert_eq!(lexer.next_fragment(input, &mut pos).projection(), (0, ""));
		assert_eq!(pos, input.len());
	}

	#[test]
	fn event_boundary() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" \n");
		schema.add_timestamp_rule("ts", Regex::from_pattern("[0-9][0-9][0-9][0-9]\\-[0-9][0-9]\\-[0-9][0-9]").unwrap());
		schema.add_rule("word", Regex::from_pattern("[a-zA-Z]+").unwrap());

		let mut lexer: Lexer<'_, '_> = Lexer::new(&schema).unwrap();
		let input: &str = "2024-01-15 hello\n2024-01-16 world\n";
		let mut pos: usize = 0;

		let f1 = lexer.next_fragment(input, &mut pos);
		assert_eq!(f1.rule, 1); // ts
		assert!(f1.is_event_start); // at start of input

		let f2 = lexer.next_fragment(input, &mut pos);
		assert_eq!(f2.rule, 2); // word "hello"
		assert!(!f2.is_event_start);

		let f3 = lexer.next_fragment(input, &mut pos);
		assert_eq!(f3.rule, 1); // ts
		assert!(f3.is_event_start); // after \n

		let f4 = lexer.next_fragment(input, &mut pos);
		assert_eq!(f4.rule, 2); // word "world"
		assert!(!f4.is_event_start);
	}

	#[test]
	fn capture_boundaries() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("kv", Regex::from_pattern("(?<key>[a-z]+)=(?<val>[0-9]+)").unwrap());

		let mut lexer: Lexer<'_, '_> = Lexer::new(&schema).unwrap();
		let input: &str = "foo=123 bar=456 ";
		let mut pos: usize = 0;

		let f1 = lexer.next_fragment(input, &mut pos);
		assert_eq!(f1.lexeme, "foo=123");
		let user_caps: Vec<(&str, &str)> = f1.captures.iter()
			.map(|c| (c.name.as_utf8().unwrap(), c.lexeme.as_utf8().unwrap()))
			.filter(|(name, _)| *name != "kv") // skip whole-match capture
			.collect();
		assert!(user_caps.contains(&("key", "foo")),
			"Expected key='foo', got: {user_caps:?}");
		assert!(user_caps.contains(&("val", "123")),
			"Expected val='123', got: {user_caps:?}");
	}

	impl<'schema, 'input, 'buffer> Fragment<'schema, 'input, 'buffer> {
		fn projection(&self) -> (usize, &'input str) {
			(self.rule, self.lexeme)
		}
	}
}
