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
		let scratch: LeftmostScratch = spec.combined_dfa.leftmost_scratch();
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

		match self.search(input, start, char_before) {
			None => Self::static_text_token(input, pos, input.len()),
			Some(m) if m.start == start => self.variable_token(input, pos, m, dfa_execution),
			Some(m) => {
				debug_assert!(m.start > start);
				self.pending = Some(m);
				Self::static_text_token(input, pos, m.start)
			},
		}
	}

	/// Leftmost-longest search for the next match at or after `pos`
	/// (see `docs/leftmost-longest-lexing.md`, design B): one fused scan.
	///
	/// [`ParsingSpec::scan`] finds delimiters and (when
	/// [`ParsingSpec::unanchored_scan`] is `Some`) first bytes of unanchored rules.
	/// Start-anchored rules are attempted at boundaries only
	/// (`pos` when `char_before` is a delimiter, else the position after each delimiter);
	/// candidates are probed in ascending order with single-threaded `execute`,
	/// which resolves longest match and priority per position.
	/// With unanchored rules whose first-byte set is dense or inexact,
	/// everything falls back to `execute_leftmost` over the combined DFA.
	///
	/// As in [`crate::dfa::FlattenedDfa::execute_leftmost`],
	/// no match may start past the position just after the first `'\n'` at or after `pos`;
	/// no match may start at end of input.
	/// A match may still *span* newlines (the searches do not cut it off).
	fn search(&mut self, input: &[u8], pos: usize, char_before: u32) -> Option<LeftmostMatch> {
		let spec: &ParsingSpec = &self.spec;
		if spec.unanchored_scan.is_none() && spec.has_unanchored_rules {
			return spec
				.combined_dfa
				.execute_leftmost(input, pos, char_before, &mut self.scratch);
		}

		let attempt = |s: usize, before: u32| -> Option<LeftmostMatch> {
			spec.combined_dfa
				.execute(&input[s..], before)
				.map(|m| LeftmostMatch {
					rule_idx: m.rule_idx,
					maybe_encoding_idx: m.maybe_encoding_idx,
					start: s,
					end: s + m.lexeme.len(),
				})
		};

		// Boundary probe at `pos` itself.
		if spec
			.delimiters
			.contains(char::from_u32(char_before).unwrap_or('\0'))
			&& let Some(m) = attempt(pos, char_before)
		{
			return Some(m);
		}

		let mut last_probed: usize = pos; // boundary probes must not repeat a position
		let mut i: usize = pos;
		loop {
			let hit: usize = spec.scan.find_in(&input[i..]).map(|rel| i + rel)?;
			let byte: u8 = input[hit];

			// Delimiter? Then the position after it is a boundary and is probed;
			// a newline also ends the search window (nothing may start past it).
			let maybe_delimiter_len: Option<usize> = if byte < 0x80 {
				spec.is_ascii_delimiter[usize::from(byte)].then_some(1)
			} else {
				// Maybe the lead byte of a multi-byte delimiter.
				let (ch, len) = crate::utils::utf8::decode_scalar(&input[hit..]);
				spec.delimiters.contains(ch).then_some(len)
			};

			match maybe_delimiter_len {
				Some(len) => {
					// A delimiter byte may also start an unanchored rule (e.g. `@...`);
					// probe the delimiter position first (it starts earlier).
					if spec
						.unanchored_scan
						.as_ref()
						.is_some_and(|scan| scan.contains(byte))
					{
						let before: u32 =
							u32::from(Utf8Chars::new(&input[..hit]).next_back().unwrap_or('\n'));
						if let Some(m) = attempt(hit, before) {
							return Some(m);
						}
					}
					let boundary: usize = hit + len;
					if boundary < input.len() && boundary != last_probed {
						last_probed = boundary;
						// The position after the delimiter is a boundary
						// (the delimiter is the lookbehind).
						let before: u32 = if len == 1 {
							u32::from(byte)
						} else {
							crate::utils::utf8::decode_scalar(&input[hit..]).0 as u32
						};
						if let Some(m) = attempt(boundary, before) {
							return Some(m);
						}
					}
					if byte == b'\n' {
						return None;
					}
				},
				// A non-delimiter hit is a first byte of an unanchored rule.
				None => {
					debug_assert!(
						spec.unanchored_scan
							.as_ref()
							.is_none_or(|s| s.contains(byte))
					);
					let before: u32 =
						u32::from(Utf8Chars::new(&input[..hit]).next_back().unwrap_or('\n'));
					if let Some(m) = attempt(hit, before) {
						return Some(m);
					}
				},
			}
			i = hit + 1;
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
