#[cfg(test)]
mod test;

use std::num::NonZero;
use std::sync::Arc;

use crate::dfa::TdfaExecution;
use crate::ffi::CArray;
use crate::ffi::CRange;
use crate::ffi::CUtf8;
use crate::ffi::UncheckedCArray;
use crate::lexer::Lexer;
use crate::lexer::Token;
use crate::log_event::LogEvent;
use crate::log_event::Match;
use crate::log_event::MatchFfiPointers;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::RuleInfo;

/// A parser is almost stateless aside from 2/3 fields:
///
/// - An owned copy of the substring of the input text
///   for the "current" (most recently returned) log event.
///   - This greatly simplifies lifetime management, especially in the presence of FFI,
///     since the parser doesn't own a whole copy of the input text.
///   - Most calls to [`Parser::next_event`] won't require allocation;
///     the memory is bounded by the max log event size.
/// - An owned copy of the text and matches for the header of the next log event.
/// - (Optional/implementation detail) [`TdfaExecution`] cache.
///
#[derive(Debug, Clone)]
pub struct Parser {
	/// We hold an `Arc<ParsingSpec>` since it may be convenient for others to
	/// clone/"hold an owned reference" to the spec too.
	pub spec: Arc<ParsingSpec>,
	lexer: Lexer,
	current_log: WorkingLogEvent,

	// We don't use an `Option<WorkingLogEvent>` since
	// we want to reuse the underlying memory of `pending_header` through resets.
	seen_header: bool,
	pending_header: WorkingLogEvent,
	dfa_execution: TdfaExecution,
}

/// Owned data for a [`LogEvent`].
/// A parser has one for the current (most recently returned) log event,
/// and one for the header of the next log event,
/// so that the memory can be re-used.
#[derive(Debug, Clone)]
struct WorkingLogEvent {
	/// The full text/substring of the input of the log event
	/// (or of the header matched so far).
	message: String,
	all_matches: Vec<Match>,
	root_indices: Vec<usize>,
	leaf_indices: Vec<usize>,
}

impl Parser {
	pub fn new(spec: Arc<ParsingSpec>) -> Self {
		let lexer: Lexer = Lexer::new(spec.clone());
		let mut registers: usize = 0;
		let mut tags: usize = 0;
		for rule in spec.rules.iter() {
			registers = registers.max(rule.dfa.number_of_registers);
			tags = tags.max(rule.dfa.tags.len());
		}
		let dfa_execution: TdfaExecution = TdfaExecution::new(registers, tags);

		Self {
			spec,
			lexer,
			current_log: WorkingLogEvent::new(),
			seen_header: false,
			pending_header: WorkingLogEvent::new(),
			dfa_execution,
		}
	}

	/// Return the next log event;
	/// update `*pos` to the position after the returned event.
	/// Returns `None` iff `*pos == input.len()`.
	pub fn next_event<AsBytes>(&mut self, input: &AsBytes, pos: &mut usize) -> Option<LogEvent<'_>>
	where
		AsBytes: AsRef<[u8]> + ?Sized,
	{
		let input: &[u8] = input.as_ref();

		if *pos == input.len() {
			return None;
		}

		let pos_after_header: usize = *pos;

		std::mem::swap(&mut self.current_log, &mut self.pending_header);
		self.pending_header.clear();

		let header_len: usize = self.current_log.message.len();

		let mut previous_was_newline: bool = true;

		let pos_end: usize = loop {
			let pos_before_token: usize = *pos;
			let token_start: usize = pos_before_token - pos_after_header + header_len;
			let token_starting_capture_count: usize = self.current_log.all_matches.len();
			let token_starting_leaf_indices: usize = self.current_log.leaf_indices.len();
			match self.lexer.next_token(input, pos, &mut self.dfa_execution) {
				Token::Variable {
					rule,
					maybe_encoding_idx,
					lexeme,
					has_captures,
				} => {
					let variable_is_implicit_capture: bool = !has_captures;

					let variable_capture: Match = Match {
						rule_idx: rule.idx,
						sub_rule_id: None,
						parent_index: token_starting_capture_count,
						range: CRange {
							start: token_start,
							end: token_start + lexeme.len(),
						},
						is_leaf: variable_is_implicit_capture,
						encoding_idx: maybe_encoding_idx.map(NonZero::from),
						ffi_pointers: MatchFfiPointers::NULL,
					};

					self.current_log.all_matches.push(variable_capture);
					if variable_is_implicit_capture {
						self.current_log
							.leaf_indices
							.push(token_starting_capture_count);
					}
					self.current_log
						.root_indices
						.push(token_starting_capture_count);

					for regex_capture in self.dfa_execution.captures.iter() {
						let capture_index: usize = self.current_log.all_matches.len();
						assert_eq!(rule.idx, regex_capture.rule_idx);
						self.current_log.all_matches.push(Match {
							rule_idx: rule.idx,
							sub_rule_id: Some(regex_capture.capture_id),
							parent_index: token_starting_capture_count + regex_capture.parent_index,
							range: CRange {
								start: token_start + regex_capture.range.start,
								end: token_start + regex_capture.range.end,
							},
							is_leaf: regex_capture.is_leaf,
							encoding_idx: regex_capture.maybe_encoding_idx.map(NonZero::from),
							ffi_pointers: MatchFfiPointers::NULL,
						});
						if regex_capture.is_leaf {
							self.current_log.leaf_indices.push(capture_index);
						}
					}

					if &*rule.name == "header" && previous_was_newline {
						if self.seen_header {
							assert_eq!(self.pending_header.message.len(), 0);
							assert_eq!(self.pending_header.all_matches.len(), 0);
							assert_eq!(self.pending_header.root_indices.len(), 0);
							assert_eq!(self.pending_header.leaf_indices.len(), 0);
							self.pending_header.message.push_str(lexeme);
							for mut capture in self
								.current_log
								.all_matches
								.drain(token_starting_capture_count..)
							{
								capture.range.start -= token_start;
								capture.range.end -= token_start;
								capture.parent_index -= token_starting_capture_count;
								self.pending_header.all_matches.push(capture);
							}
							for mut index in self
								.current_log
								.leaf_indices
								.drain(token_starting_leaf_indices..)
							{
								index -= token_starting_capture_count;
								self.pending_header.leaf_indices.push(index);
							}
							self.current_log.root_indices.pop().unwrap();
							self.pending_header.root_indices.push(0);
							break pos_before_token;
						} else if token_start == 0 {
							self.seen_header = true;
						}
					}
				},
				Token::Newline => {
					if !self.seen_header {
						break *pos;
					}
					previous_was_newline = true;
					continue;
				},
				Token::StaticText(static_text) => {
					assert!(!static_text.is_empty());
				},
				Token::EndOfInput => {
					assert_eq!(*pos, input.len());
					break *pos;
				},
			}
			previous_was_newline = false;
		};

		self.current_log.message.push_str(unsafe {
			// SAFETY: validated by the lexer.
			std::str::from_utf8_unchecked(&input[pos_after_header..pos_end])
		});

		let matches_base: *const Match = self.current_log.all_matches.as_ptr();
		for (i, mat) in self.current_log.all_matches.iter_mut().enumerate() {
			mat.ffi_pointers.parent = if mat.parent_index != i {
				// The "more optimizable" pointer primitive `add` is technically ok here,
				// but is/would need to be marked `unsafe`.
				matches_base.wrapping_add(mat.parent_index)
			} else {
				std::ptr::null()
			};
			mat.ffi_pointers.lexeme =
				UncheckedCArray::new(&self.current_log.message[mat.range.start..mat.range.end]);

			let rule_info: &RuleInfo = &self.spec[mat.rule_idx][mat.sub_rule_id];
			mat.ffi_pointers.rule_name = UncheckedCArray::new(&rule_info.name);
			mat.ffi_pointers.fully_qualified_name =
				UncheckedCArray::new(&rule_info.fully_qualified_name);
		}

		Some(LogEvent {
			spec: &self.spec,
			message: CUtf8::new(&self.current_log.message),
			all_matches: CArray::from_slice(&self.current_log.all_matches),
			root_indices: CArray::from_slice(&self.current_log.root_indices),
			leaf_indices: CArray::from_slice(&self.current_log.leaf_indices),
		})
	}

	/// Reset the internal state of the parser;
	/// used when "refilling" the input buffer.
	pub fn reset(&mut self) {
		self.lexer.reset();
		self.current_log.clear();
		self.seen_header = false;
		self.pending_header.clear();
	}
}

impl WorkingLogEvent {
	fn new() -> Self {
		Self {
			message: String::new(),
			all_matches: Vec::new(),
			root_indices: Vec::new(),
			leaf_indices: Vec::new(),
		}
	}

	fn clear(&mut self) {
		self.message.clear();
		self.all_matches.clear();
		self.leaf_indices.clear();
		self.root_indices.clear();
	}
}
