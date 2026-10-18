#[cfg(test)]
mod test;

use std::sync::Arc;

use crate::dfa::LeftmostMatch;
use crate::dfa::LeftmostScratch;
use crate::dfa::TdfaExecution;
use crate::parsing_spec::EncodingIdx;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::RootRule;
use crate::utils::utf8::Utf8Chars;

#[derive(Debug, Clone)]
pub struct Lexer {
	/// Strictly speaking, this field is unnecessary;
	/// `Parser` already has the spec and could pass it every time to `Lexer::next_token`.
	/// However, it's cheap and cleaner to clone it here for encapsulation.
	spec: Arc<ParsingSpec>,
	/// Reused working space for the leftmost-longest search.
	scratch: LeftmostScratch,
	/// A match found ahead of the current position by a search
	/// whose result has not been returned yet;
	/// the gap before it is served as static text (and newlines) first.
	pending: Option<LeftmostMatch>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Token<'spec, 'input> {
	Variable {
		rule: &'spec RootRule,
		maybe_encoding_idx: Option<EncodingIdx>,
		lexeme: &'input str,
		has_captures: bool,
	},
	Newline,
	StaticText(&'input str),
	EndOfInput,
}

impl Lexer {
	pub fn new(spec: Arc<ParsingSpec>) -> Self {
		let scratch: LeftmostScratch = spec.dfa_for_parsing.leftmost_scratch();
		Self {
			spec,
			scratch,
			pending: None,
		}
	}

	/// Drop any pending match; used when the parser's input buffer is refilled.
	pub fn reset(&mut self) {
		self.pending = None;
	}

	/// Return the next [`Token`] from `input` starting from `*pos`,
	/// and update `*pos` to index the next character after the returned token.
	///
	/// `dfa_execution` is cached memory/working space for the underlying DFA execution,
	/// and also contains the captured text for sub-rules.
	/// It should be created via [`TdfaExecution::new`].
	pub fn next_token<'spec, 'input>(
		&'spec mut self,
		input: &'input [u8],
		pos: &mut usize,
		dfa_execution: &mut TdfaExecution,
	) -> Token<'spec, 'input> {
		let start: usize = *pos;

		if let Some(pending) = self.pending {
			if start == pending.start {
				self.pending = None;
				return self.variable_token(input, pos, pending, dfa_execution);
			}
			debug_assert!(start < pending.start);
			return Self::static_text_token(input, pos, pending.start);
		}

		if start == input.len() {
			return Token::EndOfInput;
		}

		// A newline is emitted before considering the following text,
		// so no search ever starts *at* a newline.
		if input[start] == b'\n' {
			*pos += 1;
			return Token::Newline;
		}

		let char_before: u32 =
			u32::from(Utf8Chars::new(&input[..start]).next_back().unwrap_or('\n'));

		match self.spec.dfa_for_parsing.execute_leftmost(
			input,
			start,
			char_before,
			&mut self.scratch,
		) {
			None => Self::static_text_token(input, pos, input.len()),
			Some(m) if m.start == start => self.variable_token(input, pos, m, dfa_execution),
			Some(m) => {
				debug_assert!(m.start > start);
				self.pending = Some(m);
				Self::static_text_token(input, pos, m.start)
			},
		}
	}

	/// Emit [`Token::StaticText`] from `*pos` to `end`,
	/// splitting off (and returning) a [`Token::Newline`] first whenever the gap contains one.
	///
	/// `*pos` must index the start of a character strictly before `end`.
	/// All characters in `[*pos..end]` must have been decoded (validated)
	/// by the search that found the gap.
	///
	/// On return, `*pos` indexes the first character after the returned token.
	fn static_text_token<'spec, 'input>(
		input: &'input [u8],
		pos: &mut usize,
		end: usize,
	) -> Token<'spec, 'input> {
		let start: usize = *pos;
		debug_assert!(start < end);
		match input[start..end].iter().position(|&b| b == b'\n') {
			Some(0) => {
				*pos += 1;
				Token::Newline
			},
			Some(k) => {
				*pos += k;
				// SAFETY: `[start..*pos]` spans whole validated UTF-8 scalars
				// (see the doc comment above).
				Token::StaticText(unsafe { std::str::from_utf8_unchecked(&input[start..*pos]) })
			},
			None => {
				*pos = end;
				// SAFETY: as above.
				Token::StaticText(unsafe { std::str::from_utf8_unchecked(&input[start..*pos]) })
			},
		}
	}

	/// Emit the [`Token::Variable`] for a found match,
	/// extracting sub-rule captures into `dfa_execution` when the rule has any.
	///
	/// On return, `*pos` indexes the first character after the match.
	fn variable_token<'spec, 'input>(
		&'spec self,
		input: &'input [u8],
		pos: &mut usize,
		m: LeftmostMatch,
		dfa_execution: &mut TdfaExecution,
	) -> Token<'spec, 'input> {
		let rule: &RootRule = &self.spec[m.rule_idx];
		assert_eq!(rule.idx, m.rule_idx);

		// Even if we don't call [`Tdfa::execute_with_captures`], we need to clear any possible
		// captures from the previous call to [`Lexer::next_token`].
		dfa_execution.clear();

		let has_captures: bool = rule.has_captures();
		// SAFETY: `m.start` and `m.end` index character boundaries
		// (start position and last consumed lookahead, both from the search's decoding),
		// over input bytes that search already validated.
		let lexeme: &'input str = unsafe { std::str::from_utf8_unchecked(&input[m.start..m.end]) };
		if has_captures {
			let matched: bool = rule.dfa.execute_with_captures(lexeme, dfa_execution, rule);
			assert!(matched);
		}
		*pos = m.end;
		Token::Variable {
			rule,
			maybe_encoding_idx: m.maybe_encoding_idx,
			lexeme,
			has_captures,
		}
	}
}
