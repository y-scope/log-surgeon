//! FFI is inherently unsafe in that the Rust compiler cannot verify the validity of foreign calls;
//! however, it's counterproductive to code auditing to simply mark every FFI function as unsafe.
//!
//! We assume that calls to these functions are "as if" they came from other Rust code;
//! i.e. the values are valid and lifetimes don't violate the rules of the Rust Abstract Machine.
//! This includes custom types such as [`CCharArray`],
//! for which in Rust source (outside its own module),
//! it is (should be) impossible to materialize an invalid pointer/lifetime/slice value.
//! Therefore, even though it's possible for a foreign caller to pass an invalid [`CCharArray`]
//! to a function below, those functions would not be (are not) marked `unsafe`.
//!

use std::sync::Arc;

use crate::ffi::CCharArray;
use crate::log_event::LogEvent;
use crate::log_event::Match;
use crate::parser::Parser;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::regex::Regex;
use crate::search::Interpretation;
use crate::search::SearchString;
use crate::search::SubQuery;

#[derive(Debug)]
pub struct SearchResult {
	leaf_captures: Vec<Match>,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn log_surgeon_enable_tracing() {
	crate::enable_tracing();
}

mod parsing_spec {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_new() -> Box<ParsingSpecBuilder> {
		Box::new(ParsingSpecBuilder::new())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_set_delimiters(
		builder: &mut ParsingSpecBuilder,
		delimiters: CCharArray<'_>,
	) {
		let delimiters: &str = delimiters.as_utf8().unwrap();
		builder.set_delimiters(delimiters);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_add_rule_with_priority(
		builder: &mut ParsingSpecBuilder,
		priority: i32,
		name: CCharArray<'_>,
		pattern: CCharArray<'_>,
	) -> bool {
		let name: &str = name.as_utf8().unwrap();
		let pattern: &str = pattern.as_utf8().unwrap();
		if let Err(err) = builder.add_rule_with_priority(priority, name, pattern) {
			eprintln!("Invalid pattern '{}': {:?}", pattern.escape_default(), err);
			return false;
		}
		true
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_add_encoding(
		builder: &mut ParsingSpecBuilder,
		name: CCharArray<'_>,
		pattern: CCharArray<'_>,
	) -> bool {
		let name: &str = name.as_utf8().unwrap();
		let pattern: &str = pattern.as_utf8().unwrap();
		let regex: Regex = match Regex::from_pattern(pattern) {
			Ok(anchored_regex) => anchored_regex.inner,
			Err(err) => {
				eprintln!("Invalid pattern '{}': {:?}", pattern.escape_default(), err);
				return false;
			},
		};
		// TODO unwrap
		builder.add_encoding(name, regex).unwrap();
		true
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_build(builder: Box<ParsingSpecBuilder>) -> Box<ParsingSpec> {
		Box::new(builder.build())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_from_definition(definition: CCharArray<'_>) -> Option<Box<ParsingSpec>> {
		let definition: &str = definition.as_utf8().unwrap();
		if let Ok(builder) = ParsingSpecBuilder::from_parsing_spec_definition(definition) {
			Some(Box::new(builder.build()))
		} else {
			None
		}
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_from_definition(
		definition: CCharArray<'_>,
	) -> Option<Box<ParsingSpecBuilder>> {
		let definition: &str = definition.as_utf8().unwrap();
		if let Ok(builder) = ParsingSpecBuilder::from_parsing_spec_definition(definition) {
			Some(Box::new(builder))
		} else {
			None
		}
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_get_encoding(
		parser: &Parser,
		encoding_idx: usize,
		i: usize,
	) -> CCharArray<'_> {
		let Some(possible_encodings): Option<&Vec<String>> = parser.spec.encodings.get(encoding_idx) else {
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
	extern "C" fn log_surgeon_parser_new(parsing_spec: Box<ParsingSpec>) -> Box<Parser> {
		let parser: Parser = Parser::new(Arc::new(*parsing_spec));
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
	extern "C" fn log_surgeon_search_query_interpretations(
		parser: &Parser,
		input: CCharArray<'_>,
		name: CCharArray<'_>,
	) -> Box<Vec<Interpretation>> {
		let query: SearchString = SearchString::parse(input.as_utf8().unwrap()).unwrap();
		let name: &str = name.as_utf8().unwrap();
		let interpretations: Vec<Interpretation> = query.get_interpretations(&parser.spec, name);
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

/// Ideally, these would be defined by a macro,
/// but then `cbindgen` can't process them without `-Zunpretty=expanded`, which is only in nightly...
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

/// Ideally, these would be defined by a macro,
/// but then `cbindgen` can't process them without `-Zunpretty=expanded`, which is only in nightly...
mod destructor_impls {
	use super::*;

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
