use std::str::Chars;

use crate::dfa::CompressedDfa;
use crate::dfa::JittedDfa;
use crate::dfa::MatchedRule;
use crate::dfa::TdfaExecution;
use crate::schema::RootRule;
use crate::schema::RuleIdx;
use crate::schema::Schema;

#[derive(Debug, Eq, PartialEq)]
pub enum Token<'schema, 'input> {
	Variable {
		rule: &'schema RootRule,
		lexeme: &'input str,
		has_captures: bool,
	},
	Newline,
	StaticText(&'input str),
	EndOfInput,
}

impl Schema {
	pub fn next_token<'schema, 'input>(
		&'schema self,
		input: &'input str,
		pos: &mut usize,
		last_was_delimited: u32,
		data: &mut TdfaExecution,
		jitted_dfa: JittedDfa,
		compressed: &CompressedDfa,
	) -> Token<'schema, 'input> {
		let start: usize = *pos;

		/*
		for (offset, ch) in input[start..].char_indices() {
			if let Some(MatchedRule { rule_idx, lexeme }) =
				self.execute_dfa(input, pos + offset, last_was_delimited, jitted_dfa)
			{
				let rule: &RootRule = &self[rule_idx];
				let has_captures: bool = rule.has_captures();
				data.clear();
				if has_captures {
					let matched: bool = rule.dfa.execute_with_captures(lexeme, data, rule.idx);
					assert!(matched);
				}
				*pos += offset + lexeme.len();
				return Token::Variable {
					rule,
					lexeme,
					has_captures,
				};
			} else if ch == '\n' {
				*pos += ch.len_utf8();
				return Token::Newline;
			}
		}
		*pos = input.len();
		return Token::EndOfInput;
		*/

		if start == input.len() {
			return Token::EndOfInput;
		}

		if let Some(MatchedRule { rule_idx, lexeme }) =
			self.execute_dfa::<false>(&input[start..], last_was_delimited, jitted_dfa, compressed)
		{
			let rule: &RootRule = &self[rule_idx];
			let has_captures: bool = rule.has_captures();
			data.clear();
			if has_captures {
				let matched: bool = rule.dfa.execute_with_captures(lexeme, data, rule.idx);
				assert!(matched);
			}
			*pos += lexeme.len();
			Token::Variable {
				rule,
				lexeme,
				has_captures,
			}
		} else {
			let mut chars: Chars<'_> = input[start..].chars();
			// We checked for `start == input.len()` above.
			let first: char = chars.next().unwrap();
			*pos += first.len_utf8();
			if first == '\n' {
				return Token::Newline;
			} else if !self.is_delimiter(first) {
				self.glob_static_text(input, pos);
			}
			Token::StaticText(&input[start..*pos])
		}
	}

	fn execute_dfa<'input, const JIT: bool>(
		&self,
		input: &'input str,
		last_was_delimited: u32,
		jitted_dfa: JittedDfa,
		compressed: &CompressedDfa,
	) -> Option<MatchedRule<'input>> {
		if JIT {
			let input: std::ops::Range<*const u8> = input.as_bytes().as_ptr_range();
			let mut end: *const u8 = std::ptr::null();

			let rule_idx: RuleIdx = jitted_dfa(input.start, input.end, last_was_delimited, &mut end)?;
			let lexeme: &str = unsafe {
				let start: *const u8 = input.start;
				let len: isize = end.offset_from(start);
				assert!(len >= 0);
				let bytes: &[u8] = std::slice::from_raw_parts(start, len as usize);
				std::str::from_utf8_unchecked(bytes)
			};
			Some(MatchedRule { rule_idx, lexeme })
		} else {
			// self.main_dfa.execute_without_captures(input, last_was_delimited)
			compressed.execute(input, last_was_delimited)
		}
	}

	fn glob_static_text(&self, input: &str, pos: &mut usize) {
		for ch in input[*pos..].chars() {
			if ch == '\n' {
				break;
			}
			*pos += ch.len_utf8();
			if self.is_delimiter(ch) {
				break;
			}
		}
	}

	fn is_delimiter(&self, ch: char) -> bool {
		if let Ok(i) = u8::try_from(ch)
			&& let Some(ch_is_delimiter) = self.ascii_delimiters.get(usize::from(i))
		{
			*ch_is_delimiter
		} else {
			self.non_ascii_delimiters.contains(ch)
		}
	}
}
