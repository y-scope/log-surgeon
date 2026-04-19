use std::ffi::c_char;
use std::marker::PhantomData;
use std::num::NonZero;
use std::str::Utf8Error;

use crate::dfa::MatchedCapture;
use crate::dfa::Tdfa;
use crate::dfa::TdfaExecution;
use crate::ffi::UncheckedCArray;
use crate::log_event::Capture;
use crate::log_event::CaptureFfiPointers;
use crate::log_event::LogEvent;
use crate::parser::Parser;
use crate::regex::Regex;
use crate::regex::RegexError;
use crate::regex::TopLevelRegex;
use crate::schema::Rule;
use crate::schema::Schema;
use crate::schema::SchemaBuilder;
use crate::schema::VariableOrCaptures;
use crate::search::Interpretation;
use crate::search::SearchString;
use crate::search::SubQuery;
use crate::utils::Range;

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
	leaf_captures: Vec<Capture>,
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
	unsafe extern "C" fn log_surgeon_schema_builder_build(builder: Box<SchemaBuilder>) -> Box<Schema> {
		Box::new(builder.build())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_schema_from_definition(definition: CCharArray<'_>) -> Option<Box<Schema>> {
		let definition: &str = definition.as_utf8().unwrap();
		if let Ok(schema) = Schema::from_schema_definition(definition) {
			Some(Box::new(schema))
		} else {
			None
		}
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
	extern "C" fn log_surgeon_log_event_log_type<'a>(log_event: &'a LogEvent<'_>) -> CCharArray<'a> {
		CCharArray::from_utf8(log_event.log_type.as_str())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_all_captures<'a>(log_event: &LogEvent<'a>, len: &mut usize) -> *const Capture {
		*len = log_event.all_captures.len();
		log_event.all_captures.as_ptr()
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_leaf_capture_indices<'a>(
		log_event: &LogEvent<'a>,
		len: &mut usize,
	) -> *const usize {
		*len = log_event.leaf_indices.len();
		log_event.leaf_indices.as_ptr()
	}
}

mod query {
	use super::*;

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_search_query_interpretations(
		parser: &Parser,
		input: CCharArray<'_>,
		name: CCharArray<'_>,
	) -> Box<Vec<Interpretation>> {
		let query: SearchString = SearchString::parse(input.as_utf8().unwrap()).unwrap();
		let name: &str = name.as_utf8().unwrap();
		let interpretations: Vec<Interpretation> = query.interpretations_for_name(&parser.lexer.schema, name);
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
	extern "C" fn log_surgeon_search_sub_query_get_rule(sub_query: &SubQuery) -> Option<NonZero<u16>> {
		sub_query.rule_idx
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_sub_query_get_name(sub_query: &SubQuery) -> CCharArray<'_> {
		CCharArray::from_utf8(&sub_query.name)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_sub_query_match_input(sub_query: &SubQuery, input: CCharArray<'_>) -> bool {
		let input: &str = input.as_utf8().unwrap();
		sub_query.execute(input)
	}

	#[unsafe(no_mangle)]
	#[tracing::instrument(skip_all, level = "trace")]
	extern "C" fn log_surgeon_search_by_named_type<'a>(
		schema: &'a Schema,
		name: CCharArray<'_>,
		value: CCharArray<'a>,
	) -> Box<SearchResult> {
		let name: &str = name.as_utf8().unwrap();
		let value: &str = value.as_utf8().unwrap();

		let mut leaf_captures: Vec<Capture> = Vec::new();
		let on_capture: fn(&mut Vec<Capture>, &Rule, &MatchedCapture, &str) = |leaf_captures, rule, capture, value| {
			if capture.is_leaf {
				leaf_captures.push(Capture {
					rule_idx: rule.idx,
					capture_id: Some(capture.capture_id),
					parent_index: usize::MAX,
					parent_id: capture.parent_id,
					range: capture.range,
					is_leaf: true,
					ffi_pointers: CaptureFfiPointers {
						parent: std::ptr::null(),
						lexeme: UncheckedCArray::from_str(&value[capture.range.start..capture.range.end]),
						variable_name: UncheckedCArray::from_str(&rule.name),
						capture_name: UncheckedCArray::from_str(
							&rule.capture_info[capture.capture_id.get() as usize].name,
						),
					},
				});
			}
		};

		let Some(rows): Option<VariableOrCaptures<Regex>> = schema.regexes_for_name(name) else {
			return Box::new(SearchResult { leaf_captures });
		};

		match rows {
			VariableOrCaptures::Variable(rows) => {
				for (rule_idx, _regex) in rows.into_iter() {
					let rule: &Rule = &schema[rule_idx];
					let dfa: Tdfa = Tdfa::for_rules(std::iter::once(rule), schema.delimiters.clone());
					let mut data: TdfaExecution = dfa.execution_data();
					if dfa
						.execute_with_captures_for_search(value, u32::from(schema.anchor_ch), &mut data)
						.is_some()
					{
						data.captures
							.iter()
							.for_each(|capture| on_capture(&mut leaf_captures, rule, capture, value));
						if leaf_captures.is_empty() {
							leaf_captures.push(Capture {
								rule_idx: rule.idx,
								capture_id: None,
								parent_index: usize::MAX,
								parent_id: None,
								range: Range {
									start: 0,
									end: value.len(),
								},
								is_leaf: true,
								ffi_pointers: CaptureFfiPointers {
									parent: std::ptr::null(),
									lexeme: UncheckedCArray::from_str(value),
									variable_name: UncheckedCArray::from_str(&rule.name),
									capture_name: UncheckedCArray::from_str(""),
								},
							});
						}
					}
				}
			},
			VariableOrCaptures::Captures(rows) => {
				for (rule_idx, _info, regex) in rows.into_iter() {
					let rule: &Rule = &schema[rule_idx];
					let regex: Regex = Regex::Sequence(vec![Regex::AnyChar, regex]);
					let dfa: Tdfa = Tdfa::for_rules(
						std::iter::once(&Rule::new(
							rule.idx,
							rule.name.clone(),
							TopLevelRegex {
								anchor_before: false,
								anchor_after: false,
								inner: regex,
							},
						)),
						schema.delimiters.clone(),
					);
					let mut data: TdfaExecution = dfa.execution_data();
					dfa.execute_with_captures_for_search(value, u32::from(schema.anchor_ch), &mut data);
					data.captures
						.iter()
						.for_each(|capture| on_capture(&mut leaf_captures, rule, capture, value));
				}
			},
		}

		Box::new(SearchResult { leaf_captures })
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_result_get_leaf_captures<'a>(
		search_result: &'a SearchResult,
		len: &mut usize,
	) -> *const Capture {
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
