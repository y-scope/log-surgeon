//! Based on Angelo Borsotti and Ulya Trafimovich. 2022. A closer look at TDFA.
//! - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
//! - <https://arxiv.org/abs/2206.01398>
//!
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;
use std::num::NonZero;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::PolicyFunction;
use crate::interval_tree::PolicyNoop;
use crate::nfa::AutomataCapture;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::SpontaneousTransitionKind;
use crate::nfa::Tag;
use crate::nfa::Tnfa;
use crate::schema::Rule;
use crate::schema::RuleIdx;
use crate::search::SymbolicChar;
use crate::utils::Range;

#[derive(Debug, Clone)]
pub struct Tdfa {
	states: Vec<DfaState>,
	kernels: BTreeMap<Kernel, usize>,
	/// Bijection between `tags` and `{ 0..tags.len() }`.
	tags: Vec<Tag>,
	tag_pairs: Vec<usize>,
	/// During construction, this is the "current" count;
	/// after construction, this is the "total required".
	/// The first `tags.len()` are initial registers for the corresponding tags.
	/// The second `tags.len()` (i.e. `tags.len()..(2 * tags.len())`) are the corresponding final registers.
	number_of_registers: usize,
	// delimiters: String,
	anchor_ch: char,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct TdfaExecution {
	pub captures: Vec<MatchedCapture>,
	registers: Vec<Option<NonZero<usize>>>,
	prefix_tree: PrefixTree,
	num_tags: usize,
}

#[derive(Debug)]
pub struct MatchedRule<'input> {
	pub rule: RuleIdx,
	pub lexeme: &'input str,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct MatchedCapture {
	pub rule: RuleIdx,
	pub capture_id: NonZero<u32>,
	pub parent_id: Option<NonZero<u32>>,
	pub parent_index: usize,
	pub is_leaf: bool,
	/// Relative to rule/variable match.
	pub range: Range<usize>,
}

#[derive(Debug, Clone)]
struct DfaState {
	kernel: Kernel,
	transitions: IntervalTree<u32, Transition>,
	/// If this is a final state (the kernel contains an accepting NFA state),
	/// the rule that this state has matched for,
	/// and whether we should "backtrack" a symbol (used to implement ends-with lookahead/anchor).
	accepting_rule: Option<(RuleIdx, bool)>,
	/// Register operations upon finalizing a match (if applicable); copy to the final registers.
	final_operations: Vec<RegisterOperation>,
	/// Cache/combined map from this state's configurations of "register -> which tag it holds".
	/// Present for debugging.
	tag_for_register: BTreeMap<usize, Tag>,
	/// Registers that may be clobbered after leaving this state.
	/// See [`Tdfa::compute_registers_clobbered`].
	registers_clobbered: BTreeSet<usize>,
	/// We cache the outgoing transitions for the first so many "common" characters;
	/// ASCII is most common and happens to be the first 0x80 unicode code points.
	/// However, the cache size can be changed here without touching the rest of the code.
	/// Of course, in practice, the cache is presumed to be much much smaller
	/// than the full range of unicode code points,
	/// but technically the code should work for any value here;
	/// comments in the relevant parts of the implementation explain why.
	ascii_cache: [Transition; 0x80],
}

/// In untagged DFA, the kernel of a DFA state is simply the set of corresponding NFA states;
/// all other information, including state transitions, are derived from the kernel.
/// In tagged TDFA, the kernel of a TDFA state is a set of corresponding [`Configuration`]s.
///
/// Conceptually, a kernel may more accurately be represented using `BTreeMap<NfaIdx, Configuration>`;
/// an NFA state should not show up more than once in a kernel, since:
///
/// 1. by construction, distinct NFA states have distinct "next states",
///    so [`Tdfa::step_on_interval`] always "lands on" a list of distinct NFA states, and
/// 2. the [`Tdfa::epsilon_closure`] procedure (using a depth-first search)
///    only saves the first path of NFA states reachable through epsilon transitions.
///
/// However, both of the aforementioned procedures operate more naturally on a list of `Configuration`s,
/// and `Vec<Configuration>` naturally has better memory locality.
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Kernel(Vec<Configuration>);

/// A "configuration" is essentially an augmented NFA state (as documented per field).
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Configuration {
	nfa_state: NfaIdx,
	/// A mapping "tag (by ID/index) -> register"; answers "which register holds this tag?".
	register_for_tag: Vec<usize>,
	/// Sequence of tags accumulated to reach this state during [`Dfa::epsilon_closure`]
	/// (corresponding to the execution of positive/negative tags during NFA simulation).
	tag_path_in_closure: Vec<(Tag, SymbolicPosition)>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
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
	dfa_state: usize,
	rule: RuleIdx,
	consumed: usize,
}

impl Tdfa {
	pub fn execute(&self, input: &str) -> bool {
		self.execute_without_captures(input, 0, &mut self.execution_data())
			.is_some()
	}

	pub fn execute_without_captures<'input>(
		&self,
		input: &'input str,
		last_was_delimited: u32,
		execution_data: &mut TdfaExecution,
	) -> Option<MatchedRule<'input>> {
		self.execute_internal::<false>(input, last_was_delimited, execution_data)
	}

	pub fn execute_with_captures<'input>(
		&self,
		input: &'input str,
		last_was_delimited: u32,
		data: &mut TdfaExecution,
	) -> Option<MatchedRule<'input>> {
		self.execute_internal::<true>(input, last_was_delimited, data)
	}

	#[tracing::instrument(skip_all, level = "trace")]
	pub fn execute_with_captures_for_search<'input>(
		&self,
		input: &'input str,
		last_was_delimited: u32,
		data: &mut TdfaExecution,
	) -> Option<MatchedRule<'input>> {
		self.execute_internal::<true>(input, last_was_delimited, data)
	}

	fn execute_internal<'input, const CAPTURE: bool>(
		&self,
		input: &'input str,
		last_was_delimited: u32,
		execution_data: &mut TdfaExecution,
	) -> Option<MatchedRule<'input>> {
		let Some(anchor_transition): Option<&Transition> = self.lookup_transition(0, last_was_delimited) else {
			panic!("invalid first transition");
		};
		let mut current_state: usize = anchor_transition.target;

		execution_data.clear();

		// Vector of prefix tree node indices.
		let registers: &mut [Option<NonZero<usize>>] = &mut execution_data.registers;
		let prefix_tree: &mut PrefixTree = &mut execution_data.prefix_tree;
		let captures: &mut Vec<MatchedCapture> = &mut execution_data.captures;

		let mut maybe_backup: Option<BackupState> = None;

		for (pos, ch) in input
			.char_indices()
			.chain(std::iter::once((input.len(), self.anchor_ch)))
		{
			if let Some(transition) = self.lookup_transition(current_state, u32::from(ch)) {
				if CAPTURE {
					// Apply transition operations before updating position; lookahead 1 in TDFA(1).
					self.apply_operations(
						registers,
						prefix_tree,
						pos,
						&transition.operations,
						&self.states[current_state].tag_for_register,
					);
				}
				current_state = transition.target;
				if let Some((rule, backtrack)) = self.states[current_state].accepting_rule {
					if (pos < input.len()) || backtrack {
						// The current character is a "real input" (not the chained EOF pseudo-symbol),
						// or the accepting state is after a lookahead and will backtrack...

						let consumed: usize = pos + if backtrack { 0 } else { ch.len_utf8() };

						if let Some(backup) = maybe_backup
							&& backup.consumed >= consumed
						{
							// If the previously matched rule's lexeme was at least as long
							// (technically, it can only be exactly as long, since it can't be longer),
							// then keep the previous rule, since it has higher priority.
							// Equivalently, "if we matched via an anchor, don't overwrite the previous rule
							// if it also matched the full text".
							continue;
						}

						maybe_backup = Some(BackupState {
							dfa_state: current_state,
							rule,
							consumed,
						});
					}
				}
			} else {
				break;
			}
		}

		let backup: BackupState = maybe_backup?;

		if CAPTURE {
			self.apply_operations(
				registers,
				prefix_tree,
				backup.consumed,
				&self.states[backup.dfa_state].final_operations,
				&self.states[backup.dfa_state].tag_for_register,
			);

			for (start, stop) in self.tag_pairs.iter().enumerate().rev() {
				let capture: &AutomataCapture = self.tags[start].capture();

				let mut maybe_start: Option<NonZero<usize>> = registers[self.tags.len() + start];
				let mut maybe_stop: Option<NonZero<usize>> = registers[self.tags.len() + stop];

				while let Some(start_node) = maybe_start {
					let stop_node: NonZero<usize> = maybe_stop.unwrap();

					let start: usize = prefix_tree[start_node].lexeme_position;
					let end: usize = prefix_tree[stop_node].lexeme_position;
					captures.push(MatchedCapture {
						rule: backup.rule,
						capture_id: capture.capture_info.id,
						parent_id: capture.capture_info.parent_id,
						parent_index: usize::MAX,
						is_leaf: capture.capture_info.is_leaf(),
						range: Range { start, end },
					});
					maybe_start = prefix_tree[start_node].maybe_predecessor;
					maybe_stop = prefix_tree[stop_node].maybe_predecessor;
				}
			}
			captures.sort_by(|lhs, rhs| {
				lhs.range
					.start
					.cmp(&rhs.range.start)
					.then(lhs.range.end.cmp(&rhs.range.end).reverse())
					.then(lhs.capture_id.cmp(&rhs.capture_id))
			});
			for i in 0..captures.len() {
				if let Some(parent_id) = captures[i].parent_id {
					// Linear search since it should usually be small.
					for j in 0..i {
						if captures[j].capture_id == parent_id
							&& (captures[j].range.start <= captures[i].range.start)
							&& (captures[i].range.end <= captures[j].range.end)
						{
							captures[i].parent_index = j;
						}
					}
					// TODO Happens in search.
					// assert_ne!(captures[i].parent_index, usize::MAX);
				} else {
					captures[i].parent_index = 0;
				}
			}
		}

		Some(MatchedRule {
			rule: backup.rule,
			lexeme: &input[..backup.consumed],
		})
	}

	fn lookup_transition(&self, current_state: usize, ch: u32) -> Option<&Transition> {
		let cache_index: usize = {
			const _: () = const {
				assert!(usize::BITS >= u32::BITS, "lossy cast from `u32` to `usize`");
			};
			ch as usize
		};
		let current_state: &DfaState = &self.states[current_state];
		if let Some(transition) = current_state.ascii_cache.get(cache_index) {
			if transition.is_valid() { Some(transition) } else { None }
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
		_tag_for_register: &BTreeMap<usize, Tag>,
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
							let node: NonZero<usize> = prefix_tree.add_node(registers[o.destination], pos);
							registers[o.destination] = Some(node);
						}
					}
				},
			}
		}
	}
}

impl Tdfa {
	pub fn simulate(&self, input: &[SymbolicChar]) -> Vec<(RuleIdx, Vec<MatchedCapture>)> {
		#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
		struct Path {
			position: usize,
			state: usize,
			registers: Vec<Option<NonZero<usize>>>,
			prefix_tree: PrefixTree,
		}

		let Some(anchor_transition): Option<&Transition> = self.lookup_transition(0, u32::from(self.anchor_ch)) else {
			panic!("invalid first transition");
		};

		let initial_path: Path = Path {
			position: 0,
			state: anchor_transition.target,
			registers: vec![None; self.number_of_registers],
			prefix_tree: PrefixTree::new(),
		};

		let mut current_paths: Vec<Path> = vec![initial_path];
		let mut seen: BTreeSet<Path> = BTreeSet::from_iter(current_paths.iter().cloned());
		let mut final_states: Vec<(RuleIdx, Vec<MatchedCapture>)> = Vec::new();

		while let Some(mut path) = current_paths.pop() {
			let registers: &mut Vec<Option<NonZero<usize>>> = &mut path.registers;
			let prefix_tree: &mut PrefixTree = &mut path.prefix_tree;
			let Some(ch): Option<SymbolicChar> = input.get(path.position).copied() else {
				if let Some((rule, needs_backtrack)) = self.states[path.state].accepting_rule {
					if needs_backtrack {
						todo!();
					}
					self.apply_operations(
						registers,
						prefix_tree,
						path.position,
						&self.states[path.state].final_operations,
						&self.states[path.state].tag_for_register,
					);
					let mut captures: Vec<MatchedCapture> = Vec::new();
					for (start, stop) in self.tag_pairs.iter().enumerate().rev() {
						let capture: &AutomataCapture = self.tags[start].capture();

						let mut maybe_start: Option<NonZero<usize>> = registers[self.tags.len() + start];
						let mut maybe_stop: Option<NonZero<usize>> = registers[self.tags.len() + stop];

						while let Some(start_node) = maybe_start {
							let stop_node: NonZero<usize> = maybe_stop.unwrap();

							let start: usize = prefix_tree[start_node].lexeme_position;
							let end: usize = prefix_tree[stop_node].lexeme_position;
							captures.push(MatchedCapture {
								rule,
								capture_id: capture.capture_info.id,
								parent_id: capture.capture_info.parent_id,
								parent_index: usize::MAX,
								is_leaf: capture.capture_info.is_leaf(),
								range: Range { start, end },
							});
							maybe_start = prefix_tree[start_node].maybe_predecessor;
							maybe_stop = prefix_tree[stop_node].maybe_predecessor;
						}
					}
					captures.sort_by(|lhs, rhs| {
						lhs.range
							.start
							.cmp(&rhs.range.start)
							.then(lhs.range.end.cmp(&rhs.range.end).reverse())
							.then(lhs.capture_id.cmp(&rhs.capture_id))
					});
					for i in 0..captures.len() {
						if let Some(parent_id) = captures[i].parent_id {
							// Linear search since it should usually be small.
							for j in 0..i {
								if captures[j].capture_id == parent_id
									&& (captures[j].range.start <= captures[i].range.start)
									&& (captures[i].range.end <= captures[j].range.end)
								{
									captures[i].parent_index = j;
								}
							}
							// TODO Happens in search.
							// assert_ne!(captures[i].parent_index, usize::MAX);
						}
					}
					final_states.push((rule, captures));
				}
				continue;
			};
			match ch {
				SymbolicChar::Literal(ch) => {
					if let Some(transition) = self.lookup_transition(path.state, u32::from(ch)) {
						self.apply_operations(
							registers,
							prefix_tree,
							path.position,
							&transition.operations,
							&self.states[path.state].tag_for_register,
						);
						let new_path: Path = Path {
							position: path.position + 1,
							state: transition.target,
							registers: path.registers,
							prefix_tree: path.prefix_tree,
						};
						if !seen.contains(&new_path) {
							current_paths.push(new_path.clone());
							seen.insert(new_path);
						}
					}
				},
				SymbolicChar::WildcardOne => {
					for (_interval, transition) in self.states[path.state].transitions.iter() {
						self.apply_operations(
							registers,
							prefix_tree,
							path.position,
							&transition.operations,
							&self.states[path.state].tag_for_register,
						);
						let new_path: Path = Path {
							position: path.position + 1,
							state: transition.target,
							registers: registers.clone(),
							prefix_tree: prefix_tree.clone(),
						};
						if !seen.contains(&new_path) {
							current_paths.push(new_path.clone());
							seen.insert(new_path);
						}
					}
				},
				SymbolicChar::WildcardStar => {
					for (_interval, transition) in self.states[path.state].transitions.iter() {
						self.apply_operations(
							registers,
							prefix_tree,
							path.position,
							&transition.operations,
							&self.states[path.state].tag_for_register,
						);
						{
							// Wildcard, didn't advance.
							let new_path: Path = Path {
								position: path.position,
								state: transition.target,
								registers: registers.clone(),
								prefix_tree: prefix_tree.clone(),
							};
							if !seen.contains(&new_path) {
								current_paths.push(new_path.clone());
								seen.insert(new_path);
							}
						}
						{
							// Wildcard, advanced.
							let new_path: Path = Path {
								position: path.position + 1,
								state: transition.target,
								registers: registers.clone(),
								prefix_tree: prefix_tree.clone(),
							};
							if !seen.contains(&new_path) {
								current_paths.push(new_path.clone());
								seen.insert(new_path);
							}
						}
					}
				},
			}
		}

		final_states
	}
}

impl Tdfa {
	pub fn capture_info(&self, i: usize) -> &AutomataCapture {
		self.tags[i].capture()
	}

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
	pub fn for_rules<'a, Rules>(rules: Rules, delimiters: String) -> Self
	where
		Rules: IntoIterator<Item = &'a Rule>,
	{
		let nfa: Tnfa = Tnfa::for_rules(rules, delimiters.clone());
		Self::determinization(&nfa, delimiters)
	}

	/// Algorithm 3 in the paper.
	#[tracing::instrument(skip_all, level = "trace")]
	fn determinization(nfa: &Tnfa, delimiters: String) -> Self {
		assert_eq!(nfa.tags().len() % 2, 0);
		let mut tag_pairs: Vec<usize> = Vec::with_capacity(nfa.tags().len() / 2);
		for (i, tag) in nfa.tags().iter().enumerate() {
			if i >= nfa.tags().len() / 2 {
				break;
			}
			let j: usize = i + nfa.tags().len() / 2;
			assert_eq!(tag.capture(), nfa.tags()[j].capture());
			tag_pairs.push(j);
		}

		let anchor_ch: char = delimiters.chars().next().unwrap();
		let mut dfa: Self = Self {
			states: Vec::new(),
			kernels: BTreeMap::new(),
			tags: nfa.tags().to_owned(),
			tag_pairs,
			number_of_registers: 2 * nfa.tags().len(),
			// delimiters,
			anchor_ch,
		};

		let initial: (Configuration, Vec<(Tag, SymbolicPosition)>) = (
			Configuration {
				nfa_state: nfa.begin(),
				register_for_tag: (0..dfa.tags.len()).collect::<Vec<_>>(),
				tag_path_in_closure: Vec::new(),
			},
			Vec::new(),
		);

		let initial: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Self::epsilon_closure(nfa, vec![initial]);

		dfa.add_state(nfa, initial, &mut Vec::new());

		// Note: New states may be created and appended to `dfa.states` inside the loop;
		// `dfa.states.len()` is not constant.
		let mut i: usize = 0;
		while i < dfa.states.len() {
			// Since we may append to `dfa.states`, it may resize
			// and a reference to `dfa.states[i]` here would become invalid
			// (borrow checker will complain without the `.clone()`.
			let kernel: Kernel = dfa.states[i].kernel.clone();

			let mut register_action_tag: BTreeMap<(Tag, RegisterAction), usize> = BTreeMap::new();
			for &interval in kernel.nontrivial_transitions(nfa).iter() {
				let next: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> =
					Self::step_on_interval(nfa, kernel.0.clone(), interval);
				let next: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Self::epsilon_closure(nfa, next);

				let (next, mut operations): (
					Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
					Vec<RegisterOperation>,
				) = dfa.transition_operations(next, &mut register_action_tag);

				let next: usize = dfa.add_state(nfa, next, &mut operations);
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

		dfa.fallback_regops();

		for state in dfa.states.iter_mut() {
			for (i, cached_transition) in state.ascii_cache.iter_mut().enumerate() {
				// It doesn't matter whether this is a (lossless) upcast (`usize::BITS <= u32::BITS`)
				// or (lossy) downcast (`usize::BITS > u32::BITS`);
				// a lossless cast is necessarily harmless,
				// and a lossy downcast simply means the cache contains more slots than necessary,
				// which won't be touched during simulation/lexing.
				if let Some(transition) = state.transitions.lookup(i as u32) {
					*cached_transition = transition.clone();
				}
			}
		}

		dfa
	}

	fn add_state(
		&mut self,
		nfa: &Tnfa,
		configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
		ops: &mut Vec<RegisterOperation>,
	) -> usize {
		let mut accepting_rule: Option<(RuleIdx, bool)> = None;
		let mut final_operations: Vec<RegisterOperation> = Vec::new();
		let mut tag_for_register: BTreeMap<usize, Tag> = BTreeMap::new();
		let configurations: Vec<Configuration> = configurations
			.into_iter()
			.map(|(config, _)| {
				if let Some((rule, needs_backtrack)) = nfa[config.nfa_state].accepts_for_rule() {
					if accepting_rule.is_none() {
						accepting_rule = Some((rule, needs_backtrack));
					}
					final_operations = self.final_operations(&config.register_for_tag, &config.tag_path_in_closure);
				}
				config
			})
			.collect::<Vec<_>>();
		let kernel: Kernel = Kernel(configurations);

		if let Some(&idx) = self.kernels.get(&kernel) {
			return idx;
		}

		for (other, &idx) in self.kernels.iter() {
			if let Some(new_ops) = self.try_find_bijection(&kernel.0, &other.0, ops.clone()) {
				*ops = new_ops;
				return idx;
			}
		}

		for config in kernel.0.iter() {
			for (i, &r) in config.register_for_tag.iter().enumerate() {
				let old: Option<Tag> = tag_for_register.insert(r, self.tags[i].clone());
				assert!(old.is_none() || (old.as_ref() == Some(&self.tags[i])));
			}
		}

		let idx: usize = self.states.len();
		self.states.push(DfaState {
			kernel: kernel.clone(),
			transitions: IntervalTree::new(),
			accepting_rule,
			final_operations,
			tag_for_register,
			registers_clobbered: BTreeSet::new(),
			ascii_cache: std::array::from_fn(|_| Transition::invalid()),
		});
		self.kernels.insert(kernel, idx);
		idx
	}

	fn try_find_bijection(
		&self,
		lhs: &[Configuration],
		rhs: &[Configuration],
		mut ops: Vec<RegisterOperation>,
	) -> Option<Vec<RegisterOperation>> {
		// Do they contain the same NFA states with the same lookahead tags?
		for x in lhs.iter() {
			rhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tag_path_in_closure == y.tag_path_in_closure))?;
		}
		for x in rhs.iter() {
			lhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tag_path_in_closure == y.tag_path_in_closure))?;
		}

		// `m1`: register in `lhs` -> register in `rhs`.
		// `m2`: register in `rhs` -> register in `lhs`.
		let mut m1: BTreeMap<usize, usize> = BTreeMap::new();
		let mut m2: BTreeMap<usize, usize> = BTreeMap::new();

		for x in lhs.iter() {
			for y in rhs.iter() {
				if x.nfa_state != y.nfa_state {
					continue;
				}
				for tag_idx in 0..self.tags.len() {
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
		}

		for o in ops.iter_mut() {
			o.destination = m1.remove(&o.destination).unwrap();
		}
		let mut copies: Vec<RegisterOperation> = Vec::new();
		for (&j, &i) in m1.iter() {
			copies.push(RegisterOperation {
				destination: i,
				action: RegisterAction::CopyFrom { source: j },
			});
		}

		ops.extend_from_slice(&copies[..]);

		Self::topological_sort(ops)
	}

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
						RegisterAction::CopyFrom { source } | RegisterAction::Append { source, .. } => {
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
						RegisterAction::CopyFrom { source } | RegisterAction::Append { source, .. } => {
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

	fn step_on_interval(
		nfa: &Tnfa,
		configurations: Vec<Configuration>,
		interval: Interval<u32>,
	) -> Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> {
		let mut next_states: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Vec::new();

		for config in configurations.iter() {
			let nfa_state: &NfaState = &nfa[config.nfa_state];
			// Remark: Accepting states have no outgoing transitions.
			if let Some(targets) = nfa_state.transitions().lookup(interval.start()) {
				assert!(
					nfa_state
						.transitions()
						.lookup_interval(interval.start())
						.unwrap()
						.contains(&interval)
				);
				for &next in targets.iter() {
					next_states.push((
						Configuration {
							nfa_state: next,
							register_for_tag: config.register_for_tag.clone(),
							tag_path_in_closure: Vec::new(),
						},
						config.tag_path_in_closure.clone(),
					));
				}
			}
		}

		Kernel::invariants(&next_states);
		next_states
	}

	fn epsilon_closure(
		nfa: &Tnfa,
		configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
	) -> Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> {
		let mut closure: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Vec::new();

		let mut nfa_states_on_stack: BTreeSet<NfaIdx> = configurations
			.iter()
			.map(|(config, _)| config.nfa_state)
			.collect::<BTreeSet<_>>();

		let mut stack: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = configurations.clone();
		stack.reverse();

		while let Some((config, inherited)) = stack.pop() {
			closure.push((config.clone(), inherited.clone()));

			// Remark: Accepting states have no outgoing transitions.
			for transition in nfa[config.nfa_state].spontaneous().iter().rev() {
				if nfa_states_on_stack.contains(&transition.target) {
					continue;
				}

				let mut new_config: Configuration = Configuration {
					nfa_state: transition.target,
					..config.clone()
				};

				match &transition.kind {
					SpontaneousTransitionKind::Positive(tag) => {
						new_config
							.tag_path_in_closure
							.push((tag.clone(), SymbolicPosition::Current));
					},
					SpontaneousTransitionKind::Negative(tag) => {
						new_config
							.tag_path_in_closure
							.push((tag.clone(), SymbolicPosition::Nil));
					},
					SpontaneousTransitionKind::Epsilon => (),
				}

				nfa_states_on_stack.insert(new_config.nfa_state);
				stack.push((new_config, inherited.clone()));
			}
		}

		Kernel::invariants(&closure);
		closure
	}

	fn transition_operations(
		&mut self,
		configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
		register_action_tag: &mut BTreeMap<(Tag, RegisterAction), usize>,
	) -> (
		Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
		Vec<RegisterOperation>,
	) {
		let mut new_configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Vec::new();
		let mut ops: BTreeSet<RegisterOperation> = BTreeSet::new();

		for (mut config, inherited) in configurations.into_iter() {
			for (tag_idx, tag) in self.tags.iter().enumerate() {
				let history: Vec<SymbolicPosition> = Self::filter_history_for_tag(&inherited, tag);
				if history.is_empty() {
					continue;
				}
				let action: RegisterAction = Self::operation_rhs(&config.register_for_tag, history, tag_idx);
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

	fn final_operations(&self, registers: &[usize], history: &[(Tag, SymbolicPosition)]) -> Vec<RegisterOperation> {
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

	fn operation_rhs(registers: &[usize], history: Vec<SymbolicPosition>, tag_idx: usize) -> RegisterAction {
		RegisterAction::Append {
			source: registers[tag_idx],
			history,
		}
	}

	fn filter_history_for_tag(history: &[(Tag, SymbolicPosition)], tag1: &Tag) -> Vec<SymbolicPosition> {
		history
			.iter()
			.filter_map(|(tag2, pos)| if tag2 == tag1 { Some(*pos) } else { None })
			.collect::<Vec<_>>()
	}
}

impl Tdfa {
	/// Algorithm 4 in the paper.
	fn fallback_regops(&mut self) {
		for i in 0..self.states.len() {
			self.states[i].registers_clobbered = self.compute_registers_clobbered(i);
		}
		for i in 0..self.states.len() {
			let mut backup_ops: Vec<RegisterOperation> = Vec::new();
			let mut transitions: IntervalTree<u32, Transition> = self.states[i].transitions.clone();
			for (_, transition) in transitions.iter_mut() {
				if self.states[transition.target].accepting_rule.is_some() {
					continue;
				}
				for final_op in self.states[i].final_operations.iter() {
					if self.states[transition.target]
						.registers_clobbered
						.contains(&final_op.action.source())
					{
						transition.operations.push(RegisterOperation {
							destination: final_op.destination,
							action: RegisterAction::CopyFrom {
								source: final_op.action.source(),
							},
						});
						if let RegisterAction::Append { source, history } = &final_op.action {
							backup_ops.push(RegisterOperation {
								destination: final_op.destination,
								action: RegisterAction::Append {
									source: *source,
									history: history.clone(),
								},
							});
						}
					}
				}
			}
			self.states[i].transitions = transitions;
			std::mem::swap(&mut self.states[i].final_operations, &mut backup_ops);
			self.states[i].final_operations.extend_from_slice(&backup_ops);
		}
	}

	fn compute_registers_clobbered(&self, state: usize) -> BTreeSet<usize> {
		let mut clobbered: BTreeSet<usize> = BTreeSet::new();
		let mut visited: BTreeSet<usize> = BTreeSet::new();
		let mut stack: Vec<usize> = vec![state];
		while let Some(state) = stack.pop() {
			for (_, transition) in self.states[state].transitions.iter() {
				for op in transition.operations.iter() {
					if self.states[transition.target].accepting_rule.is_some() {
						continue;
					}
					clobbered.insert(op.destination);
					if visited.insert(transition.target) {
						stack.push(transition.target);
					}
				}
			}
		}
		clobbered
	}
}

impl TdfaExecution {
	fn clear(&mut self) {
		self.captures.clear();
		self.prefix_tree.clear();
		// TODO explain why don't need to clear rest of the registers
		for register in self.registers[0..self.num_tags].iter_mut() {
			*register = None;
		}
	}
}

impl Kernel {
	/// There should be no duplicate NFA states; see comment above on [`Kernel`].
	fn invariants(configurations: &[(Configuration, Vec<(Tag, SymbolicPosition)>)]) {
		let states: Vec<NfaIdx> = configurations
			.iter()
			.map(|(config, _)| config.nfa_state)
			.collect::<Vec<_>>();
		let mut seen: BTreeSet<NfaIdx> = BTreeSet::new();
		let mut unique_states: Vec<NfaIdx> = states.clone();
		unique_states.retain(|state| seen.insert(*state));
		assert_eq!(states, unique_states);
	}

	fn nontrivial_transitions(&self, nfa: &Tnfa) -> Vec<Interval<u32>> {
		let mut transitions: IntervalTree<u32, ()> = IntervalTree::new();

		for config in self.0.iter() {
			let nfa_state: &NfaState = &nfa[config.nfa_state];
			// Remark: Accepting states have no outgoing transitions.
			for (interval, _) in nfa_state.transitions().iter() {
				transitions.insert(interval, (), PolicyNoop);
			}
		}

		transitions.iter().map(|(interval, _)| interval).collect::<Vec<_>>()
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

impl RegisterAction {
	fn source(&self) -> usize {
		match self {
			&Self::CopyFrom { source } => source,
			Self::Append { source, .. } => *source,
		}
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

	fn add_node(&mut self, maybe_predecessor: Option<NonZero<usize>>, lexeme_position: usize) -> NonZero<usize> {
		assert!(maybe_predecessor.map_or(0, NonZero::get) < self.nodes.len());
		let len: NonZero<usize> =
			NonZero::new(self.nodes.len()).expect("prefix tree should always be constructed with a root node");
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

/*
#[cfg(test)]
mod test {
	use super::*;
	use crate::schema::Schema;

	#[test]
	fn complex_pattern() {
		let mut schema: Schema = Schema::new();
		schema
			.add_rule("hello", "0((?<foobar>1(2[a-zA-Z])*)*|(?<baz>xyz))*world")
			.unwrap();
		let dfa: Tdfa = schema.build_dfa();
		let b: bool = dfa.execute("012a2b2c12z12zxyzxyzxyzworld");
		assert!(b);
	}

	#[test]
	fn group_with_overlapping_range() {
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", r"[aa]").unwrap();
		let dfa: Tdfa = schema.build_dfa();
		let b: bool = dfa.execute("a");
		assert!(b);
	}
}
*/
