//! Based on Angelo Borsotti and Ulya Trafimovich. 2022. A closer look at TDFA.
//! - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
//! - <https://arxiv.org/abs/2206.01398>
//!
//! [tdfa]: https://arxiv.org/abs/2206.01398

mod flattened;
#[cfg(test)]
mod test;

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;
use std::num::NonZero;
use std::range::Range;
use std::sync::Arc;

pub use flattened::FlattenedDfa;
pub use flattened::LeftmostMatch;
pub use flattened::LeftmostScratch;
pub(crate) use flattened::lead_byte_of;
use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashMap;

use crate::graph::Csr;
use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::PolicyFunction;
use crate::nfa::CaptureTag;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::Tnfa;
use crate::nfa::Transitions;
use crate::parsing_spec::Encoding;
use crate::parsing_spec::EncodingIdx;
use crate::parsing_spec::RootRule;
use crate::parsing_spec::RuleIdx;
use crate::parsing_spec::RuleInfo;
use crate::regex::Regex;

#[derive(Debug, Clone)]
pub struct Tdfa {
	states: Vec<DfaState>,
	kernels: FxHashMap<Kernel, usize>,
	/// Kernels indexed by the sorted set of NFA states they contain.
	///
	/// A kernel can only match another up to register renaming if the two contain the same NFA
	/// states, so [`Tdfa::lookup_or_add_state`] only needs to run the (linear per kernel)
	/// register bijection check against the candidates in the matching bucket.
	kernels_by_states: FxHashMap<Vec<NfaIdx>, Vec<usize>>,
	pub tags: Vec<CaptureTag>,
	/// Bijection between indices of start/end capture pair tags.
	tag_pairs: Vec<usize>,
	/// During construction, this is the "current" count;
	/// after construction, this is the "total required".
	/// The first `tags.len()` are initial registers for the corresponding tags.
	/// The second `tags.len()` (i.e. `tags.len()..(2 * tags.len())`)
	/// are the corresponding final registers.
	pub number_of_registers: usize,
}

/// Captures are unbounded; e.g. with the regex `(?<digit>[0-9]+)*`,
/// so this struct caches memory for [`Tdfa::execute_with_captures`].
#[derive(Debug, Clone)]
pub struct TdfaExecution {
	pub captures: Vec<MatchedCapture>,
	registers: Vec<Option<NonZero<usize>>>,
	prefix_tree: PrefixTree,
	num_tags: usize,
}

#[derive(Debug)]
pub struct MatchedRule<'input> {
	pub rule_idx: RuleIdx,
	pub maybe_encoding_idx: Option<EncodingIdx>,
	pub lexeme: &'input str,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MatchedCapture {
	pub rule_idx: RuleIdx,
	pub capture_id: NonZero<u16>,
	pub maybe_encoding_idx: Option<EncodingIdx>,
	pub parent_id: Option<NonZero<u16>>,
	pub parent_index: usize,
	pub is_leaf: bool,
	/// Relative to root [`MatchedRule`]'s lexeme.
	pub range: Range<usize>,
}

#[derive(Debug, Clone)]
struct DfaState {
	kernel: Kernel,
	transitions: IntervalTree<u32, Transition>,
	/// If this is a final state (the kernel contains an accepting NFA state),
	/// the rule that this state has matched for.
	accepting_rule: Option<(RuleIdx, Option<EncodingIdx>)>,
	/// Register operations upon finalizing a match (if applicable);
	/// copy to the final registers.
	final_operations: Vec<RegisterOperation>,
	/// Cache/combined map from this state's configurations of "register -> which tag it holds".
	/// Present for debugging.
	tag_for_register: BTreeMap<usize, CaptureTag>,
	/// We cache the outgoing transitions for the first so many "common" characters;
	/// ASCII is most common and happens to be the first 0x80 unicode code points.
	/// However, the cache size can be changed here without touching the rest of the code.
	/// Of course, in practice, the cache is presumed to be much much smaller
	/// than the full range of unicode code points;
	/// but technically the code should work for any value here
	/// (comments in the relevant parts of the implementation explain why).
	ascii_cache: [Transition; 0x80],
}

/// In untagged DFA, the kernel of a DFA state is simply the set of corresponding NFA states;
/// all other information, including state transitions, are derived from the kernel.
/// In tagged TDFA, the kernel of a TDFA state is a set of corresponding [`Configuration`]s.
///
/// Conceptually, a kernel may more accurately be represented using
/// `BTreeMap<NfaIdx, Configuration>`;
/// an NFA state should not show up more than once in a kernel, since:
///
/// 1. by construction, distinct NFA states have distinct "next states" on non-epsilon transitions,
///    so [`Kernel::step_on_intervals`] always "lands on" a list of distinct NFA states, and
/// 2. the [`Tdfa::epsilon_closure`] procedure (using a depth-first search)
///    only saves the first path of NFA states reachable through epsilon transitions.
///
/// However, both of the aforementioned procedures operate more naturally on a list
/// of `Configuration`s, and `Vec<Configuration>` naturally has better memory locality.
#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Kernel(Vec<Configuration>);

/// A "configuration" is essentially an augmented NFA state (as documented per field).
#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Configuration {
	nfa_state: NfaIdx,
	/// A mapping "tag (by ID/index) -> register";
	/// answers "which register holds this tag?".
	register_for_tag: Vec<usize>,
	/// Sequence of tags accumulated to reach this state during [`Tdfa::epsilon_closure`]
	/// (corresponding to the execution of positive/negative tags during NFA simulation).
	tag_path_in_closure: Vec<(CaptureTag, SymbolicPosition)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Transition {
	/// `usize::MAX` is used as an "invalid/empty" marker value.
	/// See [`NfaIdx`] for a note on why this is "safe".
	target: usize,
	operations: Vec<RegisterOperation>,
}

/// Compared to the paper, we factor out the left hand side of a register operation.
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct RegisterOperation {
	destination: usize,
	action: RegisterAction,
}

/// "Right hand side" of a register operation:
///
/// - "**set**" register to `Current` or `Nil`.
///   Used for single value tags;
///   not relevant since we treat every tag/capture as (possibly) multi-valued.
/// - "**copy**" from register `source`.
/// - copy from register `source` and "**append**" `history`.
///   Used for multi-value tags.
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum RegisterAction {
	CopyFrom {
		source: usize,
	},
	Append {
		source: usize,
		history: Vec<SymbolicPosition>,
	},
}

#[derive(Debug, Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum SymbolicPosition {
	Current,
	Nil,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct PrefixTree {
	nodes: Vec<PrefixTreeNode>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct PrefixTreeNode {
	maybe_predecessor: Option<NonZero<usize>>,
	lexeme_position: usize,
}

#[derive(Debug, Clone, Copy)]
struct BackupState {
	rule_idx: RuleIdx,
	maybe_encoding_idx: Option<EncodingIdx>,
	consumed: usize,
}

impl Tdfa {
	pub const BLANK: Self = Self {
		states: Vec::new(),
		kernels: FxHashMap::with_hasher(FxBuildHasher),
		kernels_by_states: FxHashMap::with_hasher(FxBuildHasher),
		tags: Vec::new(),
		tag_pairs: Vec::new(),
		number_of_registers: 0,
	};

	/// Similar to [`Tdfa::execute_without_captures`],
	/// but without anchor transitions/lookaround characters.
	/// Only relevant for testing.
	#[cfg(test)]
	pub fn execute(&self, input: &str) -> bool {
		let mut current_state: usize = 0;
		for (_pos, ch) in input.char_indices() {
			if let Some(transition) = self.lookup_transition(current_state, u32::from(ch)) {
				current_state = transition.target;
			} else {
				return false;
			}
		}
		true
	}

	/// Used for determining which rule matched.
	/// Assumes the DFA was constructed **with** anchor transitions.
	///
	/// [`TdfaExecution::captures`] is sorted:
	/// 1. left to right w.r.t. the input,
	/// 2. top-down w.r.t the regex
	///    (i.e. left to right w.r.t. the regex pattern; parent before children).
	///
	/// `last_was_delimited` should be the "previous" character in the input,
	/// or newline (`'\n'`) if at the very start of input.
	///
	/// Newline is "always" an anchor character,
	/// since newlines are already (and necessarily) used to terminate/separate log events;
	/// an anchored rule should match the same in a log event,
	/// regardless of the log event's position in the entire log.
	pub fn execute_without_captures<'input>(
		&self,
		input: &'input str,
		last_was_delimited: u32,
	) -> Option<MatchedRule<'input>> {
		let anchor_transition: &Transition = self.lookup_transition(0, last_was_delimited)?;
		let mut current_state: usize = anchor_transition.target;

		let mut maybe_backup: Option<BackupState> = None;

		for (pos, ch) in input
			.char_indices()
			.chain(std::iter::once((input.len(), '\n')))
		{
			if let Some(transition) = self.lookup_transition(current_state, u32::from(ch)) {
				current_state = transition.target;
				if let Some((rule_idx, maybe_encoding_idx)) =
					self.states[current_state].accepting_rule
				{
					maybe_backup = Some(BackupState {
						rule_idx,
						maybe_encoding_idx,
						consumed: pos,
					});
				}
			} else {
				break;
			}
		}

		let backup: BackupState = maybe_backup?;

		Some(MatchedRule {
			rule_idx: backup.rule_idx,
			maybe_encoding_idx: backup.maybe_encoding_idx,
			lexeme: &input[..backup.consumed],
		})
	}

	/// Assumes the DFA was constructed **without** anchor transitions.
	/// Captures are stored in `execution_data`, sorted:
	///
	/// 1. left-to-right (lexicographically with respect to the input),
	/// 2. top-down; parent captures first
	///    (lexicographically with respect to the regex pattern).
	pub fn execute_with_captures(
		&self,
		input: &str,
		execution_data: &mut TdfaExecution,
		rule: &RootRule,
	) -> bool {
		let rule_idx: RuleIdx = rule.idx;
		let mut current_state: usize = 0;

		// Err on the side of safety and clear the data here;
		// we could require the caller do this,
		// but it's not statically verifiable so we'd just end up panicking at runtime anyways.
		execution_data.clear();

		// Vector of prefix tree node indices.
		let registers: &mut [Option<NonZero<usize>>] = &mut execution_data.registers;
		let prefix_tree: &mut PrefixTree = &mut execution_data.prefix_tree;
		let captures: &mut Vec<MatchedCapture> = &mut execution_data.captures;
		assert_eq!(captures, &[]);

		for (pos, ch) in input.char_indices() {
			if let Some(transition) = self.lookup_transition(current_state, u32::from(ch)) {
				// Apply transition operations before updating position; lookahead 1 in TDFA(1).
				self.apply_operations(
					registers,
					prefix_tree,
					pos,
					&transition.operations,
					&self.states[current_state].tag_for_register,
				);
				current_state = transition.target;
			} else {
				return false;
			}
		}

		if self.states[current_state].accepting_rule.is_none() {
			return false;
		}

		self.apply_operations(
			registers,
			prefix_tree,
			input.len(),
			&self.states[current_state].final_operations,
			&self.states[current_state].tag_for_register,
		);

		for (open, close) in self
			.tag_pairs
			.iter()
			.copied()
			.enumerate()
			// Skip close tags.
			.step_by(2)
		{
			let tag: &CaptureTag = &self.tags[open];

			assert_ne!(tag.capture_id, 0);

			assert!(self.tags[close].is_close);
			assert_eq!(self.tags[close].capture_id, tag.capture_id);

			let capture: &RuleInfo = &rule[tag.capture_id];

			let mut maybe_open: Option<NonZero<usize>> = registers[self.tags.len() + open];
			let mut maybe_close: Option<NonZero<usize>> = registers[self.tags.len() + close];

			while let Some(open_node) = maybe_open {
				let close_node: NonZero<usize> = maybe_close.unwrap();

				let start: usize = prefix_tree[open_node].lexeme_position;
				let end: usize = prefix_tree[close_node].lexeme_position;
				captures.push(MatchedCapture {
					rule_idx,
					// `NonZero`; see the corresponding note in `MatchedCapture`.
					capture_id: NonZero::new(tag.capture_id).unwrap(),
					maybe_encoding_idx: tag.maybe_encoding_idx,
					parent_id: NonZero::new(capture.parent_id),
					parent_index: usize::MAX,
					is_leaf: capture.is_leaf(),
					range: Range { start, end },
				});
				maybe_open = prefix_tree[open_node].maybe_predecessor;
				maybe_close = prefix_tree[close_node].maybe_predecessor;
			}
		}
		captures
			.sort_unstable_by_key(|cap| (cap.range.start, Reverse(cap.range.end), cap.capture_id));
		for i in 0..captures.len() {
			if let Some(parent_id) = captures[i].parent_id {
				// Linear scan (backwards) since it should usually be small.
				for j in (0..i).rev() {
					if (captures[j].capture_id == parent_id)
						&& (captures[i].range.end <= captures[j].range.end)
					{
						assert!(captures[j].range.start <= captures[i].range.start);
						captures[i].parent_index = 1 + j;
						break;
					}
				}
				assert_ne!(captures[i].parent_index, usize::MAX);
			} else {
				captures[i].parent_index = 0;
			}
		}
		true
	}

	fn lookup_transition(&self, current_state: usize, ch: u32) -> Option<&Transition> {
		let cache_index: usize = {
			// Cast safety - see [`crate::_USIZE_AT_LEAST_32_BITS`].
			ch as usize
		};
		let current_state: &DfaState = &self.states[current_state];
		if let Some(transition) = current_state.ascii_cache.get(cache_index) {
			if transition.is_valid() {
				Some(transition)
			} else {
				None
			}
		} else {
			current_state.transitions.lookup(ch)
		}
	}

	fn apply_operations(
		&self,
		registers: &mut [Option<NonZero<usize>>],
		prefix_tree: &mut PrefixTree,
		pos: usize,
		ops: &[RegisterOperation],
		_tag_for_register: &BTreeMap<usize, CaptureTag>,
	) {
		for o in ops.iter() {
			match &o.action {
				RegisterAction::CopyFrom { source } => {
					registers[o.destination] = registers[*source];
				},
				RegisterAction::Append { source, history } => {
					registers[o.destination] = registers[*source];
					for &symbolic in history.iter() {
						if symbolic == SymbolicPosition::Current {
							let node: NonZero<usize> =
								prefix_tree.add_node(registers[o.destination], pos);
							registers[o.destination] = Some(node);
						}
					}
				},
			}
		}
	}
}

impl Tdfa {
	pub fn execution_data(&self) -> TdfaExecution {
		TdfaExecution {
			captures: Vec::new(),
			registers: vec![None; self.number_of_registers],
			prefix_tree: PrefixTree::new(),
			num_tags: self.tags.len(),
		}
	}
}

impl Tdfa {
	/// Construct the TDFA for the combination (alternation) of multiple rules,
	/// i.e. from a parsing specification.
	///
	/// Capturing is not enabled.
	#[tracing::instrument(skip_all, level = "trace")]
	pub fn for_rules(rules: &[RootRule], delimiters: &str, encodings: &[Arc<Encoding>]) -> Self {
		let nfa: Tnfa = Tnfa::for_rules(rules, delimiters, encodings);
		Self::determinization::<false>(&nfa)
	}

	/// Construct the TDFA for a single rule, with captures.
	pub fn for_single_rule(rule_idx: RuleIdx, regex: &Regex, encodings: &[Arc<Encoding>]) -> Self {
		let nfa: Tnfa = Tnfa::for_single_rule(rule_idx, regex, encodings);
		Self::determinization::<true>(&nfa)
	}

	/// Initialize the cache for ASCII transitions, must be called after deserializing.
	pub fn initialize_ascii_cache(&mut self) {
		for state in self.states.iter_mut() {
			state.initialize_ascii_cache();
		}
	}

	/// Algorithm 3 in the [paper][tdfa].
	pub fn determinization<const WITH_TAGS: bool>(nfa: &Tnfa) -> Self {
		let tags: Vec<CaptureTag> = if WITH_TAGS {
			nfa.tags().iter().cloned().collect::<Vec<_>>()
		} else {
			Vec::new()
		};
		assert_eq!(tags.len() % 2, 0);
		let mut tag_pairs: Vec<usize> = Vec::with_capacity(tags.len());
		// XXX: use array_chunks when stable.
		for (i, chunk) in tags.chunks_exact(2).enumerate() {
			assert_eq!(chunk[0].rule_idx, chunk[1].rule_idx);
			assert_eq!(chunk[0].capture_id, chunk[1].capture_id);
			assert_eq!(chunk[0].maybe_encoding_idx, chunk[1].maybe_encoding_idx);
			tag_pairs.push((i * 2) + 1);
			tag_pairs.push(i * 2);
		}

		let mut dfa: Self = Self {
			states: Vec::new(),
			kernels: FxHashMap::with_hasher(FxBuildHasher),
			kernels_by_states: FxHashMap::with_hasher(FxBuildHasher),
			tags,
			tag_pairs,
			number_of_registers: 2 * nfa.tags().len(),
		};

		let initial: (Configuration, Vec<(CaptureTag, SymbolicPosition)>) = (
			Configuration {
				nfa_state: NfaIdx::BEGIN,
				register_for_tag: (0..dfa.tags.len()).collect::<Vec<_>>(),
				tag_path_in_closure: Vec::new(),
			},
			Vec::new(),
		);

		let initial: Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)> =
			Self::epsilon_closure::<WITH_TAGS>(nfa, &vec![initial]);

		dfa.lookup_or_add_state(nfa, initial, &mut Vec::new());

		// Note: New states may be created and appended to `dfa.states` inside the loop;
		// `dfa.states.len()` is not constant.
		let mut i: usize = 0;
		while i < dfa.states.len() {
			// Since we may append to `dfa.states`, it may resize,
			// and a reference to `dfa.states[i]` here would become invalid
			// (borrow checker will complain without the `.clone()`.
			let kernel: Kernel = dfa.states[i].kernel.clone();

			let mut register_action_tag: BTreeMap<(CaptureTag, RegisterAction), usize> =
				BTreeMap::new();
			for (interval, next) in kernel.step_on_intervals(nfa).iter() {
				let next: Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)> =
					Self::epsilon_closure::<WITH_TAGS>(nfa, next);

				let (next, mut operations): (
					Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)>,
					Vec<RegisterOperation>,
				) = dfa.transition_operations(next, &mut register_action_tag);

				let next: usize = dfa.lookup_or_add_state(nfa, next, &mut operations);
				dfa.states[i].transitions.insert(
					interval,
					Transition {
						target: next,
						operations,
					},
					PolicyFunction::new(|existing, new| {
						panic!(
							"state {i} transition on {interval:?} had existing target {:?}, trying to insert {:?}",
							existing, new
						);
					}),
				);
			}

			i += 1;
		}

		dfa.initialize_ascii_cache();

		dfa
	}

	/// Algorithm 3 in the [paper][tdfa].
	///
	/// Returns the index of the DFA state for the given kernel
	/// (the set of NFA configurations landed on after a transition),
	/// reusing an existing state when possible.
	///
	/// Kernels may be equal up to register (re)naming.
	fn lookup_or_add_state(
		&mut self,
		nfa: &Tnfa,
		configurations: Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)>,
		ops: &mut Vec<RegisterOperation>,
	) -> usize {
		let kernel: Kernel = Kernel(Vec::from_iter(
			configurations.into_iter().map(|(config, _)| config),
		));

		if let Some(&idx) = self.kernels.get(&kernel) {
			return idx;
		}

		let mut kernel_states: Vec<NfaIdx> = Vec::with_capacity(kernel.0.len());
		kernel_states.extend(kernel.0.iter().map(|config| config.nfa_state));
		// Sorting makes the index key independent of the order the closure happened to produce.
		kernel_states.sort_unstable();

		if let Some(candidates) = self.kernels_by_states.get(&kernel_states) {
			for &idx in candidates {
				let other: &Kernel = &self.states[idx].kernel;
				let Some(bijection) = kernel.try_register_bijection(other) else {
					continue;
				};
				if let Some(new_ops) =
					Self::map_operations_through_bijection(ops.clone(), bijection)
				{
					*ops = new_ops;
					return idx;
				}
			}
		}

		// No existing state matches; create a new one.
		// The first accepting configuration has priority.
		let mut accepting_rule: Option<(RuleIdx, Option<EncodingIdx>)> = None;
		let mut final_operations: Vec<RegisterOperation> = Vec::new();
		let mut tag_for_register: BTreeMap<usize, CaptureTag> = BTreeMap::new();
		for config in kernel.0.iter() {
			let nfa_state: &NfaState = &nfa[config.nfa_state];
			if accepting_rule.is_none()
				&& let Some(accepting_data) = nfa_state.maybe_accepting_data
			{
				accepting_rule = Some(accepting_data);
				final_operations =
					self.final_operations(&config.register_for_tag, &config.tag_path_in_closure);
			}
			for (tag_idx, &register) in config.register_for_tag.iter().enumerate() {
				let old: Option<CaptureTag> =
					tag_for_register.insert(register, self.tags[tag_idx].clone());
				assert!(old.is_none() || (old.as_ref() == Some(&self.tags[tag_idx])));
			}
		}

		let idx: usize = self.states.len();
		self.states.push(DfaState {
			kernel: kernel.clone(),
			transitions: IntervalTree::new(),
			accepting_rule,
			final_operations,
			tag_for_register,
			ascii_cache: DfaState::default_ascii_cache(),
		});
		self.kernels.insert(kernel, idx);
		self.kernels_by_states
			.entry(kernel_states)
			.or_default()
			.push(idx);
		idx
	}

	/// Algorithm 3 in the [paper][tdfa].
	///
	/// Rewrites `ops` (whose destinations are registers of the new kernel)
	/// to target the corresponding registers of the existing kernel, per `bijection`,
	/// and appends the copy operations needed to reconcile the remaining registers.
	///
	/// Returns `None` if reconciling would require breaking a nontrivial cycle of copies.
	fn map_operations_through_bijection(
		mut ops: Vec<RegisterOperation>,
		mut bijection: BTreeMap<usize, usize>,
	) -> Option<Vec<RegisterOperation>> {
		// Destinations already written by `ops` need no copy;
		// remove them from the bijection.
		for o in ops.iter_mut() {
			o.destination = bijection.remove(&o.destination).unwrap();
		}
		ops.extend(
			bijection
				.iter()
				.map(|(&source, &destination)| RegisterOperation {
					destination,
					action: RegisterAction::CopyFrom { source },
				}),
		);
		Self::topological_sort(ops)
	}

	/// Algorithm 3 in the [paper][tdfa].
	fn topological_sort(mut ops: Vec<RegisterOperation>) -> Option<Vec<RegisterOperation>> {
		let mut in_degree_register: BTreeMap<usize, usize> = BTreeMap::new();
		for o in ops.iter() {
			match &o.action {
				RegisterAction::CopyFrom { source } | RegisterAction::Append { source, .. } => {
					in_degree_register.insert(*source, 0);
					in_degree_register.insert(o.destination, 0);
				},
			}
		}
		for o in ops.iter() {
			match &o.action {
				RegisterAction::CopyFrom { source } | RegisterAction::Append { source, .. } => {
					*in_degree_register.get_mut(source).unwrap() += 1;
				},
			}
		}

		let mut new_ops: Vec<RegisterOperation> = Vec::new();
		let mut nontrivial_cycle: bool = false;

		let mut tmp_ops: Vec<RegisterOperation> = Vec::new();
		while !ops.is_empty() {
			let mut anything_added: bool = false;
			for o in ops.drain(..) {
				if in_degree_register[&o.destination] == 0 {
					match &o.action {
						RegisterAction::CopyFrom { source }
						| RegisterAction::Append { source, .. } => {
							*in_degree_register.get_mut(source).unwrap() += 1;
						},
					}
					new_ops.push(o);
					anything_added = true;
				} else {
					tmp_ops.push(o);
				}
			}
			std::mem::swap(&mut ops, &mut tmp_ops);
			if !anything_added {
				for o in ops.iter() {
					match &o.action {
						RegisterAction::CopyFrom { source }
						| RegisterAction::Append { source, .. } => {
							if *source != o.destination {
								nontrivial_cycle = true;
							}
						},
					}
				}
				new_ops.extend(ops.drain(..));
			}
		}

		(!nontrivial_cycle).then_some(new_ops)
	}

	/// Algorithm 3 in the [paper][tdfa].
	fn epsilon_closure<const WITH_TAGS: bool>(
		nfa: &Tnfa,
		configurations: &Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)>,
	) -> Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)> {
		let mut closure: Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)> = Vec::new();

		let mut nfa_states_on_stack: BTreeSet<NfaIdx> = configurations
			.iter()
			.map(|(config, _)| config.nfa_state)
			.collect::<BTreeSet<_>>();

		let mut stack: Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)> =
			configurations.clone();
		stack.reverse();

		while let Some((config, inherited)) = stack.pop() {
			closure.push((config.clone(), inherited.clone()));

			// Remark: Accepting states have no outgoing transitions.
			match &nfa[config.nfa_state].transitions {
				Transitions::Interval(_) => {
					continue;
				},
				Transitions::Spontaneous(transitions) => {
					for &target in transitions.iter().rev() {
						if nfa_states_on_stack.contains(&target) {
							continue;
						}

						let new_config: Configuration = Configuration {
							nfa_state: target,
							..config.clone()
						};

						nfa_states_on_stack.insert(new_config.nfa_state);
						stack.push((new_config, inherited.clone()));
					}
				},
				Transitions::Tagged {
					tag,
					positive,
					target,
				} => {
					if nfa_states_on_stack.contains(target) {
						continue;
					}

					let mut new_config: Configuration = Configuration {
						nfa_state: *target,
						..config.clone()
					};

					if WITH_TAGS {
						new_config.tag_path_in_closure.push((
							tag.clone(),
							if *positive {
								SymbolicPosition::Current
							} else {
								SymbolicPosition::Nil
							},
						));
					} else {
						// If `!WITH_TAGS`, this path is identical to the branch
						// for `Transitions::Spontaneous`.
					}

					nfa_states_on_stack.insert(new_config.nfa_state);
					stack.push((new_config, inherited.clone()));
				},
			}
		}

		Kernel::invariants(&closure);
		closure
	}

	/// Algorithm 3 in the [paper][tdfa].
	fn transition_operations(
		&mut self,
		configurations: Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)>,
		register_action_tag: &mut BTreeMap<(CaptureTag, RegisterAction), usize>,
	) -> (
		Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)>,
		Vec<RegisterOperation>,
	) {
		let mut new_configurations: Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)> =
			Vec::new();
		let mut ops: BTreeSet<RegisterOperation> = BTreeSet::new();

		for (mut config, inherited) in configurations.into_iter() {
			for (tag_idx, tag) in self.tags.iter().enumerate() {
				let history: Vec<SymbolicPosition> = Self::filter_history_for_tag(&inherited, tag);
				if history.is_empty() {
					continue;
				}
				let action: RegisterAction =
					Self::operation_rhs(&config.register_for_tag, history, tag_idx);
				let target: usize = *register_action_tag
					.entry((tag.clone(), action.clone()))
					.or_insert_with_key(|(_, _)| {
						let r: usize = self.number_of_registers;
						self.number_of_registers += 1;
						r
					});
				ops.insert(RegisterOperation {
					destination: target,
					action,
				});
				config.register_for_tag[tag_idx] = target;
			}
			new_configurations.push((config, inherited));
		}

		(new_configurations, ops.into_iter().collect::<Vec<_>>())
	}

	/// Algorithm 3 in the [paper][tdfa].
	fn final_operations(
		&self,
		registers: &[usize],
		history: &[(CaptureTag, SymbolicPosition)],
	) -> Vec<RegisterOperation> {
		let mut ops: Vec<RegisterOperation> = Vec::new();

		for (tag_idx, tag) in self.tags.iter().enumerate() {
			let history: Vec<SymbolicPosition> = Self::filter_history_for_tag(history, tag);
			let action: RegisterAction = if history.is_empty() {
				RegisterAction::CopyFrom {
					source: registers[tag_idx],
				}
			} else {
				Self::operation_rhs(registers, history, tag_idx)
			};
			ops.push(RegisterOperation {
				destination: self.tags.len() + tag_idx,
				action,
			});
		}

		ops
	}

	/// Algorithm 3 in the [paper][tdfa].
	fn operation_rhs(
		registers: &[usize],
		history: Vec<SymbolicPosition>,
		tag_idx: usize,
	) -> RegisterAction {
		RegisterAction::Append {
			source: registers[tag_idx],
			history,
		}
	}

	/// Algorithm 3 in the [paper][tdfa].
	fn filter_history_for_tag(
		history: &[(CaptureTag, SymbolicPosition)],
		tag1: &CaptureTag,
	) -> Vec<SymbolicPosition> {
		history
			.iter()
			.filter_map(|(tag2, pos)| if tag2 == tag1 { Some(*pos) } else { None })
			.collect::<Vec<_>>()
	}
}

impl Tdfa {
	pub fn all_intervals(&self) -> Vec<Interval<u32>> {
		use crate::interval_tree::PolicyNoop;

		let mut all_intervals: IntervalTree<u32, ()> = IntervalTree::new();
		for state in self.states.iter() {
			for (interval, _) in state.transitions.iter() {
				all_intervals.insert(interval, (), PolicyNoop);
			}
		}
		Vec::from_iter(all_intervals.iter().map(|(interval, &())| interval))
	}
}

impl Tdfa {
	/// Compute the canonical (minimal) DFA.
	///
	/// This should not be used with a TDFA (DFA with tagged transitions).
	///
	/// The result has no kernels (and no kernel lookup table),
	/// so it cannot be extended by further determinization.
	#[tracing::instrument(skip_all, level = "trace")]
	pub fn canonicalize(&self) -> Tdfa {
		let partitions: Vec<Vec<usize>> = self.partition_states();

		let mut partition_for_state: Vec<usize> = vec![usize::MAX; self.states.len()];
		for (i, x) in partitions.iter().enumerate() {
			for &s in x.iter() {
				partition_for_state[s] = i;
			}
		}

		let mut new_states: Vec<DfaState> = Vec::with_capacity(partitions.len());

		for x in partitions.iter() {
			let mut maybe_transitions: Option<IntervalTree<u32, Transition>> = None;
			for &s in x.iter() {
				let state: &DfaState = &self.states[s];
				if let Some(transitions) = &maybe_transitions {
					for (interval, transition) in transitions.iter() {
						assert_eq!(
							transition.target,
							partition_for_state
								[state.transitions.lookup(interval.start()).unwrap().target]
						);
					}
					for (interval, transition) in state.transitions.iter() {
						assert_eq!(
							partition_for_state[transition.target],
							transitions.lookup(interval.start()).unwrap().target
						);
					}
				} else {
					let mut transitions: IntervalTree<u32, Transition> = state.transitions.clone();
					for (_, transition) in transitions.iter_mut() {
						transition.target = partition_for_state[transition.target];
					}
					// TODO: is this necessary?
					transitions.coalesce();
					maybe_transitions = Some(transitions);
				}
			}
			let representative: &DfaState = &self.states[*x.first().unwrap()];
			new_states.push(DfaState {
				// Kernels only serve determinization (state lookup);
				// merging and re-indexing them here was the peak memory of an uncached
				// spec build (several copies of every configuration), for no later use.
				kernel: Kernel(Vec::new()),
				transitions: maybe_transitions.unwrap(),
				accepting_rule: representative.accepting_rule,
				final_operations: Vec::new(),
				tag_for_register: BTreeMap::new(),
				ascii_cache: DfaState::default_ascii_cache(),
			});
		}

		let mut canonical: Self = Self {
			states: new_states,
			kernels: FxHashMap::with_hasher(FxBuildHasher),
			kernels_by_states: FxHashMap::with_hasher(FxBuildHasher),
			tags: Vec::new(),
			tag_pairs: Vec::new(),
			number_of_registers: 0,
		};
		canonical.initialize_ascii_cache();
		canonical
	}

	/// Hopcroft's DFA minimization algorithm.
	#[tracing::instrument(skip_all, level = "trace")]
	fn partition_states(&self) -> Vec<Vec<usize>> {
		if self.states.is_empty() {
			return Vec::new();
		}

		let all_intervals: Vec<Interval<u32>> = self.all_intervals();

		// CSR for reversed edges (predecessors).
		let csr: Csr<(u32, u32)> = Csr::build(
			self.states.len(),
			self.states.iter().enumerate().flat_map(|(source, state)| {
				// We need `move` for `source`, but not `all_classes`.
				let all_classes: &[Interval<u32>] = &all_intervals;
				state.transitions.iter().map(move |(interval, transition)| {
					let first: usize =
						all_classes.partition_point(|class| class.end() < interval.start());
					let last: usize =
						all_classes.partition_point(|class| class.end() < interval.end());
					assert_eq!(all_classes[first].start(), interval.start());
					assert_eq!(all_classes[last].end(), interval.end());
					// There are at most `u32::MAX` intervals,
					// so at most `u32::MAX` classes.
					(transition.target, source, (first as u32, last as u32))
				})
			}),
		);

		let mut by_accepting: BTreeMap<Option<(RuleIdx, Option<EncodingIdx>)>, Vec<usize>> =
			BTreeMap::new();
		for (i, state) in self.states.iter().enumerate() {
			by_accepting
				.entry(state.accepting_rule)
				.or_insert_with(Vec::new)
				.push(i);
		}

		// `members[starts[p]..ends[p]]` are the states in partition `p`.
		let mut members: Vec<usize> = Vec::with_capacity(self.states.len());
		let mut starts: Vec<usize> = Vec::new();
		let mut ends: Vec<usize> = Vec::new();
		// For state `i`, `position[i]` is an index into `membership`;
		// `membership[position[i]] == i`.
		let mut position: Vec<usize> = vec![0; self.states.len()];
		let mut partition_for_state: Vec<usize> = vec![0; self.states.len()];

		for initial_partition in by_accepting.into_values() {
			let p: usize = starts.len();
			starts.push(members.len());
			for state in initial_partition.into_iter() {
				position[state] = members.len();
				members.push(state);
				partition_for_state[state] = p;
			}
			ends.push(members.len());
		}

		let mut stack: Vec<usize> = Vec::from_iter(0..starts.len());
		let mut is_queued: Vec<bool> = vec![true; starts.len()];
		let mut marked: Vec<usize> = vec![0; starts.len()];

		// Snapshot of states of currently-processing block.
		let mut splitter: Vec<usize> = Vec::new();
		let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); all_intervals.len()];
		let mut dirty_classes: Vec<usize> = Vec::new();
		let mut dirty_partitions: Vec<usize> = Vec::new();

		while let Some(splitter_block) = stack.pop() {
			is_queued[splitter_block] = false;

			splitter.clear();
			splitter.extend_from_slice(&members[starts[splitter_block]..ends[splitter_block]]);

			for &state in splitter.iter() {
				for edge in csr[state].iter() {
					// Were originally `usize`s.
					let first: usize = edge.value.0 as usize;
					let last: usize = edge.value.1 as usize;
					for (offset, bucket) in predecessors[first..=last].iter_mut().enumerate() {
						if bucket.is_empty() {
							dirty_classes.push(first + offset);
						}
						bucket.push(edge.target);
					}
				}
			}

			for class in dirty_classes.drain(..) {
				for state in predecessors[class].drain(..) {
					let p: usize = partition_for_state[state];
					if marked[p] == 0 {
						dirty_partitions.push(p);
					}
					let from: usize = position[state];
					let to: usize = starts[p] + marked[p];
					members.swap(from, to);
					position[members[from]] = from;
					position[members[to]] = to;
					marked[p] += 1;
				}

				for p in dirty_partitions.drain(..) {
					let count: usize = std::mem::replace(&mut marked[p], 0);
					if count == (ends[p] - starts[p]) {
						continue;
					}
					let new_partition: usize = starts.len();
					starts.push(starts[p]);
					ends.push(starts[p] + count);
					marked.push(0);
					for i in starts[p]..(starts[p] + count) {
						partition_for_state[members[i]] = new_partition;
					}
					starts[p] += count;

					let requeue: usize = if is_queued[p] || (count <= (ends[p] - starts[p])) {
						new_partition
					} else {
						p
					};
					is_queued.push(false);
					if !is_queued[requeue] {
						stack.push(requeue);
						is_queued[requeue] = true;
					}
				}
			}
		}

		let mut partitions: Vec<Vec<usize>> = Vec::from_iter(
			(0..starts.len()).map(|p| Vec::from_iter(members[starts[p]..ends[p]].iter().copied())),
		);

		// Entry should be first.
		partitions.swap(0, partition_for_state[0]);

		partitions
	}
}

impl DfaState {
	fn initialize_ascii_cache(&mut self) {
		for (i, cached_transition) in self.ascii_cache.iter_mut().enumerate() {
			// It doesn't matter whether this is a (lossless) upcast (`usize::BITS <= u32::BITS`)
			// or (lossy) downcast (`usize::BITS > u32::BITS`).
			// A lossless cast is necessarily harmless,
			// and a lossy downcast simply means the cache contains more slots than necessary,
			// which won't be touched during simulation/lexing.
			if let Some(transition) = self.transitions.lookup(i as u32) {
				*cached_transition = transition.clone();
			}
		}
	}

	fn default_ascii_cache() -> [Transition; 0x80] {
		std::array::from_fn(|_| Transition::invalid())
	}
}

impl TdfaExecution {
	pub fn new(registers: usize, tags: usize) -> Self {
		Self {
			captures: Vec::new(),
			registers: vec![None; registers],
			prefix_tree: PrefixTree::new(),
			num_tags: tags,
		}
	}

	/// Reset the state between TDFA executions.
	pub fn clear(&mut self) {
		self.captures.clear();
		self.prefix_tree.clear();
		// We only need to reset the initial registers;
		// see also: `[Tdfa::final_operations]`.
		self.registers[0..self.num_tags].fill(None);
		// self.registers[..].fill(None);
	}
}

impl Kernel {
	/// There should be no duplicate NFA states; see comment above on [`Kernel`].
	fn invariants(configurations: &[(Configuration, Vec<(CaptureTag, SymbolicPosition)>)]) {
		let states: Vec<NfaIdx> = configurations
			.iter()
			.map(|(config, _)| config.nfa_state)
			.collect::<Vec<_>>();
		let mut seen: BTreeSet<NfaIdx> = BTreeSet::new();
		let mut unique_states: Vec<NfaIdx> = states.clone();
		unique_states.retain(|state| seen.insert(*state));
		assert_eq!(states, unique_states);
	}

	/// Algorithm 3 in the [paper][tdfa].
	fn step_on_intervals(
		&self,
		nfa: &Tnfa,
	) -> IntervalTree<u32, Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)>> {
		use crate::interval_tree::PolicyExtend;

		let mut combined: IntervalTree<
			u32,
			Vec<(Configuration, Vec<(CaptureTag, SymbolicPosition)>)>,
		> = IntervalTree::new();

		for config in self.0.iter() {
			let nfa_state: &NfaState = &nfa[config.nfa_state];
			// Remark: Accepting states have no outgoing transitions.
			if let Transitions::Interval(transitions) = &nfa_state.transitions {
				for (interval, &target) in transitions.iter() {
					combined.insert(
						interval,
						vec![(
							Configuration {
								nfa_state: target,
								register_for_tag: config.register_for_tag.clone(),
								tag_path_in_closure: Vec::new(),
							},
							config.tag_path_in_closure.clone(),
						)],
						PolicyExtend,
					);
				}
			}
		}

		combined
	}

	/// Algorithm 3 in the [paper][tdfa].
	///
	/// Returns a map from register in `self` to register in `other`.
	fn try_register_bijection(&self, other: &Self) -> Option<BTreeMap<usize, usize>> {
		let lhs: &[Configuration] = &self.0;
		let rhs: &[Configuration] = &other.0;

		if lhs.len() != rhs.len() {
			return None;
		}

		let mut rhs_states: Vec<usize> = vec![usize::MAX; lhs.len()];

		// Do they contain the same NFA states with the same lookahead tags?
		for (i, x) in lhs.iter().enumerate() {
			rhs_states[i] = rhs.iter().position(|y| {
				(x.nfa_state == y.nfa_state) && (x.tag_path_in_closure == y.tag_path_in_closure)
			})?;
		}

		// `m1`: register in `lhs` -> register in `rhs`.
		// `m2`: register in `rhs` -> register in `lhs`.
		let mut m1: BTreeMap<usize, usize> = BTreeMap::new();
		let mut m2: BTreeMap<usize, usize> = BTreeMap::new();

		for (i, &j) in rhs_states.iter().enumerate() {
			let x: &Configuration = &lhs[i];
			let y: &Configuration = &rhs[j];
			assert_eq!(x.nfa_state, y.nfa_state);
			assert_eq!(x.register_for_tag.len(), y.register_for_tag.len());
			for tag_idx in 0..x.register_for_tag.len() {
				let i: usize = x.register_for_tag[tag_idx];
				let j: usize = y.register_for_tag[tag_idx];
				match (m1.entry(i), m2.entry(j)) {
					(Entry::Vacant(e1), Entry::Vacant(e2)) => {
						// Associate `i` (in `lhs`) with `j` (in `rhs`).
						e1.insert(j);
						e2.insert(i);
					},
					(Entry::Occupied(e1), Entry::Occupied(e2)) => {
						// Unless `m1[i] == m2[j]`, the bijection breaks.
						if (*e1.get() != j) || (*e2.get() != i) {
							return None;
						}
					},
					_ => {
						// Something doesn't match - not a bijection.
						return None;
					},
				}
			}
		}

		Some(m1)
	}
}

impl Transition {
	fn invalid() -> Self {
		Self {
			target: usize::MAX,
			operations: Vec::new(),
		}
	}

	fn is_valid(&self) -> bool {
		self.target != usize::MAX
	}
}

impl PrefixTree {
	const ROOT_NODE: PrefixTreeNode = PrefixTreeNode {
		maybe_predecessor: None,
		lexeme_position: 0,
	};

	fn new() -> Self {
		Self {
			nodes: vec![Self::ROOT_NODE],
		}
	}

	fn add_node(
		&mut self,
		maybe_predecessor: Option<NonZero<usize>>,
		lexeme_position: usize,
	) -> NonZero<usize> {
		assert!(maybe_predecessor.map_or(0, NonZero::get) < self.nodes.len());
		let len: NonZero<usize> = NonZero::new(self.nodes.len())
			.expect("prefix tree should always be constructed with a root node");
		self.nodes.push(PrefixTreeNode {
			maybe_predecessor,
			lexeme_position,
		});
		len
	}

	fn clear(&mut self) {
		self.nodes.clear();
		self.nodes.push(Self::ROOT_NODE);
	}
}

impl std::ops::Index<NonZero<usize>> for PrefixTree {
	type Output = PrefixTreeNode;

	fn index(&self, i: NonZero<usize>) -> &Self::Output {
		&self.nodes[i.get()]
	}
}
