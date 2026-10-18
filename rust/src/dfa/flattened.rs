use crate::dfa::BackupState;
use crate::dfa::MatchedRule;
use crate::dfa::Tdfa;
use crate::interval_tree::Interval;
use crate::parsing_spec::EncodingIdx;
use crate::parsing_spec::RuleIdx;

/// Transition target states are stored as 16-bit values;
/// if the MSB is set, the target state is accepting
/// (has a non-`None` [`FlattenedDfa::accepts_for_rule`],
/// so there are 15 bits for the target state index.
///
/// This lets the hot loop test acceptance with a single load (the transition itself)
/// instead of a second dependent load into `accepts_for_rule` for every character.
const ACCEPTING_BIT: u16 = 1 << (u16::BITS - 1);
const STATE_MASK: u16 = !ACCEPTING_BIT;

const _: () = {
	assert!(STATE_MASK == i16::MAX as u16);
};

/// A dense table representation of a DFA (untagged);
/// a "struct of arrays" for improved data locality accessing transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlattenedDfa {
	all_intervals: Vec<Interval<u32>>,
	accepts_for_rule: Vec<Option<(RuleIdx, Option<EncodingIdx>)>>,
	/// Flattened `states.len() * 0x80` table of encoded transitions for ASCII bytes,
	/// indexed by `state * 0x80 + byte`.
	///
	/// Encoded values: target state index in the low bits,
	/// plus [`ACCEPTING_BIT`] if the target state is accepting.
	///
	/// `0`, normally indexing to the start state, means "no transition",
	/// since by construction there are no transitions to the start state.
	///
	/// It would make more sense to store these as `Vec<[u16; 0x80]>`,
	/// but Serde chokes on arrays...
	/// <https://github.com/serde-rs/serde/issues/1937>.
	ascii_transitions: Vec<u16>,
	non_ascii_transitions: Vec<u16>,
}

/// A leftmost-longest match over all rules, as returned by
/// [`FlattenedDfa::execute_leftmost`].
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct LeftmostMatch {
	pub rule_idx: RuleIdx,
	pub maybe_encoding_idx: Option<EncodingIdx>,
	/// Byte offset where the match starts.
	pub start: usize,
	/// Byte offset one past the matched text.
	pub end: usize,
}

/// Cached memory for [`FlattenedDfa::execute_leftmost`].
#[derive(Debug, Clone)]
pub struct LeftmostScratch {
	/// Live `(start position, state)` threads, sorted by ascending start position.
	threads: Vec<(usize, u16)>,
	/// `stamps[s] == next_mark - 1` iff a live thread occupies state `s` at the current
	/// position. Two threads in the same state at the same position are equivalent,
	/// so only the earliest-start one is kept (earliest start wins).
	///
	/// `next_mark` only ever increases,
	/// so stale entries (from earlier positions or earlier calls) never collide
	/// with the current position's mark.
	stamps: Vec<usize>,
	next_mark: usize,
}

impl Tdfa {
	/// Panics if the number of states is greater than [`STATE_MASK`]
	/// (`2 ** 15` or approximately 32k states as of 2026-10-06).
	pub fn flatten(&self) -> FlattenedDfa {
		assert!(
			self.states.len() <= usize::from(STATE_MASK),
			"too many DFA states to encode"
		);

		let mut all_intervals: Vec<Interval<u32>> = self.all_intervals();

		all_intervals.retain(|interval| interval.end() >= 0x80);
		if let Some(first) = all_intervals.first_mut() {
			if first.start() == 0x80 {
				*first = Interval::new(0x80, first.end());
			}
		}

		let mut accepts_for_rule: Vec<Option<(RuleIdx, Option<EncodingIdx>)>> =
			Vec::with_capacity(self.states.len());
		let mut ascii_transitions: Vec<u16> = vec![0; self.states.len() * 0x80];
		let mut non_ascii_transitions: Vec<u16> =
			Vec::with_capacity(self.states.len() * all_intervals.len());

		let encode = |target: usize| -> u16 {
			let state: u16 = u16::try_from(target).unwrap();
			if self.states[target].accepting_rule.is_some() {
				state | ACCEPTING_BIT
			} else {
				state
			}
		};

		for (state_index, state) in self.states.iter().enumerate() {
			accepts_for_rule.push(state.accepting_rule);
			let base: usize = state_index * 0x80;
			for i in 0..0x80usize {
				if let Some(transition) = state.transitions.lookup(u32::try_from(i).unwrap()) {
					assert_ne!(transition.target, 0);
					ascii_transitions[base + i] = encode(transition.target);
				}
			}
			for &interval in all_intervals.iter() {
				if let Some(transition) = state.transitions.lookup(interval.start()) {
					assert_ne!(transition.target, 0);
					non_ascii_transitions.push(encode(transition.target));
				} else {
					non_ascii_transitions.push(0);
				}
			}
		}

		FlattenedDfa {
			all_intervals,
			accepts_for_rule,
			ascii_transitions,
			non_ascii_transitions,
		}
	}
}

impl FlattenedDfa {
	/// All bytes that can start a match when the lookbehind character is `lookbehind`
	/// (the "startable" byte set for the prefilters of design B; exact).
	///
	/// State 0's transitions are *lookbehind* transitions
	/// (`^` = delimiter-only, else any char): `lookbehind` selects the post-anchor
	/// state, so a non-delimiter `lookbehind` selects exactly the unanchored rules.
	/// A byte is startable if that state has a transition on it;
	/// non-ASCII chars are reported via their UTF-8 lead bytes
	/// (lead bytes of same-length scalars advance monotonically,
	/// so each fixed-length interval sub-range contributes a contiguous range).
	pub fn initial_bytes(&self, lookbehind: u32) -> Vec<u8> {
		let Some(encoded) = self.lookup_next_state(0, lookbehind) else {
			return Vec::new();
		};
		let state: usize = usize::from(encoded & STATE_MASK);

		let mut out: Vec<u8> = Vec::new();
		let base: usize = state * 0x80;
		for byte in 0..0x80usize {
			if self.ascii_transitions[base + byte] != 0 {
				out.push(u8::try_from(byte).unwrap());
			}
		}
		let intervals_per_state: usize = self.all_intervals.len();
		let row_start: usize = state * intervals_per_state;
		if intervals_per_state > 0
			&& self.non_ascii_transitions[row_start..row_start + intervals_per_state]
				.iter()
				.any(|&target| target != 0)
		{
			for interval in self.all_intervals.iter() {
				for (lo, hi, len) in split_at_utf8_lengths(interval.start(), interval.end()) {
					out.extend(lead_byte_of(lo, len)..=lead_byte_of(hi, len));
				}
			}
		}
		out.sort_unstable();
		out.dedup();
		out
	}

	pub const BLANK: Self = Self {
		all_intervals: Vec::new(),
		accepts_for_rule: Vec::new(),
		ascii_transitions: Vec::new(),
		non_ascii_transitions: Vec::new(),
	};

	pub fn execute<'input>(
		&self,
		input: &'input [u8],
		last_was_delimited: u32,
	) -> Option<MatchedRule<'input>> {
		// The anchor transition's target may itself be accepting,
		// but (matching [`Tdfa::execute_without_captures`])
		// acceptance is only recorded while consuming input.
		let mut current_state: u16 = self.lookup_next_state(0, last_was_delimited)? & STATE_MASK;

		let mut maybe_backup: Option<BackupState> = None;

		let ascii_transitions: &[u16] = &self.ascii_transitions;
		let mut i: usize = 0;
		while i < input.len() {
			let b: u8 = input[i];
			// `consumed` is recorded *before* advancing,
			// so it holds the position of the character that led to the accepting state
			// (matching `Tdfa::execute_without_captures`).
			let (encoded, advance): (u16, usize) = if b < 0x80 {
				// Fast path: ASCII bytes map directly into the per-state table,
				// so we avoid UTF-8 decoding entirely.
				// The table also encodes whether the target is accepting,
				// saving a second lookup.
				// SAFETY: `current_state` is always a valid state index
				// (it comes from a masked transition value, or the anchor transition at the top),
				// and `b < 0x80`.
				let encoded: u16 = unsafe {
					*ascii_transitions
						.get_unchecked(usize::from(current_state) * 0x80 + usize::from(b))
				};
				(encoded, 1)
			} else {
				// Non-ASCII: decode a single UTF-8 scalar.
				// [`decode_scalar`] validates only the bytes of that scalar,
				// so the cost does not depend on the remaining input length.
				let (ch, len): (char, usize) = crate::utils::utf8::decode_scalar(&input[i..]);
				match self.lookup_next_state(current_state, u32::from(ch)) {
					Some(encoded) => (encoded, len),
					None => {
						break;
					},
				}
			};

			if encoded == 0 {
				break;
			}
			if encoded & ACCEPTING_BIT != 0 {
				let (rule_idx, maybe_encoding_idx) =
					self.accepts_for_rule[usize::from(encoded & STATE_MASK)].unwrap();
				maybe_backup = Some(BackupState {
					rule_idx,
					maybe_encoding_idx,
					consumed: i,
				});
			}
			current_state = encoded & STATE_MASK;
			i += advance;
		}

		// Treat end-of-input as if a newline followed.
		//
		// (Note: skipped if the loop broke early on a failed transition,
		// matching the `chain`-based iteration in [`Tdfa::execute_without_captures`].)
		if (i == input.len())
			&& let Some(encoded) = self.lookup_next_state(current_state, u32::from('\n'))
		{
			current_state = encoded & STATE_MASK;
			if encoded & ACCEPTING_BIT != 0 {
				let (rule_idx, maybe_encoding_idx) =
					self.accepts_for_rule[usize::from(current_state)].unwrap();
				maybe_backup = Some(BackupState {
					rule_idx,
					maybe_encoding_idx,
					consumed: input.len(),
				});
			}
		}

		let backup: BackupState = maybe_backup?;

		// SAFETY: the loop only advances past ASCII bytes (valid UTF-8 by definition)
		// and past non-ASCII scalars that were just validated
		// via [`crate::utils::utf8::decode_scalar`],
		// so `input[..consumed]` is valid UTF-8.
		let lexeme: &'input str =
			unsafe { std::str::from_utf8_unchecked(&input[..backup.consumed]) };

		Some(MatchedRule {
			rule_idx: backup.rule_idx,
			maybe_encoding_idx: backup.maybe_encoding_idx,
			lexeme,
		})
	}

	/// Leftmost-longest (grep semantics) search for a match starting at or after `pos`,
	/// simulated "like an NFA" over this DFA; see `docs/leftmost-longest-lexing.md`.
	///
	/// Threads (start position, DFA state) are spawned at every position from `pos`
	/// up to and including the position just after the first `'\n'` at or after `pos`;
	/// the lexer does not look for matches beginning past the current line.
	/// (Live threads are not cut off by the spawn window:
	/// a match starting inside it may span newlines.)
	/// The spawn state is the start state's transition on the lookbehind character,
	/// so `^`-anchored threads die immediately at any non-boundary position.
	///
	/// Threads in the same state at the same position are merged, keeping the earliest
	/// start, so the live set never exceeds the number of states.
	/// As soon as a match is found, all threads with a later start are killed;
	/// the surviving earliest-start threads run on,
	/// so the result is (earliest start, longest end, first rule in spec order).
	///
	/// `char_before` is the character just before `pos`, or `'\n'` when `pos == 0`.
	///
	/// Returns `None` when no rule matches anywhere in the window.
	pub fn execute_leftmost(
		&self,
		input: &[u8],
		pos: usize,
		char_before: u32,
		scratch: &mut LeftmostScratch,
	) -> Option<LeftmostMatch> {
		debug_assert!(pos <= input.len());
		if self.ascii_transitions.is_empty() {
			return None;
		}
		debug_assert_eq!(scratch.stamps.len(), self.ascii_transitions.len() / 0x80);

		let threads: &mut Vec<(usize, u16)> = &mut scratch.threads;
		threads.clear();
		let stamps: &mut [usize] = &mut scratch.stamps;

		let mut maybe_match: Option<LeftmostMatch> = None;

		// The position just after the first newline is the last viable start.
		// `pos` itself is always viable, even at the start of a line
		// (where the lookbehind is `'\n'`).
		let mut spawning_done: bool = false;

		// The character just before the current scanning position,
		// decoded once per position regardless of how many threads are live.
		let mut prev: u32 = char_before;

		const NEWLINE: u32 = 0xA; // u32::from('\n')

		let mut i: usize = pos;
		loop {
			// Gap scanning: nothing alive, so a spawn attempt and its first step can
			// share one iteration; instantly-dying spawns never touch the thread list.
			while threads.is_empty() {
				if maybe_match.is_some() || i == input.len() || spawning_done {
					// Nothing alive or spawnable can change the answer anymore.
					return maybe_match;
				}
				let b: u8 = input[i];
				let (ch, advance): (u32, usize) = if b < 0x80 {
					(u32::from(b), 1)
				} else {
					let (ch, len): (char, usize) = crate::utils::utf8::decode_scalar(&input[i..]);
					(u32::from(ch), len)
				};
				if let Some(encoded0) = self.lookup_next_state(0, prev) {
					// The fresh spawn's first step happens here, on the same character.
					if let Some(encoded) = self.lookup_next_state(encoded0 & STATE_MASK, ch) {
						self.record_accept(&mut maybe_match, i, encoded, i);
						threads.push((i, encoded & STATE_MASK));
						if i > pos && prev == NEWLINE {
							spawning_done = true;
						}
						i += advance;
						prev = ch;
						break;
					}
				}
				if i > pos && prev == NEWLINE {
					// The position just past the first newline is the last viable start.
					spawning_done = true;
				}
				i += advance;
				prev = ch;
			}

			// Stepping: advance every live thread on the same character.
			//
			// A fresh spawn is handled in the stepping paths below; `fresh` always has
			// the latest start, so merging keeps the earlier-start thread.
			while !threads.is_empty() {
				let mut fresh: Option<(usize, u16)> = None;
				if maybe_match.is_none() && !spawning_done && i < input.len() {
					if let Some(encoded) = self.lookup_next_state(0, prev) {
						// The spawn is dominated (an equal-or-earlier-start thread is
						// already in this state) iff a live thread holds the same state:
						// live threads at this position have start <= `i`,
						// and only one spawn happens per position.
						let spawned: u16 = encoded & STATE_MASK;
						if !threads.iter().any(|&(_, state)| state == spawned) {
							fresh = Some((i, spawned));
						}
					}
					if i > pos && prev == NEWLINE {
						spawning_done = true;
					}
				}

				if i == input.len() {
					// End-of-input lookahead, matching [`FlattenedDfa::execute`]:
					// a trailing `'\n'` lets end-anchored rules accept.
					for &(start, state) in threads.iter().chain(&fresh) {
						let Some(encoded) = self.lookup_next_state(state, NEWLINE) else {
							continue;
						};
						self.record_accept(&mut maybe_match, start, encoded, input.len());
					}
					return maybe_match;
				}

				let b: u8 = input[i];
				let (ch, advance): (u32, usize) = if b < 0x80 {
					(u32::from(b), 1)
				} else {
					let (ch, len): (char, usize) = crate::utils::utf8::decode_scalar(&input[i..]);
					(u32::from(ch), len)
				};

				match (threads.len(), fresh) {
					(1, None) => {
						// Fast path: with a single live thread there is nothing to merge,
						// so the position stamps are not touched.
						let (start, state): (usize, u16) = threads[0];
						if maybe_match.is_some_and(|m| start > m.start) {
							threads.clear();
						} else if let Some(encoded) = self.lookup_next_state(state, ch) {
							self.record_accept(&mut maybe_match, start, encoded, i);
							threads[0].1 = encoded & STATE_MASK;
						} else {
							threads.clear();
						}
					},
					(1, Some(_)) | (2, None) => {
						// Two total threads (counting `fresh`, always the latest start):
						// merge with a direct target comparison instead of the stamps.
						let (start0, state0): (usize, u16) = threads[0];
						let second: Option<(usize, u16)> = match fresh {
							Some(f) => Some(f),
							None => Some(threads[1]),
						};
						let kill0: bool = maybe_match.is_some_and(|m| start0 > m.start);
						let encoded0: Option<u16> = match kill0 {
							true => None,
							false => self.lookup_next_state(state0, ch),
						};
						let encoded1: Option<u16> = match second {
							Some((_, state1)) => self.lookup_next_state(state1, ch),
							None => None,
						};
						threads.clear();
						// A merged (equal-target) thread need not record its acceptance:
						// the kept thread is in the same accepting state with the same end
						// and a smaller-or-equal start, which always wins.
						if let Some(encoded0) = encoded0 {
							self.record_accept(&mut maybe_match, start0, encoded0, i);
							threads.push((start0, encoded0 & STATE_MASK));
						}
						if let (Some((start1, _)), Some(encoded1)) = (second, encoded1)
							&& threads
								.first()
								.is_none_or(|&(_, target)| (encoded1 & STATE_MASK) != target)
						{
							self.record_accept(&mut maybe_match, start1, encoded1, i);
							threads.push((start1, encoded1 & STATE_MASK));
						}
					},
					_ => {
						// Three or more live threads: stamp-marked merge.
						if let Some(f) = fresh {
							// `fresh` has the latest start, so appending keeps
							// `threads` sorted.
							threads.push(f);
						}
						let mark: usize = scratch.next_mark;
						scratch.next_mark = mark.wrapping_add(1);
						let mut w: usize = 0;
						for r in 0..threads.len() {
							let (start, state) = threads[r];
							if maybe_match.is_some_and(|m| start > m.start) {
								// Killed by the accepted match: earliest start wins.
								continue;
							}
							let Some(encoded) = self.lookup_next_state(state, ch) else {
								continue;
							};
							let next: usize = usize::from(encoded & STATE_MASK);
							let stamp: &mut usize = &mut stamps[next];
							if *stamp == mark {
								// Merged into an equivalent earlier-start thread.
								continue;
							}
							*stamp = mark;
							// See the note on merged threads above.
							self.record_accept(&mut maybe_match, start, encoded, i);
							threads[w] = (start, encoded & STATE_MASK);
							w += 1;
						}
						threads.truncate(w);
					},
				}

				i += advance;
				prev = ch;
			}
		}
	}

	/// Record an acceptance triggered by the lookahead character at byte position `end`,
	/// if the transition `encoded` lands on an accepting state.
	/// (The lexeme ends just before the lookahead, matching [`FlattenedDfa::execute`].)
	#[inline(always)]
	fn record_accept(
		&self,
		best: &mut Option<LeftmostMatch>,
		start: usize,
		encoded: u16,
		end: usize,
	) {
		if encoded & ACCEPTING_BIT == 0 {
			return;
		}
		let (rule_idx, maybe_encoding_idx) =
			self.accepts_for_rule[usize::from(encoded & STATE_MASK)].unwrap();
		Self::consider(
			best,
			LeftmostMatch {
				rule_idx,
				maybe_encoding_idx,
				start,
				end,
			},
		);
	}

	/// Fold `candidate` into `best` under leftmost-longest priority:
	/// earliest start, then longest match, then smallest [`RuleIdx`]
	/// (which is highest spec priority; see [`crate::parsing_spec::ParsingSpecBuilder::build`]).
	#[inline(always)]
	fn consider(best: &mut Option<LeftmostMatch>, candidate: LeftmostMatch) {
		let replace: bool = match best {
			None => true,
			Some(m) => {
				candidate.start < m.start
					|| (candidate.start == m.start
						&& (candidate.end > m.end
							|| (candidate.end == m.end
								&& u16::from(candidate.rule_idx) < u16::from(m.rule_idx))))
			},
		};
		if replace {
			*best = Some(candidate);
		}
	}

	#[inline(always)]
	fn lookup_next_state(&self, current_state: u16, ch: u32) -> Option<u16> {
		let current_state: usize = usize::from(current_state);
		let encoded: u16 = if ch < 0x80 {
			self.ascii_transitions[(current_state * 0x80) + (ch as usize)]
		} else {
			let char_class: usize = self.char_to_class(ch);
			if char_class == usize::MAX {
				return None;
			}
			self.non_ascii_transitions[(current_state * self.all_intervals.len()) + char_class]
		};
		// `0` means no transition. Valid transitions always have a nonzero target state.
		(encoded != 0).then_some(encoded)
	}

	fn char_to_class(&self, ch: u32) -> usize {
		// TODO refactor with interval tree
		let i: usize = self
			.all_intervals
			.partition_point(|interval| interval.end() < ch);
		if let Some(interval) = self.all_intervals.get(i) {
			if interval.start() <= ch {
				return i;
			}
		}
		usize::MAX
	}
}

impl LeftmostScratch {
	fn new(states: usize) -> Self {
		Self {
			threads: Vec::new(),
			stamps: vec![usize::MAX; states],
			next_mark: 0,
		}
	}
}

impl FlattenedDfa {
	/// A reusable scratch for [`FlattenedDfa::execute_leftmost`], sized to this DFA.
	pub fn leftmost_scratch(&self) -> LeftmostScratch {
		LeftmostScratch::new(self.ascii_transitions.len() / 0x80)
	}
}

/// The lead byte of the UTF-8 encoding of `ch` (ASCII included).
pub(crate) fn lead_byte_of(ch: u32, utf8_len: usize) -> u8 {
	match utf8_len {
		1 => u8::try_from(ch).unwrap(),
		2 => 0xC0 | u8::try_from(ch >> 6).unwrap(),
		3 => 0xE0 | u8::try_from(ch >> 12).unwrap(),
		_ => 0xF0 | u8::try_from(ch >> 18).unwrap(),
	}
}

/// Split the scalar range `[lo, hi]` into `(lo, hi, utf8_len)` sub-ranges
/// of uniform UTF-8 length, skipping the surrogate range 0xD800..=0xDFFF.
/// Lead-byte ranges stay contiguous across that gap
/// (0xD7FF and 0xE000 have adjacent lead bytes).
fn split_at_utf8_lengths(lo: u32, hi: u32) -> Vec<(u32, u32, usize)> {
	// `(surrogate start, end of class, length)` for the 3-byte classes.
	const THREE_BYTE: [(u32, u32, usize); 2] = [(0x800, 0xD7FF, 3), (0xE000, 0xFFFF, 3)];
	let mut out: Vec<(u32, u32, usize)> = Vec::new();
	let add = |out: &mut Vec<(u32, u32, usize)>, lo: u32, hi: u32, len: usize| {
		if lo <= hi && lo <= 0x10FFFF {
			out.push((lo, hi.min(0x10FFFF), len));
		}
	};
	if lo < 0x800 {
		add(&mut out, lo, hi.min(0x7FF), 2);
	}
	for &(start, end, len) in THREE_BYTE.iter() {
		if lo <= end && hi >= start {
			add(&mut out, lo.max(start), hi.min(end), len);
		}
	}
	if hi >= 0x10000 {
		add(&mut out, lo.max(0x10000), hi, 4);
	}
	out
}
