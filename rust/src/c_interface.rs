use std::convert::Infallible;
use std::ffi::c_char;
use std::marker::PhantomData;
use std::num::NonZero;
use std::str::Utf8Error;

use crate::log_event::LogEvent;
use crate::parser::Parser;
use crate::query::Interpretation;
use crate::query::SearchString;
use crate::regex::Regex;
use crate::regex::RegexError;
use crate::schema::Schema;

/// Represents a C `T const*` pointer + `size_t` length as a single ABI-stable value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CArray<'lifetime, T> {
	pointer: *const T,
	length: usize,
	_lifetime: PhantomData<&'lifetime [T]>,
}

pub type CCharArray<'lifetime> = CArray<'lifetime, c_char>;

#[repr(C)]
#[derive(Debug)]
pub struct CCapture<'event> {
	pub rule_id: usize,
	/// `None`/zero when it is an implicit capture of the entire variable pattern.
	pub capture_id: Option<NonZero<u32>>,
	pub parent_id: Option<NonZero<u32>>,

	/// Offset relative to start of log event message.
	pub start: usize,
	/// Offset relative to start of log event message.
	pub end: usize,

	pub variable_name: CCharArray<'event>,
	pub capture_name: CCharArray<'event>,
	pub lexeme: CCharArray<'event>,
}

#[unsafe(no_mangle)]
extern "C" fn log_surgeon_schema_new() -> Box<Schema> {
	Box::new(Schema::new())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn log_surgeon_schema_set_delimiters(schema: &mut Schema, delimiters: CCharArray<'_>) {
	schema.set_delimiters(delimiters.as_utf8().unwrap());
}

#[unsafe(no_mangle)]
unsafe extern "C" fn log_surgeon_schema_add_rule_with_priority<'pattern>(
	schema: &mut Schema,
	priority: i32,
	name: CCharArray<'_>,
	pattern: CCharArray<'pattern>,
) -> Option<Box<RegexError<'pattern>>> {
	let name: &str = name.as_utf8().unwrap();
	let pattern: &str = pattern.as_utf8().unwrap();
	let regex: Regex = match Regex::from_pattern(pattern) {
		Ok(regex) => regex,
		Err(err) => {
			return Some(Box::new(err));
		},
	};
	let Ok(_): Result<(), Infallible> = schema.add_rule_with_priority(priority, name, regex);
	None
}

#[unsafe(no_mangle)]
unsafe extern "C" fn log_surgeon_parser_new(schema: &Schema) -> Box<Parser> {
	let parser: Parser = Parser::new(schema.clone());
	Box::new(parser)
}

#[unsafe(no_mangle)]
extern "C" fn log_surgeon_parser_next<'parser, 'input>(
	parser: &'parser mut Parser,
	input: CCharArray<'input>,
	pos: &mut usize,
	out: &mut LogEvent<'parser>,
) -> bool {
	let input: &str = unsafe { input.as_utf8().unwrap_unchecked() };
	if let Some(event) = parser.next_event(input, pos) {
		*out = event;
		true
	} else {
		false
	}
}

mod log_event {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_new<'a>() -> Box<LogEvent<'a>> {
		Box::new(LogEvent::blank())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_log_type<'a>(log_event: &'a LogEvent<'_>) -> CCharArray<'a> {
		CCharArray::from_utf8(log_event.log_type.as_str())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_get_variable_window(
		log_event: &LogEvent<'_>,
		i: usize,
		start: &mut usize,
		end: &mut usize,
	) -> bool {
		if let Some(variable) = log_event.variables.get(i) {
			*start = variable.0;
			*end = variable.1;
			true
		} else {
			false
		}
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_get_capture<'a>(
		log_event: &LogEvent<'a>,
		i: usize,
		parser: &'a Parser,
	) -> CCapture<'a> {
		if let Some(capture) = log_event.captures.get(i) {
			let (variable_name, capture_name): (&str, &str) = capture.names(&parser.lexer.schema);
			let lexeme: &str = &log_event.message[capture.range.0..capture.range.1];
			return CCapture {
				rule_id: capture.rule_id,
				capture_id: capture.capture_id,
				parent_id: capture.parent_id,
				start: capture.range.0,
				end: capture.range.1,
				variable_name: CCharArray::from_utf8(variable_name),
				capture_name: CCharArray::from_utf8(capture_name),
				lexeme: CCharArray::from_utf8(lexeme),
			};
		}
		CCapture {
			rule_id: 0,
			capture_id: None,
			parent_id: None,
			start: 0,
			end: 0,
			variable_name: CCharArray::null(),
			capture_name: CCharArray::null(),
			lexeme: CCharArray::null(),
		}
	}
}

mod query {
	use super::*;

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_search_query_interpretations(
		parser: &Parser,
		input: CCharArray<'_>,
	) -> Box<Vec<Interpretation>> {
		let query: SearchString = SearchString::parse(input.as_utf8().unwrap()).unwrap();
		let interpretations: Vec<Interpretation> = query.interpretations(&parser.lexer);
		Box::new(interpretations)
	}

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_search_query_interpretation_as_string<'a>(
		interpretations: &'a Vec<Interpretation>,
		i: usize,
		len: &mut usize,
	) -> CCharArray<'a> {
		let s: &str = &interpretations[i].stringified;
		*len = s.len();
		CCharArray::from_utf8(s)
	}
}

impl<'lifetime, T> CArray<'lifetime, T> {
	pub fn null() -> Self {
		Self {
			pointer: std::ptr::null(),
			length: 0,
			_lifetime: PhantomData,
		}
	}

	pub fn as_slice(&self) -> &'lifetime [T] {
		unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
	}
}

impl<'lifetime> CCharArray<'lifetime> {
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

/// `-Zunpretty=expanded` only in nightly...
mod clone_impls {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parser_clone(value: &Parser) -> Box<Parser> {
		Box::new(value.clone())
	}

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_log_event_clone<'a>(value: &LogEvent<'a>) -> Box<LogEvent<'a>> {
		Box::new(value.clone())
	}
}

/// `-Zunpretty=expanded` only in nightly...
mod destructor_impls {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_schema_drop(value: Box<Schema>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_regex_error_drop(value: Box<RegexError<'_>>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parser_drop(value: Box<Parser>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_drop(value: Box<LogEvent<'_>>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_interpretations_drop(value: Box<Box<Vec<Interpretation>>>) {
		std::mem::drop(value);
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn basic() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("hello", "hello world").unwrap();
		schema.add_rule("bye", "goodbye").unwrap();

		let mut parser: Parser = Parser::new(schema);
		let input: CCharArray<'_> = CCharArray::from_utf8("hello world goodbye hello world  goodbye  ");
		let mut pos: usize = 0;

		let mut event: LogEvent<'_> = LogEvent::blank();

		assert!(log_surgeon_parser_next(&mut parser, input, &mut pos, &mut event));
		// Rust doesn't allow this since it doesn't know that this function simply overwrites event,
		// and that the destructor (when overwriting the old event) doesn't touch the borrow on the parser.
		//assert!(log_surgeon_parser_next(&mut parser, input, &mut pos, &mut event));
	}
}
