use std::ops::Range;

use crate::lexer::Lexer;
use crate::lexer::Token;
use crate::log_type::LogType;
use crate::schema::Schema;

#[derive(Debug)]
pub struct Parser {
	lexer: Lexer,
	maybe_pending_header: Option<PendingHeader>,
	current_log: String,
	current_captures: Vec<WorkingCapture>,
	current_variables: Vec<WorkingVariable>,
}

#[derive(Debug)]
pub struct LogEventView<'parser> {
	pub log_type: LogType,
	pub variables: Vec<Variable<'parser>>,
	pub have_header: bool,
}

#[derive(Debug, Clone)]
pub struct Variable<'parser> {
	pub rule: usize,
	pub name: &'parser str,
	pub lexeme: &'parser str,
	pub captures: Vec<Capture<'parser>>,
}

#[derive(Debug, Clone)]
pub struct Capture<'a> {
	pub name: &'a str,
	pub lexeme: &'a str,
}

#[derive(Debug)]
struct PendingHeader {
	rule: usize,
	lexeme: String,
	captures: Vec<WorkingCapture>,
}

#[derive(Debug)]
struct WorkingVariable {
	rule: usize,
	range: Range<usize>,
	captures: Range<usize>,
}

#[derive(Debug)]
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
			current_captures: Vec::new(),
			current_variables: Vec::new(),
		}
	}

	pub fn next_event(&mut self, input: &str, pos: &mut usize) -> Option<LogEventView<'_>> {
		if *pos == input.len() {
			return None;
		}

		let original_pos: usize = *pos;

		let mut static_text: String = String::new();
		let mut working_variable_types: Vec<(usize, String)> = Vec::new();
		let mut pending_static_text: usize = 0;

		self.current_log.clear();
		self.current_captures.clear();
		self.current_variables.clear();

		let mut have_header: bool = false;

		if let Some(header) = &mut self.maybe_pending_header {
			std::mem::swap(&mut header.lexeme, &mut self.current_log);
			std::mem::swap(&mut header.captures, &mut self.current_captures);
			self.current_variables.push(WorkingVariable {
				rule: header.rule,
				range: 0..self.current_log.len(),
				captures: 0..self.current_captures.len(),
			});

			working_variable_types.push((0, self.lexer.rule_name(header.rule).to_owned()));

			have_header = true;
		}

		let mut previous_was_newline: bool = false;

		loop {
			let old_pos: usize = *pos;
			let old_captures: usize = self.current_captures.len();
			match self.lexer.next_token(input, pos, |tag, start, end| {
				self.current_captures.push(WorkingCapture { tag, range: start..end });
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
							.extend(self.current_captures.drain(old_captures..));
					} else {
						self.current_variables.push(WorkingVariable {
							rule,
							range: (old_pos - original_pos)..(*pos - original_pos),
							captures: old_captures..self.current_captures.len(),
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

		for variable in self.current_variables.iter() {
			variables.push(Variable {
				rule: variable.rule,
				name: self.lexer.rule_name(variable.rule),
				lexeme: &self.current_log[variable.range.clone()],
				captures: self.current_captures[variable.captures.clone()]
					.iter()
					.map(|capture| Capture {
						name: self.lexer.capture_name(capture.tag),
						lexeme: &self.current_log[capture.range.clone()],
					})
					.collect::<Vec<_>>(),
			});
		}

		Some(LogEventView {
			log_type: LogType::new(static_text, working_variable_types),
			variables,
			have_header,
		})
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::regex::Regex;

	#[test]
	fn hmmm() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("hello", Regex::from_pattern("hello world").unwrap());
		schema.add_rule("bye", Regex::from_pattern("goodbye").unwrap());

		let mut parser: Parser = Parser::new(schema);

		parser.next_event("hello", &mut 0);
		parser.next_event("hello", &mut 0);
		parser.next_event("hello", &mut 0);
	}
}
