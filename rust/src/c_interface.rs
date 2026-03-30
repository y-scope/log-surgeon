use std::convert::Infallible;
use std::ffi::c_char;
use std::marker::PhantomData;
use std::num::NonZero;
use std::str::Utf8Error;

use crate::dfa::MatchedCapture;
use crate::dfa::Tdfa;
use crate::log_event::LogEvent;
use crate::parser::Parser;
use crate::query::Interpretation;
use crate::query::SearchString;
use crate::regex::Regex;
use crate::regex::RegexError;
use crate::schema::Rule;
use crate::schema::Schema;
use crate::schema::SchemaBuilder;

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
#[derive(Debug, Clone, Copy)]
pub struct CCapture<'event> {
	pub rule_id: Option<NonZero<u16>>,
	/// `None`/zero when it is an implicit capture of the entire variable pattern.
	pub capture_id: Option<NonZero<u32>>,
	pub parent_id: Option<NonZero<u32>>,

	/// Offset relative to start of log event message.
	pub start: usize,
	/// Offset relative to start of log event message.
	pub end: usize,

	pub is_leaf: bool,

	pub variable_name: CCharArray<'event>,
	pub capture_name: CCharArray<'event>,
	pub lexeme: CCharArray<'event>,
}

#[derive(Debug)]
pub struct SearchResult<'a> {
	#[allow(unused)]
	rule: NonZero<u16>,
	#[allow(unused)]
	variable_name: CCharArray<'a>,
	leaf_captures: Vec<CCapture<'a>>,
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

impl CCapture<'_> {
	fn null() -> Self {
		Self {
			rule_id: None,
			capture_id: None,
			parent_id: None,
			start: 0,
			end: 0,
			is_leaf: false,
			variable_name: CCharArray::null(),
			capture_name: CCharArray::null(),
			lexeme: CCharArray::null(),
		}
	}
}

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
	let regex: Regex = match Regex::from_pattern(pattern) {
		Ok(regex) => regex,
		Err(err) => {
			return Some(Box::new(err));
		},
	};
	let Ok(_): Result<(), Infallible> = builder.add_rule_with_priority(priority, name, regex);
	None
}

#[unsafe(no_mangle)]
unsafe extern "C" fn log_surgeon_schema_builder_build(builder: Box<SchemaBuilder>) -> Box<Schema> {
	Box::new(builder.build())
}

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
	extern "C" fn log_surgeon_log_event_get_leaf_capture<'a>(
		log_event: &LogEvent<'a>,
		i: usize,
		parser: &'a Parser,
	) -> CCapture<'a> {
		if let Some(capture) = log_event.leaf_captures.get(i) {
			let (variable_name, capture_name): (&str, &str) = capture.names(&parser.lexer.schema);
			let lexeme: &str = &log_event.message[capture.range.0..capture.range.1];
			return CCapture {
				rule_id: Some(capture.rule_idx.index),
				capture_id: capture.capture_id,
				parent_id: capture.parent_id,
				start: capture.range.0,
				end: capture.range.1,
				is_leaf: capture.is_leaf,
				variable_name: CCharArray::from_utf8(variable_name),
				capture_name: CCharArray::from_utf8(capture_name),
				lexeme: CCharArray::from_utf8(lexeme),
			};
		}
		CCapture::null()
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_get_non_leaf_capture<'a>(
		log_event: &LogEvent<'a>,
		i: usize,
		parser: &'a Parser,
	) -> CCapture<'a> {
		if let Some(capture) = log_event.non_leaf_captures.get(i) {
			let (variable_name, capture_name): (&str, &str) = capture.names(&parser.lexer.schema);
			let lexeme: &str = &log_event.message[capture.range.0..capture.range.1];
			return CCapture {
				rule_id: Some(capture.rule_idx.index),
				capture_id: capture.capture_id,
				parent_id: capture.parent_id,
				start: capture.range.0,
				end: capture.range.1,
				is_leaf: capture.is_leaf,
				variable_name: CCharArray::from_utf8(variable_name),
				capture_name: CCharArray::from_utf8(capture_name),
				lexeme: CCharArray::from_utf8(lexeme),
			};
		}
		CCapture::null()
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

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_by_named_type<'a>(
		schema: &'a Schema,
		name: CCharArray<'_>,
		value: CCharArray<'a>,
	) -> Option<Box<SearchResult<'a>>> {
		let parts: Vec<&str> = name.as_utf8().unwrap().split('.').collect::<Vec<_>>();
		let value: &str = value.as_utf8().unwrap();
		let variable_name: &str = parts.first().copied()?;
		let capture_names: &[&str] = &parts[1..];
		for rule in schema.rules.iter() {
			if rule.name != variable_name {
				continue;
			}
			let mut leaf_captures: Vec<CCapture<'_>> = Vec::new();
			let mut on_capture = |capture: MatchedCapture| {
				if capture.is_leaf {
					leaf_captures.push(CCapture {
						rule_id: Some(rule.idx.index),
						capture_id: Some(capture.capture_id),
						parent_id: capture.parent_id,
						start: capture.start,
						end: capture.end,
						is_leaf: true,
						variable_name: CCharArray::from_utf8(&rule.name),
						capture_name: CCharArray::from_utf8(&rule.capture_info[capture.capture_id.get() as usize].name),
						lexeme: CCharArray::from_utf8(&value[capture.start..capture.end]),
					});
				}
			};
			if let Some(first) = capture_names.first().copied() {
				find_capture(&rule.regex, first, &capture_names[1..], &mut |regex| {
					let regex: Regex = Regex::Sequence(vec![Regex::AnyChar, regex.clone()]);
					let dfa: Tdfa = Tdfa::for_rules(
						std::iter::once(&Rule::new(rule.idx, rule.name.clone(), regex)),
						schema.delimiters.clone(),
					);
					dfa.execute_with_captures(value, u32::from(schema.anchor_ch), &mut on_capture, rule.idx);
				});
				if !leaf_captures.is_empty() {
					return Some(Box::new(SearchResult {
						rule: rule.idx.index,
						variable_name: CCharArray::from_utf8(&rule.name),
						leaf_captures,
					}));
				}
			} else {
				let dfa: Tdfa = Tdfa::for_rules(std::iter::once(rule), schema.delimiters.clone());
				if dfa
					.execute_with_captures(value, u32::from(schema.anchor_ch), &mut on_capture, rule.idx)
					.is_some()
				{
					if leaf_captures.is_empty() {
						leaf_captures.push(CCapture {
							rule_id: Some(rule.idx.index),
							capture_id: None,
							parent_id: None,
							start: 0,
							end: value.len(),
							is_leaf: true,
							variable_name: CCharArray::from_utf8(&rule.name),
							capture_name: CCharArray::from_utf8(""),
							lexeme: CCharArray::from_utf8(value),
						});
					}
					return Some(Box::new(SearchResult {
						rule: rule.idx.index,
						variable_name: CCharArray::from_utf8(&rule.name),
						leaf_captures,
					}));
				} else {
					continue;
				}
			}
		}
		None
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_result_get_leaf_capture<'a>(
		search_result: &SearchResult<'a>,
		i: usize,
	) -> CCapture<'a> {
		if let Some(capture) = search_result.leaf_captures.get(i) {
			*capture
		} else {
			CCapture::null()
		}
	}

	fn find_capture<F>(regex: &Regex, first: &str, rest: &[&str], func: &mut F)
	where
		F: FnMut(&Regex),
	{
		match regex {
			Regex::Anchor(_) | Regex::AnyChar | Regex::Literal(..) | Regex::Group { .. } => (),
			Regex::Capture { info, item } => {
				if info.name == first {
					func(item);
				} else if let Some(first) = rest.first().copied() {
					find_capture(item, first, &rest[1..], func);
				}
			},
			Regex::KleeneClosure(item) | Regex::BoundedRepetition { item, .. } => {
				find_capture(item, first, rest, func);
			},
			Regex::Sequence(items) | Regex::Alternation(items) => {
				for item in items.iter() {
					find_capture(item, first, rest, func);
				}
			},
		}
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
	extern "C" fn log_surgeon_search_interpretations_drop(value: Box<Box<Vec<Interpretation>>>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_result_drop(value: Box<SearchResult<'_>>) {
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
