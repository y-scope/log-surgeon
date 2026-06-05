use crate::dfa::TdfaExecution;
use crate::ffi::UncheckedCArray;
use crate::lexing::Token;
use crate::log_event::LogEvent;
use crate::log_event::Match;
use crate::log_event::MatchFfiPointers;
use crate::log_type::LogType;
use crate::schema::RuleInfo;
use crate::schema::Schema;
use crate::utils::Range;

#[derive(Debug, Clone)]
pub struct Parser {
	pub schema: Schema,
	current_log: WorkingLogEvent,
	maybe_pending_header: Option<WorkingLogEvent>,
	dfa_execution: TdfaExecution,
}

#[derive(Debug, Clone)]
struct WorkingLogEvent {
	message: String,
	all_matches: Vec<Match>,
	leaf_indices: Vec<usize>,
	variable_indices: Vec<usize>,
}

impl Parser {
	pub fn new(schema: Schema) -> Self {
		let mut registers: usize = 0;
		let mut tags: usize = 0;
		for rule in schema.rules.iter() {
			registers = registers.max(rule.dfa.number_of_registers);
			tags = tags.max(rule.dfa.tags.len());
		}
		let dfa_execution: TdfaExecution = TdfaExecution::new(registers, tags);
		Self {
			schema,
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

		let mut previous_was_newline: bool = true;

		// Simulates whether we can match a start-anchored pattern.
		// Currently, the start-anchor just means "must come after static text".
		let mut last_was_delimited: u32 = u32::from(self.schema.anchor_ch);

		let pos_end: usize = loop {
			let pos_before_token: usize = *pos;
			let token_start: usize = pos_before_token - pos_after_header + header_len;
			let token_starting_capture_count: usize = self.current_log.all_matches.len();
			let token_starting_leaf_indices: usize = self.current_log.leaf_indices.len();
			match self
				.schema
				.next_token(input, pos, last_was_delimited, &mut self.dfa_execution)
			{
				Token::Variable {
					rule,
					lexeme,
					has_captures,
				} => {
					let name: &str = &rule.name;

					let variable_is_implicit_capture: bool = !has_captures;

					let variable_capture: Match = Match {
						rule_idx: rule.idx,
						sub_rule_id: None,
						parent_id: None,
						parent_index: token_starting_capture_count,
						range: Range {
							start: token_start,
							end: token_start + lexeme.len(),
						},
						is_leaf: variable_is_implicit_capture,
						encoding_idx: rule[None].encoding_idx,
						ffi_pointers: MatchFfiPointers::NULL,
					};

					self.current_log.all_matches.push(variable_capture);
					if variable_is_implicit_capture {
						self.current_log.leaf_indices.push(token_starting_capture_count);
					}
					self.current_log.variable_indices.push(token_starting_capture_count);

					for regex_capture in self.dfa_execution.captures.iter() {
						let capture_index: usize = self.current_log.all_matches.len();
						assert_eq!(rule.idx, regex_capture.rule_idx);
						self.current_log.all_matches.push(Match {
							rule_idx: rule.idx,
							sub_rule_id: Some(regex_capture.capture_id),
							parent_id: regex_capture.parent_id,
							parent_index: token_starting_capture_count + regex_capture.parent_index,
							range: Range {
								start: token_start + regex_capture.range.start,
								end: token_start + regex_capture.range.end,
							},
							is_leaf: regex_capture.is_leaf,
							encoding_idx: rule[Some(regex_capture.capture_id)].encoding_idx,
							ffi_pointers: MatchFfiPointers::NULL,
						});
						if regex_capture.is_leaf {
							self.current_log.leaf_indices.push(capture_index);
						}
					}

					last_was_delimited = 0;
					if name == "header" && previous_was_newline {
						if have_header {
							let pending_header: &mut WorkingLogEvent =
								self.maybe_pending_header.get_or_insert_with(WorkingLogEvent::new);
							assert_eq!(pending_header.message.len(), 0);
							assert_eq!(pending_header.all_matches.len(), 0);
							assert_eq!(pending_header.leaf_indices.len(), 0);
							assert_eq!(pending_header.variable_indices.len(), 0);
							pending_header.message.push_str(lexeme);
							for mut capture in self.current_log.all_matches.drain(token_starting_capture_count..) {
								capture.range.start -= token_start;
								capture.range.end -= token_start;
								capture.parent_index -= token_starting_capture_count;
								pending_header.all_matches.push(capture);
							}
							for mut index in self.current_log.leaf_indices.drain(token_starting_leaf_indices..) {
								index -= token_starting_capture_count;
								pending_header.leaf_indices.push(index);
							}
							self.current_log.variable_indices.pop().unwrap();
							pending_header.variable_indices.push(0);
							break pos_before_token;
						} else if token_start == 0 {
							have_header = true;
						}
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
					last_was_delimited = u32::from(self.schema.anchor_ch);
				},
				Token::EndOfInput => {
					assert_eq!(*pos, input.len());
					break *pos;
				},
			}
			previous_was_newline = false;
		};

		self.current_log.message.push_str(&input[pos_after_header..pos_end]);

		let matches_base: *const Match = self.current_log.all_matches.as_ptr();
		for mat in self.current_log.all_matches.iter_mut() {
			// The "more optimizable" pointer primitive `add` is technically ok here,
			// but is/would need to be marked `unsafe`.
			mat.ffi_pointers.parent = matches_base.wrapping_add(mat.parent_index);
			mat.ffi_pointers.lexeme = UncheckedCArray::new(&self.current_log.message[mat.range.start..mat.range.end]);

			let rule_info: &RuleInfo = &self.schema[mat.rule_idx][mat.sub_rule_id];
			mat.ffi_pointers.root_rule_name = UncheckedCArray::new(&rule_info.root_name);
			mat.ffi_pointers.rule_name = UncheckedCArray::new(if let Some(sub_rule) = &rule_info.maybe_sub_rule {
				&sub_rule.name
			} else {
				&rule_info.root_name
			});
			mat.ffi_pointers.fully_qualified_name = UncheckedCArray::new(&rule_info.fully_qualified_name);
		}

		Some(LogEvent {
			log_type: LogType::new(
				&self.schema,
				&self.current_log.message,
				self.current_log
					.leaf_indices
					.iter()
					.map(|&i| &self.current_log.all_matches[i]),
			),
			message: &self.current_log.message,
			all_matches: &self.current_log.all_matches,
			leaf_indices: &self.current_log.leaf_indices,
			variable_indices: &self.current_log.variable_indices,
		})
	}
}

impl WorkingLogEvent {
	fn new() -> Self {
		Self {
			message: String::new(),
			all_matches: Vec::new(),
			leaf_indices: Vec::new(),
			variable_indices: Vec::new(),
		}
	}

	fn clear(&mut self) {
		self.message.clear();
		self.all_matches.clear();
		self.leaf_indices.clear();
		self.variable_indices.clear();
	}
}
