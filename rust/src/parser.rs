use crate::dfa::TdfaExecution;
use crate::lexer::Lexer;
use crate::lexer::Token;
use crate::log_event::Capture;
use crate::log_event::CaptureFfiPointers;
use crate::log_event::CapturePointerLength;
use crate::log_event::CaptureRange;
use crate::log_event::LogEvent;
use crate::log_type::LogType;
use crate::schema::Schema;

#[derive(Debug, Clone)]
pub struct Parser {
	pub lexer: Lexer,
	current_log: WorkingLogEvent,
	maybe_pending_header: Option<WorkingLogEvent>,
	dfa_execution: TdfaExecution,
}

#[derive(Debug, Clone)]
struct WorkingLogEvent {
	message: String,
	all_captures: Vec<Capture>,
	leaf_indices: Vec<usize>,
	variable_indices: Vec<usize>,
}

impl Parser {
	pub fn new(schema: Schema) -> Self {
		let lexer: Lexer = Lexer::new(schema);
		let dfa_execution: TdfaExecution = lexer.dfa.execution_data();
		Self {
			lexer,
			current_log: WorkingLogEvent::new(),
			maybe_pending_header: None,
			dfa_execution,
		}
	}

	pub fn next_event(&mut self, input: &str, pos: &mut usize) -> Option<LogEvent<'_>> {
		if *pos == input.len() {
			return None;
		}

		let pos_after_header: usize = *pos;

		self.current_log.clear();

		let mut have_header: bool = false;
		if let Some(header) = &mut self.maybe_pending_header {
			std::mem::swap(header, &mut self.current_log);
			have_header = true;
		}

		let header_len: usize = self.current_log.message.len();

		let mut previous_was_newline: bool = false;

		// Simulates whether we can match a start-anchored pattern.
		// Currently, the start-anchor just means "must come after static text".
		let mut last_was_delimited: u32 = u32::from(self.lexer.schema.anchor_ch);

		let pos_end: usize = loop {
			let pos_before_token: usize = *pos;
			let token_start: usize = pos_before_token - pos_after_header + header_len;
			let token_starting_capture_count: usize = self.current_log.all_captures.len();
			let token_starting_leaf_indices: usize = self.current_log.leaf_indices.len();
			match self
				.lexer
				.next_token(input, pos, last_was_delimited, &mut self.dfa_execution)
			{
				Token::Variable {
					rule,
					lexeme,
					has_captures,
				} => {
					let name: &str = &self.lexer.schema[rule].name;

					let variable_is_implicit_capture: bool = !has_captures || self.dfa_execution.captures.is_empty();

					let variable_index: usize = self.current_log.all_captures.len();
					let variable_capture: Capture = Capture {
						rule_idx: rule,
						capture_id: None,
						parent_id: None,
						parent_index: usize::MAX,
						range: CaptureRange {
							start: token_start,
							end: token_start + lexeme.len(),
						},
						is_leaf: variable_is_implicit_capture,
						ffi_pointers: CaptureFfiPointers::NULL,
					};

					self.current_log.all_captures.push(variable_capture);
					if variable_is_implicit_capture {
						self.current_log.leaf_indices.push(variable_index);
					}
					self.current_log.variable_indices.push(variable_index);

					for regex_capture in self.dfa_execution.captures.iter() {
						let capture_index: usize = self.current_log.all_captures.len();
						self.current_log.all_captures.push(Capture {
							rule_idx: regex_capture.rule,
							capture_id: Some(regex_capture.capture_id),
							parent_id: regex_capture.parent_id,
							parent_index: token_starting_capture_count.saturating_add(regex_capture.parent_index),
							range: CaptureRange {
								start: token_start + regex_capture.begin,
								end: token_start + regex_capture.end,
							},
							is_leaf: regex_capture.is_leaf,
							ffi_pointers: CaptureFfiPointers::NULL,
						});
						if regex_capture.is_leaf {
							self.current_log.leaf_indices.push(capture_index);
						}
					}

					if name == "header" && (previous_was_newline || (!have_header && token_start > 0)) {
						let pending_header: &mut WorkingLogEvent =
							self.maybe_pending_header.get_or_insert_with(WorkingLogEvent::new);
						assert_eq!(pending_header.message.len(), 0);
						assert_eq!(pending_header.all_captures.len(), 0);
						assert_eq!(pending_header.leaf_indices.len(), 0);
						assert_eq!(pending_header.variable_indices.len(), 0);
						pending_header.message.push_str(lexeme);
						for mut capture in self.current_log.all_captures.drain(token_starting_capture_count..) {
							capture.range.start -= token_start;
							capture.range.end -= token_start;
							if capture.parent_index != usize::MAX {
								capture.parent_index -= token_starting_capture_count;
							}
							pending_header.all_captures.push(capture);
						}
						for mut index in self.current_log.leaf_indices.drain(token_starting_leaf_indices..) {
							index -= token_starting_capture_count;
							pending_header.leaf_indices.push(index);
						}
						self.current_log.variable_indices.pop().unwrap();
						pending_header.variable_indices.push(0);
						break pos_before_token;
					} else {
						last_was_delimited = 0;
					}
				},
				Token::Newline => {
					if !have_header {
						break *pos;
					}
					previous_was_newline = true;
					last_was_delimited = u32::from('\n');
					continue;
				},
				Token::StaticText(static_text) => {
					assert!(!static_text.is_empty());
					last_was_delimited = u32::from(self.lexer.schema.anchor_ch);
				},
				Token::EndOfInput => {
					assert_eq!(*pos, input.len());
					break *pos;
				},
			}
			previous_was_newline = false;
		};

		self.current_log.message.push_str(&input[pos_after_header..pos_end]);

		let captures_base: *const Capture = self.current_log.all_captures.as_ptr();
		for capture in self.current_log.all_captures.iter_mut() {
			// The "more optimizable" pointer primitive `add` is technically ok here,
			// but is/would need to be marked `unsafe`.
			capture.ffi_pointers.parent = captures_base.wrapping_add(capture.parent_index);
			capture.ffi_pointers.lexeme =
				CapturePointerLength::from_str(&self.current_log.message[capture.range.start..capture.range.end]);
			capture.ffi_pointers.variable_name =
				CapturePointerLength::from_str(&self.lexer.schema[capture.rule_idx].name);
			capture.ffi_pointers.capture_name = CapturePointerLength::from_str(
				&self.lexer.schema[capture.rule_idx]
					.capture_info(capture.capture_id)
					.name,
			);
		}

		Some(LogEvent {
			log_type: LogType::new(
				&self.lexer.schema,
				&self.current_log.message,
				self.current_log
					.leaf_indices
					.iter()
					.map(|&i| &self.current_log.all_captures[i]),
			),
			message: &self.current_log.message,
			all_captures: &self.current_log.all_captures,
			leaf_indices: &self.current_log.leaf_indices,
			variable_indices: &self.current_log.variable_indices,
		})
	}
}

impl WorkingLogEvent {
	fn new() -> Self {
		Self {
			message: String::new(),
			all_captures: Vec::new(),
			leaf_indices: Vec::new(),
			variable_indices: Vec::new(),
		}
	}

	fn clear(&mut self) {
		self.message.clear();
		self.all_captures.clear();
		self.leaf_indices.clear();
		self.variable_indices.clear();
	}
}

/*
#[cfg(test)]
mod test {
	use std::num::NonZero;

	use super::*;

	#[test]
	fn hmmm() {
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", "abc|d(?<foo>[a-z])f").unwrap();

		let mut parser: Parser = Parser::new(schema);
		let mut pos: usize = 0;

		let input: &str = "def foobarbaz";

		{
			let event: LogEvent<'_> = parser.next_event(input, &mut pos).unwrap();
			assert_eq!(pos, input.len());

			assert_eq!(event.leaf_captures[0].rule_idx, 1);
			assert_eq!(event.leaf_captures[0].capture_id, Some(NonZero::<u32>::MIN));
			assert_eq!(event.leaf_captures[0].range, (1, 2));
		}
		{
			assert_eq!(parser.next_event(input, &mut pos), None);
		}
	}

	#[test]
	fn hmmm2() {
		let mut schema: Schema = Schema::new();
		schema.add_rule("number", "[0-9]+").unwrap();
		schema
			.add_rule(
				"username",
				r"@(?<inside>[a-z]+)(?<parts>(?<dot>\.)[a-z]*(?<end>[a-z]))*",
			)
			.unwrap();

		let mut parser: Parser = Parser::new(schema);
		let mut pos: usize = 0;

		let input: &str = "\n123 awesrgesrgesrg 6346346 @someone foo@username @someone.foo.bar.baz\n";

		{
			let event: LogEvent<'_> = parser.next_event(input, &mut pos).unwrap();
			assert_eq!(pos, 1);

			assert_eq!(event.log_type.as_str(), "\n");
			assert_eq!(event.variables.len(), 0);
		}
		{
			let event: LogEvent<'_> = parser.next_event(input, &mut pos).unwrap();
			assert_eq!(pos, input.len());

			assert_eq!(
				event.log_type.as_str(),
				"%number% awesrgesrgesrg %number% %username% foo@username %username%\n"
			);

			assert_eq!(event.variables[0].name, "number");
			assert_eq!(event.variables[0].lexeme, "123");

			assert_eq!(event.variables[1].name, "number");
			assert_eq!(event.variables[1].lexeme, "6346346");

			assert_eq!(event.variables[2].name, "username");
			assert_eq!(event.variables[2].lexeme, "@someone");

			assert_eq!(event.variables[3].name, "username");
			assert_eq!(event.variables[3].lexeme, "@someone.foo.bar.baz");
			assert_eq!(event.variables[3].captures[0].name, "inside");
			assert_eq!(event.variables[3].captures[0].lexeme, "someone");
			assert_eq!(event.variables[3].captures[1].name, "parts");
			assert_eq!(event.variables[3].captures[1].lexeme, ".foo");
			assert_eq!(event.variables[3].captures[2].name, "parts");
			assert_eq!(event.variables[3].captures[2].lexeme, ".bar");
			assert_eq!(event.variables[3].captures[3].name, "parts");
			assert_eq!(event.variables[3].captures[3].lexeme, ".baz");
			assert_eq!(event.variables[3].captures[4].name, "dot");
			assert_eq!(event.variables[3].captures[4].lexeme, ".");
			assert_eq!(event.variables[3].captures[5].name, "dot");
			assert_eq!(event.variables[3].captures[5].lexeme, ".");
			assert_eq!(event.variables[3].captures[6].name, "dot");
			assert_eq!(event.variables[3].captures[6].lexeme, ".");
			assert_eq!(event.variables[3].captures[7].name, "end");
			assert_eq!(event.variables[3].captures[7].lexeme, "o");
			assert_eq!(event.variables[3].captures[8].name, "end");
			assert_eq!(event.variables[3].captures[8].lexeme, "r");
			assert_eq!(event.variables[3].captures[9].name, "end");
			assert_eq!(event.variables[3].captures[9].lexeme, "z");
		}
		{
			assert_eq!(parser.next_event(input, &mut pos), None);
		}
	}
}
*/
