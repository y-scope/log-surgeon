use std::ffi::c_char;
use std::marker::PhantomData;
use std::str::Utf8Error;

use crate::lexer::Token;
use crate::parser::LogEventView;
use crate::parser::Parser;
use crate::regex::Regex;
use crate::schema::Schema;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CSlice<'lifetime, T> {
	pointer: *const T,
	length: usize,
	_lifetime: PhantomData<&'lifetime [T]>,
}

pub type CStringView<'lifetime> = CSlice<'lifetime, c_char>;

#[repr(C)]
pub struct CLogFragment<'input> {
	/// `0` iff no variable found (static text until end of input).
	pub rule: usize,
	/// Start of variable (if found).
	pub start: *const u8,
	/// End of variable (if found).
	pub end: *const u8,
	/// Lifetime of the matched text.
	pub _marker: PhantomData<CStringView<'input>>,
}

#[repr(C)]
pub struct CCapture<'lexer, 'input> {
	pub name: CStringView<'lexer>,
	pub lexeme: CStringView<'input>,
}

pub type LogFragmentOnCapture = Option<extern "C" fn(data: *const (), name: CStringView<'_>, lexeme: CStringView<'_>)>;

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_schema_new() -> Box<Schema> {
	Box::new(Schema::new())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_delete(schema: Box<Schema>) {
	std::mem::drop(schema);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_set_delimiters(schema: &mut Schema, delimiters: CStringView<'_>) {
	schema.set_delimiters(delimiters.as_utf8().unwrap());
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_add_rule(
	schema: &mut Schema,
	name: CStringView<'_>,
	pattern: CStringView<'_>,
) {
	let name: &str = name.as_utf8().unwrap();
	let pattern: &str = pattern.as_utf8().unwrap();
	let regex: Regex = Regex::from_pattern(pattern).unwrap();
	schema.add_rule(name, regex);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_parser_new(schema: &Schema) -> Box<Parser> {
	let parser: Parser = Parser::new(schema.clone());
	Box::new(parser)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_parser_delete<'a>(parser: Box<Parser>) {
	std::mem::drop(parser);
}

/*
#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_parser_next_event<'parser, 'input>(
	parser: &'parser mut Parser,
	input: CStringView<'input>,
	pos: &mut usize,
) -> Option<Box<LogEvent<'parser, 'input>>> {
	let input: &str = input.as_utf8().unwrap();
	let event: LogEvent<'_, '_> = parser.next_event(input, pos)?;
	Some(Box::new(event))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_event_delete(event: Box<LogEvent<'_, '_>>) {
	std::mem::drop(event);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_event_token_count(event: &LogEvent<'_, '_>) -> usize {
	event.tokens.len()
}

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_event_token<'event, 'schema, 'input>(
	event: &'event LogEvent<'schema, 'input>,
	i: usize,
) -> &'event Token<'schema, 'input> {
	&event.tokens[i]
}

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_event_token_rule(token: &Token<'_, '_>) -> usize {
	match token {
		Token::Variable(variable) => variable.rule,
		Token::StaticText(_) => 0,
	}
}

#[unsafe(no_mangle)]
extern "C" fn clp_log_mecahnic_event_token_name<'schema>(token: &Token<'schema, '_>) -> CStringView<'schema> {
	CStringView::from_utf8(match token {
		Token::Variable(variable) => variable.name,
		Token::StaticText(_) => "static",
	})
}
*/

impl<'lifetime, T> CSlice<'lifetime, T> {
	pub fn as_slice(&self) -> &'lifetime [T] {
		unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
	}
}

impl<'lifetime> CStringView<'lifetime> {
	pub fn from_utf8(utf8: &'lifetime str) -> Self {
		Self {
			pointer: utf8.as_bytes().as_ptr().cast::<c_char>(),
			length: utf8.as_bytes().len(),
			_lifetime: PhantomData,
		}
	}

	pub fn as_utf8(&self) -> Result<&'lifetime str, Utf8Error> {
		let bytes: &[u8] = unsafe { std::slice::from_raw_parts(self.pointer.cast::<u8>(), self.length) };
		str::from_utf8(bytes)
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

		let mut parser: Parser = Parser::new(schema);
		let input: CStringView<'_> = CStringView::from_utf8("hello world goodbye hello world  goodbye  ");
		let mut pos: usize = 0;

		// unsafe {
		// 	assert_eq!(
		// 		clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos, None, std::ptr::null()).rule,
		// 		1
		// 	);
		// 	assert_eq!(
		// 		clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos, None, std::ptr::null()).rule,
		// 		2
		// 	);
		// 	assert_eq!(
		// 		clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos, None, std::ptr::null()).rule,
		// 		1
		// 	);
		// 	assert_eq!(
		// 		clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos, None, std::ptr::null()).rule,
		// 		2
		// 	);
		// 	assert_eq!(
		// 		clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos, None, std::ptr::null()).rule,
		// 		0
		// 	);
		// }
		// assert_eq!(pos, input.length);
	}
}
