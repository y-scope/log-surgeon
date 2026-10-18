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
// We deliberately pass shared handles such as `Box<Arc<ParsingSpec>>` across the FFI boundary:
// `Box` is the opaque-handle convention here (see `log_surgeon.hpp`),
// and the extra indirection is not a mistake.
#![allow(clippy::redundant_allocation)]

use std::sync::Arc;

use crate::ffi::CArray;
use crate::ffi::CCharArray;
use crate::ffi::CUtf8;
use crate::log_event::LogEvent;
use crate::log_event::Match;
use crate::parser::Parser;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::ParsingSpecBuilder;
use crate::regex::Regex;
use crate::search::Interpretation;
use crate::search::LeafQuery;
use crate::search::SearchString;

/// Enable tracing debugging logs; see [`README.md#Debugging`].
#[unsafe(no_mangle)]
unsafe extern "C" fn log_surgeon_enable_tracing() {
	crate::enable_tracing();
}

mod parsing_spec_builder {
	use super::*;

	/// Create a new [`ParsingSpecBuilder`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_new() -> Box<ParsingSpecBuilder> {
		Box::new(ParsingSpecBuilder::new())
	}

	/// See [`ParsingSpecBuilder::set_delimiters`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_set_delimiters(
		builder: &mut ParsingSpecBuilder,
		delimiters: CCharArray<'_>,
	) {
		let delimiters: &str = delimiters.as_utf8().unwrap();
		builder.set_delimiters(delimiters);
	}

	/// See [`ParsingSpecBuilder::add_rule_with_priority`].
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
			eprintln!("Invalid pattern '{}': {:?}.", pattern.escape_default(), err);
			return false;
		}
		true
	}

	/// See [`ParsingSpecBuilder::add_placeholder`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_add_placeholder(
		builder: &mut ParsingSpecBuilder,
		name: CCharArray<'_>,
		pattern: CCharArray<'_>,
	) -> bool {
		let name: &str = name.as_utf8().unwrap();
		let pattern: &str = pattern.as_utf8().unwrap();
		let regex: Regex = match Regex::from_pattern_with_placeholders(pattern, builder) {
			Ok(regex) => regex,
			Err(err) => {
				eprintln!("Invalid pattern '{}': {:?}.", pattern.escape_default(), err);
				return false;
			},
		};
		if builder.add_placeholder(name, regex).is_err() {
			eprintln!("Placeholder '{}' already exists.", name.escape_default());
			return false;
		}
		true
	}

	/// See [`ParsingSpecBuilder::add_encoding`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_add_encoding(
		builder: &mut ParsingSpecBuilder,
		name: CCharArray<'_>,
		pattern: CCharArray<'_>,
	) -> bool {
		let name: &str = name.as_utf8().unwrap();
		let pattern: &str = pattern.as_utf8().unwrap();
		let regex: Regex = match Regex::from_pattern(pattern) {
			Ok(regex) => regex,
			Err(err) => {
				eprintln!("Invalid pattern '{}': {:?}.", pattern.escape_default(), err);
				return false;
			},
		};
		if builder.add_encoding(name, regex).is_err() {
			eprintln!("Encoding '{}' already exists.", name.escape_default());
			return false;
		}
		true
	}

	/// Consume the (boxed) [`ParsingSpecBuilder`] to construct a shared [`ParsingSpec`].
	///
	/// The spec is returned behind an [`Arc`] so it can outlive any single [`Parser`] and back
	/// several of them (see [`log_surgeon_parsing_spec_create_parser`]),
	/// and/or be searched directly.
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_build(
		builder: Box<ParsingSpecBuilder>,
	) -> Box<Arc<ParsingSpec>> {
		Box::new(Arc::new(builder.build()))
	}

	/// See [`ParsingSpecBuilder::from_parsing_spec_definition`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_from_definition(
		definition: CCharArray<'_>,
	) -> Option<Box<ParsingSpecBuilder>> {
		let definition: &str = definition.as_utf8().unwrap();
		match ParsingSpecBuilder::from_parsing_spec_definition(definition) {
			Ok(builder) => Some(Box::new(builder)),
			Err(err) => {
				eprintln!("Parsing specification definition invalid: {err:?}.");
				return None;
			},
		}
	}
}

mod parsing_spec {
	use super::*;

	/// Create an owned [`Parser`] sharing the given [`ParsingSpec`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_create_parser(spec: &Arc<ParsingSpec>) -> Box<Parser> {
		Box::new(spec.create_parser())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_get_delimiters(spec: &Arc<ParsingSpec>) -> CUtf8<'_> {
		CUtf8::new(&spec.delimiters)
	}
}

mod parser {
	use super::*;

	/// See [`Parser::next_event`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parser_next<'parser, 'input>(
		parser: &'parser mut Parser,
		input: CCharArray<'input>,
		pos: &mut usize,
		out: &mut LogEvent<'parser>,
	) -> bool {
		let input: &[u8] = input.as_bytes();
		if let Some(event) = parser.next_event(input, pos) {
			*out = event;
			true
		} else {
			false
		}
	}

	/// See [`Parser::reset`].
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parser_reset(parser: &mut Parser) {
		parser.reset();
	}
}

mod log_event {
	use super::*;

	/// Create a (boxed) [`LogEvent`], for cached/reused return value for `log_surgeon_parser_next`.
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_new<'a>() -> Box<LogEvent<'a>> {
		Box::new(LogEvent::BLANK)
	}

	/// Get the matches of a log event.
	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_log_event_all_matches<'a>(
		log_event: &LogEvent<'a>,
		len: &mut usize,
	) -> *const Match {
		*len = log_event.all_matches.len();
		log_event.all_matches.as_ptr()
	}

	/// Get the match indices of a log event.
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
	extern "C" fn log_surgeon_search_by_log_shapes(
		spec: &Arc<ParsingSpec>,
		input: CCharArray<'_>,
		log_shapes: CArray<'_, CCharArray<'_>>,
	) -> Box<Vec<Vec<Interpretation>>> {
		let input: SearchString = SearchString::parse(input.as_utf8().unwrap()).unwrap();
		let log_shapes: Vec<&str> = log_shapes
			.iter()
			.map(|shape| shape.as_utf8().unwrap())
			.collect::<Vec<_>>();
		let interpretations_by_shapes: Vec<Vec<Interpretation>> =
			input.search_by_log_shapes(spec, &log_shapes);
		Box::new(interpretations_by_shapes)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_by_name(
		spec: &Arc<ParsingSpec>,
		input: CCharArray<'_>,
		name: CCharArray<'_>,
	) -> Box<Vec<Interpretation>> {
		let input: SearchString = SearchString::parse(input.as_utf8().unwrap()).unwrap();
		let name: &str = name.as_utf8().unwrap();
		let interpretations: Vec<Interpretation> = input.search_by_name(spec, name);
		Box::new(interpretations)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_get_interpretations_for_shape(
		interpretations: &Vec<Vec<Interpretation>>,
		i: usize,
	) -> Option<&Vec<Interpretation>> {
		interpretations.get(i)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_get_interpretation(
		interpretations: &Vec<Interpretation>,
		i: usize,
	) -> Option<&Interpretation> {
		interpretations.get(i)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_get_leaf_query(
		interpretation: &Interpretation,
		i: usize,
	) -> Option<&LeafQuery> {
		interpretation.leaf_queries.get(i)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_leaf_query_get_name(leaf_query: &LeafQuery) -> CCharArray<'_> {
		CCharArray::from_utf8(&leaf_query.fully_qualified_name)
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_leaf_query_get_value(
		leaf_query: &LeafQuery,
	) -> CCharArray<'_> {
		CCharArray::from_utf8(&leaf_query.string_value)
	}
}

/// Ideally, these would be defined by a macro,
/// but then `cbindgen` can't process them without `-Zunpretty=expanded`,
/// which is only in nightly...
mod clone_impls {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_clone(
		value: &ParsingSpecBuilder,
	) -> Box<ParsingSpecBuilder> {
		Box::new(value.clone())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_clone(
		value: &Arc<ParsingSpec>,
	) -> Box<Arc<ParsingSpec>> {
		Box::new(value.clone())
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parser_clone(value: &Parser) -> Box<Parser> {
		Box::new(value.clone())
	}

	#[unsafe(no_mangle)]
	unsafe extern "C" fn log_surgeon_log_event_clone<'a>(
		value: &LogEvent<'a>,
	) -> Box<LogEvent<'a>> {
		Box::new(value.clone())
	}
}

/// Ideally, these would be defined by a macro,
/// but then `cbindgen` can't process them without `-Zunpretty=expanded`,
/// which is only in nightly...
mod destructor_impls {
	use super::*;

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_builder_drop(value: Box<ParsingSpecBuilder>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_parsing_spec_drop(value: Box<Arc<ParsingSpec>>) {
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
	extern "C" fn log_surgeon_search_interpretations_by_name_drop(value: Box<Vec<Interpretation>>) {
		std::mem::drop(value);
	}

	#[unsafe(no_mangle)]
	extern "C" fn log_surgeon_search_interpretations_by_log_shapes_drop(
		value: Box<Vec<Vec<Interpretation>>>,
	) {
		std::mem::drop(value);
	}
}
