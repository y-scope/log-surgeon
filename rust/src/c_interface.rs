use std::ffi::c_char;
use std::marker::PhantomData;
use std::str::Utf8Error;

use crate::log_event::LogEvent;
use crate::log_event::Match;
use crate::parser::Parser;
use crate::regex::Regex;
use crate::regex::RegexError;
use crate::schema::Schema;
use crate::schema::SchemaBuilder;
use crate::search::Interpretation;
use crate::search::SearchString;
use crate::search::SubQuery;

/// Represents a C `T const*` pointer + `size_t` length as a single ABI-stable value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CArray<'lifetime, T> {
	pointer: *const T,
	length: usize,
	_lifetime: PhantomData<&'lifetime [T]>,
}

pub type CCharArray<'lifetime> = CArray<'lifetime, c_char>;

#[derive(Debug)]
pub struct SearchResult {
	leaf_captures: Vec<Match>,
}

impl<'lifetime, T> CArray<'lifetime, T> {
	pub fn null() -> Self {
		Self {
			pointer: std::ptr::null(),
			length: 0,
			_lifetime: PhantomData,
		}
	}

	pub fn from_slice(slice: &'lifetime [T]) -> Self {
		Self {
			pointer: slice.as_ptr(),
			length: slice.len(),
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

#[unsafe(no_mangle)]
unsafe extern "C" fn log_surgeon_enable_tracing() {
	crate::enable_tracing();
}

mod schema {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_schema_builder_new() -> Box<SchemaBuilder> {
		Box::new(SchemaBuilder::new())
	}

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_schema_builder_set_delimiters(
		builder: &mut SchemaBuilder,
		delimiters: CCharArray<'_>,
	) {
		builder.set_delimiters(delimiters.as_utf8().unwrap());
	}

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_schema_builder_add_rule_with_priority<'pattern>(
		builder: &mut SchemaBuilder,
		priority: i32,
		name: CCharArray<'_>,
		pattern: CCharArray<'pattern>,
	) -> Option<Box<RegexError<'pattern>>> {
		let name: &str = name.as_utf8().unwrap();
		let pattern: &str = pattern.as_utf8().unwrap();
		if let Err(err) = builder.add_rule_with_priority(priority, name, pattern) {
			return Some(Box::new(err));
		}
		None
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_schema_add_encoding<'pattern>(
		builder: &mut SchemaBuilder,
		name: CCharArray<'_>,
		pattern: CCharArray<'pattern>,
	) -> Option<Box<RegexError<'pattern>>> {
		let name: &str = name.as_utf8().unwrap();
		let pattern: &str = pattern.as_utf8().unwrap();
		let regex: Regex = match Regex::from_pattern(pattern) {
			Ok(anchored_regex) => anchored_regex.inner,
			Err(err) => {
				return Some(Box::new(err));
			},
		};
		// TODO unwrap
		builder.add_encoding(name, regex).unwrap();
		None
	}

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_schema_builder_build(builder: Box<SchemaBuilder>) -> Box<Schema> {
		Box::new(builder.build())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_schema_from_definition(definition: CCharArray<'_>) -> Option<Box<Schema>> {
		let definition: &str = definition.as_utf8().unwrap();
		if let Ok(builder) = SchemaBuilder::from_schema_definition(definition) {
			Some(Box::new(builder.build()))
		} else {
			None
		}
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_schema_builder_from_definition(definition: CCharArray<'_>) -> Option<Box<SchemaBuilder>> {
		let definition: &str = definition.as_utf8().unwrap();
		if let Ok(builder) = SchemaBuilder::from_schema_definition(definition) {
			Some(Box::new(builder))
		} else {
			None
		}
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_schema_get_encoding(parser: &Parser, encoding_idx: usize, i: usize) -> CCharArray<'_> {
		let Some(possible_encodings): Option<&Vec<String>> = parser.schema.encodings.get(encoding_idx) else {
			return CCharArray::null();
		};
		let Some(encoding_name): Option<&String> = possible_encodings.get(i) else {
			return CCharArray::null();
		};
		CCharArray::from_utf8(encoding_name)
	}
}

mod parser {
	use super::*;

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_parser_new(schema: Box<Schema>) -> Box<Parser> {
		let parser: Parser = Parser::new(*schema);
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
}

mod log_event {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_new<'a>() -> Box<LogEvent<'a>> {
		Box::new(LogEvent::BLANK)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_all_matches<'a>(log_event: &LogEvent<'a>, len: &mut usize) -> *const Match {
		*len = log_event.all_matches.len();
		log_event.all_matches.as_ptr()
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_leaf_match_indices<'a>(
		log_event: &LogEvent<'a>,
		len: &mut usize,
	) -> *const usize {
		*len = log_event.leaf_indices.len();
		log_event.leaf_indices.as_ptr()
	}
}

mod search {
	use super::*;

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_search_query_interpretations(
		parser: &Parser,
		input: CCharArray<'_>,
		name: CCharArray<'_>,
	) -> Box<Vec<Interpretation>> {
		let query: SearchString = SearchString::parse(input.as_utf8().unwrap()).unwrap();
		let name: &str = name.as_utf8().unwrap();
		let interpretations: Vec<Interpretation> = query.get_interpretations(&parser.schema, name);
		Box::new(interpretations)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_get_interpretation(
		interpretations: &Vec<Interpretation>,
		i: usize,
	) -> Option<&Interpretation> {
		interpretations.get(i)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_get_sub_query(interpretation: &Interpretation, i: usize) -> Option<&SubQuery> {
		interpretation.sub_queries.get(i)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_sub_query_get_qualified_name(sub_query: &SubQuery) -> CCharArray<'_> {
		if !sub_query.fully_qualified_name.is_empty() {
			CCharArray::from_utf8(&sub_query.fully_qualified_name)
		} else {
			CCharArray::null()
		}
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_sub_query_get_value(sub_query: &SubQuery) -> CCharArray<'_> {
		CCharArray::from_utf8(&sub_query.string_value)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_result_get_leaf_matches<'a>(
		search_result: &'a SearchResult,
		len: &mut usize,
	) -> *const Match {
		*len = search_result.leaf_captures.len();
		search_result.leaf_captures.as_ptr()
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
	extern "C" fn log_surgeon_search_interpretations_drop(value: Box<Vec<Interpretation>>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_result_drop(value: Box<SearchResult>) {
		std::mem::drop(value);
	}
}

/*
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

		let mut event: LogEvent<'_> = LogEvent::BLANK;

		assert!(log_surgeon_parser_next(&mut parser, input, &mut pos, &mut event));
		// Rust doesn't allow this since it doesn't know that this function simply overwrites event,
		// and that the destructor (when overwriting the old event) doesn't touch the borrow on the parser.
		//assert!(log_surgeon_parser_next(&mut parser, input, &mut pos, &mut event));
	}
}
*/
