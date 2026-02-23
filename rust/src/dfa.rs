/// Based on Angelo Borsotti and Ulya Trafimovich. 2022. A closer look at TDFA.
/// - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
/// - <https://arxiv.org/abs/2206.01398>
///
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;
use std::num::NonZero;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::nfa::AutomataCapture;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::SpontaneousTransitionKind;
use crate::nfa::Tag;
use crate::nfa::Tnfa;

#[derive(Debug, Clone)]
pub struct Tdfa {
	states: Vec<DfaState>,
	kernels: BTreeMap<Kernel, usize>,
	/// Bijection between `tags` and `{ 0..tags.len() }`.
	tags: Vec<Tag>,
	tag_ids: BTreeMap<Tag, DfaTagIdx>,
	/// During construction, this is the "current" count;
	/// after construction, this is the "total required".
	/// The first `tags.len()` are initial registers for the corresponding tags.
	/// The second `tags.len()` (i.e. `tags.len()..(2 * tags.len())`) are the corresponding final registers.
	number_of_registers: usize,
}

#[derive(Debug)]
pub struct MatchedRule<'input> {
	pub rule: usize,
	pub lexeme: &'input str,
}

#[derive(Debug, Clone)]
struct DfaState {
	kernel: Kernel,
	transitions: IntervalTree<u32, Transition>,
	/// If this is a final state (the kernel contains a final NFA state),
	/// the rule that this state has matched for.
	final_rule: Option<usize>,
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

/// A newtype around a tag ID/index in [`Tdfa::tag_ids`].
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct DfaTagIdx(usize);

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

#[derive(Debug)]
struct PrefixTree {
	nodes: Vec<PrefixTreeNode>,
}

#[derive(Debug)]
struct PrefixTreeNode {
	maybe_predecessor: Option<NonZero<usize>>,
	lexeme_position: usize,
}

#[derive(Debug)]
struct ExecutionData {
	dfa_state: usize,
	rule: usize,
	consumed: usize,
}

impl Tdfa {
	pub fn execute(&self, input: &str) -> bool {
		self.execute_with_captures(input, |capture, lexeme, _, _| {
			let capture: &AutomataCapture = self.capture_info(capture);
			debug!("- captured {capture:?}, {lexeme}");
		})
		.is_some()
	}

	pub fn execute_with_captures<'input, F>(&self, input: &'input str, mut on_capture: F) -> Option<MatchedRule<'input>>
	where
		F: FnMut(usize, &'input str, usize, usize),
	{
		let mut current_state: usize = 0;

		/// Vector of prefix tree node indices.
		let mut registers: Vec<Option<NonZero<usize>>> = vec![None; self.number_of_registers];
		let mut prefix_tree: PrefixTree = PrefixTree::new();

		let mut maybe_backup: Option<ExecutionData> = None;

		for (i, ch) in input.char_indices() {
			if let Some(transition) = self.lookup_transition(current_state, ch) {
				/// Apply transition operations before updating position; lookahead 1 in TDFA(1).
				self.apply_operations(
					&mut registers,
					&mut prefix_tree,
					i,
					&transition.operations,
					&self.states[current_state].tag_for_register,
				);
				current_state = transition.target;
				if let Some(rule) = self.states[current_state].final_rule {
					maybe_backup = Some(ExecutionData {
						dfa_state: current_state,
						rule,
						consumed: i + ch.len_utf8(),
					});
				}
			} else {
				break;
			}
		}

		let data: ExecutionData = maybe_backup?;

		self.apply_operations(
			&mut registers,
			&mut prefix_tree,
			data.consumed,
			&self.states[data.dfa_state].final_operations,
			&self.states[data.dfa_state].tag_for_register,
		);

		let mut captures: BTreeMap<&AutomataCapture, (usize, Vec<usize>, Vec<usize>)> = BTreeMap::new();
		for i in 0..self.tags.len() {
			let (Tag::StartCapture(capture) | Tag::StopCapture(capture)) = &self.tags[i];
			let (_, starts, ends): &mut (usize, Vec<usize>, Vec<usize>) =
				captures.entry(capture).or_insert((i, Vec::new(), Vec::new()));
			let offsets: &mut Vec<usize> = match self.tags[i] {
				Tag::StartCapture(_) => starts,
				Tag::StopCapture(_) => ends,
			};
			let mut maybe_node: Option<NonZero<usize>> = registers[self.tags.len() + i];
			while let Some(node) = maybe_node {
				offsets.push(prefix_tree[node].lexeme_position);
				maybe_node = prefix_tree[node].maybe_predecessor;
			}
			offsets.reverse();
		}

		for (_, (tag, starts, ends)) in captures.into_iter() {
			assert_eq!(starts.len(), ends.len());
			if starts.is_empty() {
				continue;
			}
			for (&i, &j) in std::iter::zip(starts.iter(), ends.iter()) {
				on_capture(tag, &input, i, j);
			}
		}

		Some(MatchedRule {
			rule: data.rule,
			lexeme: &input[..data.consumed],
		})
	}

	fn lookup_transition(&self, current_state: usize, ch: char) -> Option<&Transition> {
		let ch: u32 = u32::from(ch);
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
	pub fn capture_info(&self, i: usize) -> &AutomataCapture {
		let (Tag::StartCapture(capture) | Tag::StopCapture(capture)) = &self.tags[i];
		capture
	}
}

impl Tdfa {
	/// Algorithm 3 in the paper.
	#[tracing::instrument]
	pub fn determinization(nfa: &Tnfa) -> Self {
		let mut tag_ids: BTreeMap<Tag, DfaTagIdx> = BTreeMap::new();
		for (i, tag) in nfa.tags().iter().enumerate() {
			tag_ids.insert(tag.clone(), DfaTagIdx(i));
		}

		let mut dfa: Self = Self {
			states: Vec::new(),
			kernels: BTreeMap::new(),
			tags: nfa.tags().to_owned(),
			tag_ids,
			number_of_registers: 2 * nfa.tags().len(),
		};

		let initial: (Configuration, Vec<(Tag, SymbolicPosition)>) = (
			Configuration {
				nfa_state: nfa.begin(),
				register_for_tag: (0..dfa.tag_ids.len()).collect::<Vec<_>>(),
				tag_path_in_closure: Vec::new(),
			},
			Vec::new(),
		);

		let initial: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Self::epsilon_closure(nfa, vec![initial]);

		dfa.add_state(initial, &mut Vec::new());

		/// Note: New states may be created and appended to `dfa.states` inside the loop;
		/// `dfa.states.len()` is not constant.
		let mut i: usize = 0;
		while i < dfa.states.len() {
			/// Since we may append to `dfa.states`, it may resize
			/// and a reference to `dfa.states[i]` here would become invalid
			/// (borrow checker will complain without the `.clone()`.
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

				let next: usize = dfa.add_state(next, &mut operations);
				dfa.states[i].transitions.insert(
					interval,
					Transition {
						target: next,
						operations,
					},
					|existing, new| {
						panic!(
							"state {i} transition on {interval:?} had existing target {:?}, trying to insert {:?}",
							existing, new
						);
					},
				);
			}

			i += 1;
		}

		dfa.fallback_regops();

		for state in dfa.states.iter_mut() {
			for (i, cached_transition) in state.ascii_cache.iter_mut().enumerate() {
				/// It doesn't matter whether this is a (lossless) upcast (`usize::BITS <= u32::BITS`)
				/// or (lossy) downcast (`usize::BITS > u32::BITS`);
				/// a lossless cast is necessarily harmless,
				/// and a lossy downcast simply means the cache contains more slots than necessary,
				/// which won't be touched during simulation/lexing.
				if let Some(transition) = state.transitions.lookup(i as u32) {
					*cached_transition = transition.clone();
				}
			}
		}

		dfa
	}

	fn add_state(
		&mut self,
		configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
		ops: &mut Vec<RegisterOperation>,
	) -> usize {
		let mut final_rule: Option<usize> = None;
		let mut final_operations: Vec<RegisterOperation> = Vec::new();
		let mut tag_for_register: BTreeMap<usize, Tag> = BTreeMap::new();
		let configurations: Vec<Configuration> = configurations
			.into_iter()
			.map(|(config, _)| {
				if let Some(rule) = config.nfa_state.final_rule() {
					if final_rule.is_none() {
						final_rule = Some(rule);
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
			final_rule,
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
		/// Do they contain the same NFA states with the same lookahead tags?
		for x in lhs.iter() {
			rhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tag_path_in_closure == y.tag_path_in_closure))?;
		}
		for x in rhs.iter() {
			lhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tag_path_in_closure == y.tag_path_in_closure))?;
		}

		/// `m1`: register in `lhs` -> register in `rhs`.
		/// `m2`: register in `rhs` -> register in `lhs`.
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
							/// Associate `i` (in `lhs`) with `j` (in `rhs`).
							e1.insert(j);
							e2.insert(i);
						},
						(Entry::Occupied(e1), Entry::Occupied(e2)) => {
							/// Unless `m1[i] == m2[j]`, the bijection breaks.
							if (*e1.get() != j) || (*e2.get() != i) {
								return None;
							}
						},
						_ => {
							/// Something doesn't match - not a bijection.
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
		mut configurations: Vec<Configuration>,
		interval: Interval<u32>,
	) -> Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> {
		let mut next_states: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Vec::new();

		configurations.sort_by_key(|config| config.nfa_state);

		for config in configurations.iter() {
			if config.nfa_state.is_end() {
				continue;
			}

			let nfa_state: &NfaState = &nfa[config.nfa_state];
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

			if config.nfa_state.is_end() {
				continue;
			}

			for transition in nfa[config.nfa_state].spontaneous().iter() {
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
				if self.states[transition.target].final_rule.is_some() {
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
					if self.states[transition.target].final_rule.is_some() {
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

impl Kernel {
	/// There should be no duplicate NFA states; see comment above on [`Kernel`].
	fn invariants(configurations: &[(Configuration, Vec<(Tag, SymbolicPosition)>)]) {
		assert_eq!(
			configurations.len(),
			configurations
				.iter()
				.map(|(config, _)| config.nfa_state)
				.collect::<BTreeSet<_>>()
				.len()
		);
	}

	fn nontrivial_transitions(&self, nfa: &Tnfa) -> Vec<Interval<u32>> {
		let mut transitions: IntervalTree<u32, ()> = IntervalTree::new();

		for config in self.0.iter() {
			if config.nfa_state.is_end() {
				continue;
			}

			let nfa_state: &NfaState = &nfa[config.nfa_state];
			for (interval, _) in nfa_state.transitions().iter() {
				transitions.insert(interval, (), |(), ()| ());
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
	fn new() -> Self {
		Self {
			nodes: vec![PrefixTreeNode {
				maybe_predecessor: None,
				lexeme_position: 0,
			}],
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
}

impl std::ops::Index<NonZero<usize>> for PrefixTree {
	type Output = PrefixTreeNode;

	fn index(&self, i: NonZero<usize>) -> &Self::Output {
		&self.nodes[i.get()]
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::regex::Regex;
	use crate::schema::Schema;

	#[test]
	fn stuff() {
		{
			let r: Regex = Regex::from_pattern("0((?<foobar>1(2[a-zA-Z])*)*|(?<baz>xyz))*world").unwrap();
			let mut schema: Schema = Schema::new();
			schema.add_rule("hello", r);
			let dfa: Tdfa = schema.build_dfa();
			dbg!(&dfa);
			let b: bool = dfa.execute("012a2b2c12z12zxyzxyzxyzworld");
			assert!(b);
		}
	}
}
