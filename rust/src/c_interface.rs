use std::ffi::c_char;
use std::marker::PhantomData;
use std::num::NonZero;
use std::str::Utf8Error;

use crate::lexer::Token;
use crate::parser::LogEvent;
use crate::parser::Parser;
use crate::regex::Regex;
use crate::regex::RegexError;
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
pub struct CVariable<'event> {
	pub rule: usize,
	pub name: CStringView<'event>,
	pub lexeme: CStringView<'event>,
	// pub event: *const LogEvent<'event>,
	// pub index: usize,
}

#[repr(C)]
pub struct CCapture<'parser> {
	pub name: CStringView<'parser>,
	pub lexeme: CStringView<'parser>,
	pub id: u32,
	pub parent_id: u32,
	pub is_leaf: bool,
}

pub type LogFragmentOnCapture = Option<extern "C" fn(data: *const (), name: CStringView<'_>, lexeme: CStringView<'_>)>;

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_schema_new() -> Box<Schema> {
	Box::new(Schema::new())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_set_delimiters(schema: &mut Schema, delimiters: CStringView<'_>) {
	schema.set_delimiters(delimiters.as_utf8().unwrap());
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_add_rule<'pattern>(
	schema: &mut Schema,
	name: CStringView<'_>,
	pattern: CStringView<'pattern>,
) -> Option<Box<RegexError<'pattern>>> {
	let name: &str = name.as_utf8().unwrap();
	let pattern: &str = pattern.as_utf8().unwrap();
	let regex: Regex = match Regex::from_pattern(pattern) {
		Ok(regex) => regex,
		Err(err) => {
			return Some(Box::new(err));
		},
	};
	schema.add_rule(name, regex);
	None
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_parser_new(schema: &Schema) -> Box<Parser> {
	let parser: Parser = Parser::new(schema.clone());
	Box::new(parser)
}

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_log_event_new<'a>() -> Box<LogEvent<'a>> {
	Box::new(LogEvent::blank())
}

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_parser_next<'parser, 'input>(
	parser: &'parser mut Parser,
	input: CStringView<'input>,
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
	extern "C" fn clp_log_mechanic_log_event_log_type<'a>(log_event: &'a LogEvent<'_>) -> CStringView<'a> {
		CStringView::from_utf8(log_event.log_type.as_str())
	}

	#[unsafe(no_mangle)]
	extern "C" fn clp_log_mechanic_log_event_have_header<'a>(log_event: &'a LogEvent<'_>) -> bool {
		log_event.have_header
	}

	#[unsafe(no_mangle)]
	extern "C" fn clp_log_mechanic_log_event_variable<'a>(log_event: &LogEvent<'a>, i: usize) -> CVariable<'a> {
		if let Some(variable) = log_event.variables.get(i) {
			return CVariable {
				rule: variable.rule,
				name: CStringView::from_utf8(variable.name),
				lexeme: CStringView::from_utf8(variable.lexeme),
				// event: std::ptr::from_ref(log_event),
				// index: i,
			};
		}
		CVariable {
			rule: 0,
			name: CStringView::null(),
			lexeme: CStringView::null(),
			// event: std::ptr::null(),
			// index: 0,
		}
	}

	#[unsafe(no_mangle)]
	extern "C" fn clp_log_mechanic_log_event_capture<'a>(
		log_event: &'a LogEvent<'_>,
		i: usize,
		j: usize,
	) -> CCapture<'a> {
		if let Some(variable) = log_event.variables.get(i) {
			if let Some(capture) = variable.captures.get(j) {
				return CCapture {
					name: CStringView::from_utf8(capture.name),
					lexeme: CStringView::from_utf8(capture.lexeme),
					id: capture.id.get(),
					parent_id: capture.parent_id.map_or(0, NonZero::get),
					is_leaf: capture.is_leaf,
				};
			}
		}
		CCapture {
			name: CStringView::null(),
			lexeme: CStringView::null(),
			id: 0,
			parent_id: 0,
			is_leaf: false,
		}
	}
}

impl<'lifetime, T> CSlice<'lifetime, T> {
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

// /// cbindgen doesn't work with macros; need to write out all destructors.
// mod destructors {
// 	use super::*;

// 	#[unsafe(no_mangle)]
// 	unsafe extern "C" fn clp_log_mechanic_schema_drop(value: Box<Schema>) {
// 		std::mem::drop(value);
// 	}

// 	#[unsafe(no_mangle)]
// 	unsafe extern "C" fn clp_log_mechanic_regex_error_drop(value: Box<RegexError<'_>>) {
// 		std::mem::drop(value);
// 	}

// 	#[unsafe(no_mangle)]
// 	unsafe extern "C" fn clp_log_mechanic_parser_drop(value: Box<Parser>) {
// 		std::mem::drop(value);
// 	}

// 	#[unsafe(no_mangle)]
// 	unsafe extern "C" fn clp_log_mechanic_log_event_drop(value: Box<LogEvent<'_>>) {
// 		std::mem::drop(value);
// 	}
// }

macro_rules! destructor {
	($name:ident, $ty:ty) => {
		#[unsafe(no_mangle)]
		unsafe extern "C" fn $name(value: Box<$ty>) {
			std::mem::drop(value);
		}
	};
}

destructor!(clp_log_mechanic_schema_drop, Schema);
destructor!(clp_log_mechanic_regex_error_drop, RegexError<'_>);
destructor!(clp_log_mechanic_parser_drop, Parser);
destructor!(clp_log_mechanic_log_event_drop, LogEvent<'_>);

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

		let mut event: LogEvent<'_> = LogEvent::blank();

		assert!(clp_log_mechanic_parser_next(&mut parser, input, &mut pos, &mut event));
		// Rust doesn't allow this since it doesn't know that this function simply overwrites event,
		// and that the destructor (when overwriting the old event) doesn't touch the borrow on the parser.
		//assert!(clp_log_mechanic_parser_next(&mut parser, input, &mut pos, &mut event));
	}
}
