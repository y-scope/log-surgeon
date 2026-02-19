use std::num::NonZero;
use std::ops::Range;

use crate::lexer::Lexer;
use crate::lexer::Token;
use crate::log_type::LogType;
use crate::nfa::AutomataCapture;
use crate::schema::Schema;

#[derive(Debug, Clone)]
pub struct Parser {
	lexer: Lexer,
	maybe_pending_header: Option<PendingHeader>,
	current_log: String,
	working_captures: Vec<WorkingCapture>,
	working_variables: Vec<WorkingVariable>,
	// working_data: WorkingData,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LogEvent<'parser> {
	pub log_type: LogType,
	pub variables: Vec<Variable<'parser>>,
	pub have_header: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Variable<'parser> {
	pub rule: usize,
	pub name: &'parser str,
	pub lexeme: &'parser str,
	pub captures: Vec<Capture<'parser>>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Capture<'a> {
	pub name: &'a str,
	pub lexeme: &'a str,
	pub id: NonZero<u32>,
	pub parent_id: Option<NonZero<u32>>,
	pub is_leaf: bool,
}

#[derive(Debug, Clone)]
struct PendingHeader {
	rule: usize,
	lexeme: String,
	captures: Vec<WorkingCapture>,
}

#[derive(Debug, Clone)]
struct WorkingVariable {
	rule: usize,
	range: Range<usize>,
	captures: Range<usize>,
}

#[derive(Debug, Clone)]
struct WorkingCapture {
	tag: usize,
	range: Range<usize>,
}

#[derive(Debug)]
struct WorkingVariableType {
	rule: String,
	offset: usize,
}

impl Parser {
	pub fn new(schema: Schema) -> Self {
		let lexer: Lexer = Lexer::new(schema);
		Self {
			lexer,
			maybe_pending_header: None,
			current_log: String::new(),
			working_captures: Vec::new(),
			working_variables: Vec::new(),
		}
	}

	pub fn next_event(&mut self, input: &str, pos: &mut usize) -> Option<LogEvent<'_>> {
		if *pos == input.len() {
			return None;
		}

		let original_pos: usize = *pos;

		let mut static_text: String = String::new();
		let mut working_variable_types: Vec<(usize, String)> = Vec::new();
		let mut pending_static_text: usize = 0;

		self.current_log.clear();
		self.working_captures.clear();
		self.working_variables.clear();

		let mut have_header: bool = false;

		if let Some(header) = &mut self.maybe_pending_header {
			std::mem::swap(&mut header.lexeme, &mut self.current_log);
			std::mem::swap(&mut header.captures, &mut self.working_captures);
			self.working_variables.push(WorkingVariable {
				rule: header.rule,
				range: 0..self.current_log.len(),
				captures: 0..self.working_captures.len(),
			});

			working_variable_types.push((0, self.lexer.rule_name(header.rule).to_owned()));

			have_header = true;
		}

		let mut previous_was_newline: bool = false;

		loop {
			let old_pos: usize = *pos;
			let old_captures: usize = self.working_captures.len();
			match self.lexer.next_token(input, pos, |tag, _, start, end| {
				self.working_captures.push(WorkingCapture {
					tag,
					range: (old_pos - original_pos + start)..(old_pos - original_pos + end),
				});
			}) {
				Token::Variable { rule, name, lexeme } => {
					if name == "newline" {
						pending_static_text += lexeme.len();
						if !have_header {
							break;
						}
						previous_was_newline = true;
						continue;
					}

					if pending_static_text > 0 {
						static_text += &input[(old_pos - pending_static_text)..old_pos];
						pending_static_text = 0;
					}

					if name == "header" && (!have_header || previous_was_newline) {
						let pending_header: &mut PendingHeader =
							self.maybe_pending_header.get_or_insert(PendingHeader {
								rule: 0,
								lexeme: String::new(),
								captures: Vec::new(),
							});
						pending_header.rule = rule;
						assert_eq!(pending_header.lexeme.len(), 0);
						assert_eq!(pending_header.captures.len(), 0);
						pending_header.lexeme.push_str(lexeme);
						pending_header
							.captures
							.extend(self.working_captures.drain(old_captures..));
					} else {
						self.working_variables.push(WorkingVariable {
							rule,
							range: (old_pos - original_pos)..(*pos - original_pos),
							captures: old_captures..self.working_captures.len(),
						});
						working_variable_types.push((static_text.len(), name.to_owned()));
					}
				},
				Token::StaticText(static_text) => {
					assert_ne!(static_text, "");
					pending_static_text += static_text.len();
				},
				Token::EndOfInput => {
					assert_eq!(*pos, input.len());
					break;
				},
			}
			previous_was_newline = false;
		}

		if pending_static_text > 0 {
			static_text += &input[(*pos - pending_static_text)..*pos];
		}

		self.current_log.push_str(&input[original_pos..*pos]);

		let mut variables: Vec<Variable<'_>> = Vec::new();

		for variable in self.working_variables.iter() {
			variables.push(Variable {
				rule: variable.rule,
				name: self.lexer.rule_name(variable.rule),
				lexeme: &self.current_log[variable.range.clone()],
				captures: self.working_captures[variable.captures.clone()]
					.iter()
					.map(|capture| {
						let info: &AutomataCapture = self.lexer.capture_info(capture.tag);
						Capture {
							name: &info.capture_info.name,
							lexeme: &self.current_log[capture.range.clone()],
							id: info.capture_info.id,
							parent_id: info.capture_info.parent_id,
							is_leaf: info.capture_info.descendents == 0,
						}
					})
					.collect::<Vec<_>>(),
			});
		}

		Some(LogEvent {
			log_type: LogType::new(static_text, working_variable_types),
			variables,
			have_header,
		})
	}

	fn next_event_internal<D, F, G, H, R>(
		&mut self,
		input: &str,
		pos: &mut usize,
		data: &mut D,
		mut on_variable: F,
		mut on_capture: G,
		done: H,
	) -> Option<R>
	where
		F: FnMut(usize, &str, &mut D),
		G: FnMut(usize, &str, usize, usize, &mut D),
		H: FnOnce(String, bool, &mut D) -> R,
	{
		if *pos == input.len() {
			return None;
		}

		let original_pos: usize = *pos;

		let mut static_text: String = String::new();
		let mut pending_static_text: usize = 0;

		self.current_log.clear();
		self.working_captures.clear();
		self.working_variables.clear();

		let mut have_header: bool = false;

		if let Some(header) = &mut self.maybe_pending_header {
			std::mem::swap(&mut header.lexeme, &mut self.current_log);
			std::mem::swap(&mut header.captures, &mut self.working_captures);
			self.working_variables.push(WorkingVariable {
				rule: header.rule,
				range: 0..self.current_log.len(),
				captures: 0..self.working_captures.len(),
			});

			assert_eq!(self.lexer.rule_name(header.rule), "header");
			on_variable(0, self.lexer.rule_name(header.rule), data);

			have_header = true;
		}

		let mut previous_was_newline: bool = false;

		loop {
			let old_pos: usize = *pos;
			let old_captures: usize = self.working_captures.len();
			match self.lexer.next_token(input, pos, |tag, lexeme, start, end| {
				on_capture(tag, lexeme, start, end, data);
			}) {
				Token::Variable { rule, name, lexeme } => {
					if name == "newline" {
						pending_static_text += lexeme.len();
						if !have_header {
							break;
						}
						previous_was_newline = true;
						continue;
					}

					if pending_static_text > 0 {
						static_text += &input[(old_pos - pending_static_text)..old_pos];
						pending_static_text = 0;
					}

					if name == "header" && (!have_header || previous_was_newline) {
						let pending_header: &mut PendingHeader =
							self.maybe_pending_header.get_or_insert(PendingHeader {
								rule: 0,
								lexeme: String::new(),
								captures: Vec::new(),
							});
						pending_header.rule = rule;
						assert_eq!(pending_header.lexeme.len(), 0);
						assert_eq!(pending_header.captures.len(), 0);
						pending_header.lexeme.push_str(lexeme);
						pending_header
							.captures
							.extend(self.working_captures.drain(old_captures..));
					} else {
						self.working_variables.push(WorkingVariable {
							rule,
							range: (old_pos - original_pos)..(*pos - original_pos),
							captures: old_captures..self.working_captures.len(),
						});
						on_variable(static_text.len(), name, data);
					}
				},
				Token::StaticText(static_text) => {
					assert_ne!(static_text, "");
					pending_static_text += static_text.len();
				},
				Token::EndOfInput => {
					assert_eq!(*pos, input.len());
					break;
				},
			}
			previous_was_newline = false;
		}

		if pending_static_text > 0 {
			static_text += &input[(*pos - pending_static_text)..*pos];
		}

		self.current_log.push_str(&input[original_pos..*pos]);

		let mut variables: Vec<Variable<'_>> = Vec::new();

		for variable in self.working_variables.iter() {
			variables.push(Variable {
				rule: variable.rule,
				name: self.lexer.rule_name(variable.rule),
				lexeme: &self.current_log[variable.range.clone()],
				captures: self.working_captures[variable.captures.clone()]
					.iter()
					.map(|capture| {
						let info: &AutomataCapture = self.lexer.capture_info(capture.tag);
						Capture {
							name: &info.capture_info.name,
							lexeme: &self.current_log[capture.range.clone()],
							id: info.capture_info.id,
							parent_id: info.capture_info.parent_id,
							is_leaf: info.capture_info.descendents == 0,
						}
					})
					.collect::<Vec<_>>(),
			});
		}

		Some(done(static_text, have_header, data))
	}
}

impl<'a> LogEvent<'a> {
	pub fn blank() -> Self {
		Self {
			log_type: LogType::new(String::new(), Vec::new()),
			variables: Vec::new(),
			have_header: false,
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::regex::Regex;

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

			assert_eq!(event.variables[0].name, "hello");
			assert_eq!(event.variables[0].lexeme, "def");
			assert_eq!(event.variables[0].captures[0].name, "foo");
			assert_eq!(event.variables[0].captures[0].lexeme, "e");
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
