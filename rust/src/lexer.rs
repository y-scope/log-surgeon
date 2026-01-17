use std::marker::PhantomData;

use crate::c_interface::CStringView;
use crate::dfa::Dfa;
use crate::nfa::Nfa;
use crate::nfa::NfaError;
use crate::schema::Schema;

pub struct Lexer<'schema> {
	dfa: Dfa<'schema>,
	captures_buffer: Vec<Capture<'schema>>,
}

// TODO this is a little spicy, missing lifetime on input
#[repr(C)]
pub struct LogComponent<'schema, 'buffer> {
	pub rule: usize,
	pub start: *const u8,
	pub end: *const u8,
	pub captures: *const Capture<'schema>,
	pub captures_count: usize,
	pub _captures_lifetime: PhantomData<&'buffer [Capture<'schema>]>,
}

#[repr(C)]
pub struct Capture<'schema> {
	pub name: CStringView<'schema>,
	pub start: *const u8,
	pub end: *const u8,
}

impl<'schema> Lexer<'schema> {
	pub fn new(schema: &'schema Schema) -> Result<Self, NfaError> {
		let nfa: Nfa<'_> = Nfa::for_schema(schema)?;
		let dfa: Dfa<'_> = Dfa::determinization(&nfa);
		Ok(Self {
			dfa,
			captures_buffer: Vec::new(),
		})
	}

	pub fn next_token(&mut self, input: &str, pos: &mut usize) -> Option<LogComponent<'schema, '_>> {
		self.captures_buffer.clear();

		let start: usize = *pos;
		let (rule, full_text_match): (usize, &str) = loop {
			let maybe: Option<(usize, &str)> = self.dfa.simulate_with_captures(&input[*pos..], |name, capture| {
				self.captures_buffer.push(Capture {
					name: CStringView::from_utf8(name),
					start: capture.as_bytes().as_ptr_range().start,
					end: capture.as_bytes().as_ptr_range().end,
				});
			});
			if let Some((rule, full_match_text)) = maybe {
				if rule == 0 {
					*pos += 1;
					continue;
				} else if start != *pos {
					return Some(LogComponent {
						rule: 0,
						start: input[start..*pos].as_bytes().as_ptr_range().start,
						end: input[start..*pos].as_bytes().as_ptr_range().end,
						captures: std::ptr::null(),
						captures_count: 0,
						_captures_lifetime: PhantomData,
					});
				}
				break (rule, full_match_text);
			} else {
				if start == *pos {
					return None;
				} else {
					return Some(LogComponent {
						rule: 0,
						start: input[start..*pos].as_bytes().as_ptr_range().start,
						end: input[start..*pos].as_bytes().as_ptr_range().end,
						captures: std::ptr::null(),
						captures_count: 0,
						_captures_lifetime: PhantomData,
					});
				}
			}
		};

		// [`str::len`] is already in bytes, but this is more explicit.
		*pos += full_text_match.as_bytes().len();

		Some(LogComponent {
			rule,
			start: full_text_match.as_bytes().as_ptr_range().start,
			end: full_text_match.as_bytes().as_ptr_range().end,
			captures: self.captures_buffer.as_ptr(),
			captures_count: self.captures_buffer.len(),
			_captures_lifetime: PhantomData,
		})
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::regex::Regex;

	#[test]
	fn basic1() {
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", Regex::from_pattern("hello world").unwrap());
		schema.add_rule("bye", Regex::from_pattern("goodbye").unwrap());

		let mut lexer: Lexer<'_> = Lexer::new(&schema).unwrap();
		let input: &str = "hello worldgoodbyehello world";
		let mut pos: usize = 0;
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 1);
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 2);
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 1);
		assert_eq!(pos, input.len());
	}

	#[test]
	fn basic2() {
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", Regex::from_pattern("hello world").unwrap());
		schema.add_rule("bye", Regex::from_pattern("goodbye").unwrap());

		let mut lexer: Lexer<'_> = Lexer::new(&schema).unwrap();
		let input: &str = "hello world goodbye hello world";
		let mut pos: usize = 0;
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 1);
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 0);
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 2);
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 0);
		assert_eq!(lexer.next_token(input, &mut pos).unwrap().rule, 1);
		assert_eq!(pos, input.len());
	}
}
