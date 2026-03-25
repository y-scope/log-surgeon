use crate::lexer::Lexer;
use crate::lexer::Token;
use crate::log_event::Capture;
use crate::log_event::LogEvent;
use crate::log_type::LogType;
use crate::schema::Schema;

#[derive(Debug, Clone)]
pub struct Parser {
	pub lexer: Lexer,
	current_log: WorkingLogEvent,
	maybe_pending_header: Option<WorkingLogEvent>,
}

#[derive(Debug, Clone)]
struct WorkingLogEvent {
	message: String,
	leaf_captures: Vec<Capture>,
	all_captures: Vec<Capture>,
	variables: Vec<Capture>,
}

impl Parser {
	pub fn new(schema: Schema) -> Self {
		let lexer: Lexer = Lexer::new(schema);
		Self {
			lexer,
			current_log: WorkingLogEvent::new(),
			maybe_pending_header: None,
		}
	}

	pub fn next_event(&mut self, input: &str, pos: &mut usize) -> Option<LogEvent<'_>> {
		if *pos == input.len() {
			return None;
		}

		let original_pos: usize = *pos;

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
		loop {
			let token_start: usize = *pos;
			let token_starting_capture_count: usize = self.current_log.leaf_captures.len();
			let log_event_start: usize = token_start - original_pos + header_len;
			match self.lexer.next_token(input, pos, last_was_delimited, |regex_capture| {
				let generalized_capture: Capture = Capture {
					rule_idx: regex_capture.rule,
					capture_id: Some(regex_capture.capture_id),
					parent_id: regex_capture.parent_id,
					range: (
						log_event_start + regex_capture.start,
						log_event_start + regex_capture.end,
					),
					is_leaf: regex_capture.is_leaf,
				};
				if regex_capture.is_leaf {
					self.current_log.leaf_captures.push(generalized_capture.clone());
				}
				self.current_log.all_captures.push(generalized_capture);
			}) {
				Token::Variable {
					rule,
					lexeme,
					has_captures,
				} => {
					let name: &str = &self.lexer.schema[rule].name;

					if !has_captures || (token_starting_capture_count == self.current_log.leaf_captures.len()) {
						self.current_log.leaf_captures.push(Capture {
							rule_idx: rule,
							capture_id: None,
							parent_id: None,
							range: (log_event_start, log_event_start + lexeme.len()),
							is_leaf: true,
						})
					}

					let variable_capture: Capture = Capture {
						rule_idx: rule,
						capture_id: None,
						parent_id: None,
						range: (log_event_start, *pos - original_pos + header_len),
						is_leaf: false,
					};
					self.current_log.all_captures.push(variable_capture.clone());

					if name == "header" && (!have_header || previous_was_newline) {
						let pending_header: &mut WorkingLogEvent =
							self.maybe_pending_header.get_or_insert_with(WorkingLogEvent::new);
						assert_eq!(pending_header.message.len(), 0);
						assert_eq!(pending_header.leaf_captures.len(), 0);
						pending_header.message.push_str(lexeme);
						pending_header
							.leaf_captures
							.extend(self.current_log.leaf_captures.drain(token_starting_capture_count..));
						pending_header.variables.push(variable_capture);
						break;
					} else {
						self.current_log.variables.push(variable_capture);
						last_was_delimited = 0;
					}
				},
				Token::Newline => {
					if !have_header {
						break;
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
					break;
				},
			}
			previous_was_newline = false;
		}

		self.current_log.message.push_str(&input[original_pos..*pos]);

		self.current_log.leaf_captures.sort_by_key(|capture| capture.range);

		// let mut variables: Vec<Variable<'_>> = Vec::new();

		// for variable in self.working_variables.iter() {
		// 	variables.push(Variable {
		// 		rule: variable.rule,
		// 		name: self.lexer.rule_name(variable.rule),
		// 		lexeme: &self.current_log[variable.range.clone()],
		// 		range: (variable.range.start, variable.range.end),
		// 		captures: self.working_captures[variable.captures.clone()]
		// 			.iter()
		// 			.map(|capture| {
		// 				let info: &AutomataCapture = self.lexer.capture_info(capture.tag);
		// 				Capture {
		// 					name: &info.capture_info.name,
		// 					lexeme: &self.current_log[capture.range.clone()],
		// 					range: (
		// 						capture.range.start - variable.range.start,
		// 						capture.range.end - variable.range.start,
		// 					),
		// 					id: info.capture_info.id,
		// 					parent_id: info.capture_info.parent_id,
		// 				}
		// 			})
		// 			.collect::<Vec<_>>(),
		// 	});
		// }

		Some(LogEvent {
			log_type: LogType::new(
				&self.lexer.schema,
				&self.current_log.message,
				&self.current_log.leaf_captures,
			),
			message: &self.current_log.message,
			leaf_captures: &self.current_log.leaf_captures,
			all_captures: &self.current_log.all_captures,
			variables: &self.current_log.variables,
		})
	}
}

impl WorkingLogEvent {
	fn new() -> Self {
		Self {
			message: String::new(),
			leaf_captures: Vec::new(),
			all_captures: Vec::new(),
			variables: Vec::new(),
		}
	}

	fn clear(&mut self) {
		self.message.clear();
		self.leaf_captures.clear();
		self.all_captures.clear();
		self.variables.clear();
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
