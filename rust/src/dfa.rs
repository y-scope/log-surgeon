use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::nfa::AutomataCapture;
use crate::nfa::Nfa;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::SpontaneousTransitionKind;
use crate::nfa::Tag;

#[derive(Debug)]
pub struct Dfa {
	states: Vec<DfaState>,
	kernels: BTreeMap<Kernel, usize>,
	/// Bijection between `tags` and `{ 0..tags.len() }`.
	tags: Vec<Tag>,
	tag_ids: BTreeMap<Tag, DfaTag>,
	/// During construction, this is the "current" count;
	/// after construction, this is the "total required".
	number_of_registers: usize,
}

#[derive(Debug)]
pub struct MatchedRule<'input> {
	pub rule: usize,
	pub lexeme: &'input str,
}

#[derive(Debug)]
struct DfaState {
	kernel: Kernel,
	transitions: IntervalTree<u32, Transition>,
	final_rule: Option<usize>,
	final_operations: Vec<RegisterOp>,
	tag_for_register: BTreeMap<usize, Tag>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Kernel(Vec<Configuration>);

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Configuration {
	nfa_state: NfaIdx,
	register_for_tag: Vec<usize>,
	tag_path_in_closure: Vec<(Tag, SymbolicPosition)>,
}

#[derive(Debug, Clone)]
struct Transition {
	destination: usize,
	operations: Vec<RegisterOp>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct DfaTag(usize);

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct RegisterOp {
	target: usize,
	action: RegisterAction,
}

/// "Right hand side" of a register operation:
/// - "**set**" register to `Current` or `Nil`.
///   Used for single value tags; not relevant for us.
/// - "**copy**" from register `source`.
/// - copy from register `source` and "**append**" `history`.
///   Used for multi-value tags;
///   since multi-value tags are more general than single value tags,
///   we treat every tag/capture as multi-valued.
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
pub struct PrefixTree {
	nodes: Vec<PrefixTreeNode>,
}

#[derive(Debug)]
struct PrefixTreeNode {
	predecessor: usize,
	offset: usize,
}

impl Dfa {
	pub fn simulate(&self, input: &str) -> bool {
		self.simulate_with_captures(input, |capture, lexeme, _, _| {
			let capture: &AutomataCapture = self.capture_info(capture);
			debug!("- captured {capture:?}, {lexeme}");
		})
		.is_ok()
	}

	pub fn simulate_with_captures<'input, F>(
		&self,
		input: &'input str,
		mut on_capture: F,
	) -> Result<MatchedRule<'input>, usize>
	where
		F: FnMut(usize, &'input str, usize, usize),
	{
		let mut current_state: usize = 0;

		// Vector of prefix tree node indices.
		let mut registers: Vec<usize> = vec![0; self.number_of_registers];
		let mut prefix_tree: PrefixTree = PrefixTree::new();

		let mut maybe_backup: Option<((usize, usize), usize, usize, Vec<usize>)> = None;

		let mut last_successful_pos: usize = 0;

		for (i, ch) in input.char_indices() {
			debug!("=== step {i} (state {current_state}), ch {ch:?} ({})", u32::from(ch));
			/*
			let mut seen: BTreeSet<usize> = BTreeSet::new();
			for config in self.states[current_state].kernel.0.iter() {
				for (i, &r) in config.register_for_tag.iter().enumerate() {
					if seen.insert(r) {
						debug!("- tag ({:?}) -> {r} -> {:?}", self.tags[i], registers[r]);
					}
				}
			}
			*/
			if let Some(transition) = self.states[current_state].transitions.lookup(u32::from(ch)) {
				// TODO check/explain:
				// tags are executed on outgoing transitions, so they get the "previous" char boundary
				// the total lexeme includes the "currently consumed" char
				last_successful_pos = i;
				let consumed: usize = i + ch.len_utf8();
				// Apply transition operations before updating position; lookahead 1 in TDFA(1).
				self.apply_operations(
					&mut registers,
					&mut prefix_tree,
					i,
					&transition.operations,
					&self.states[current_state].tag_for_register,
				);
				// TODO seems to work even if we move the following line above (but not the position)
				// but based on transition_operations, the operations should be for the new state?
				current_state = transition.destination;
				if let Some(rule) = self.states[current_state].final_rule {
					maybe_backup = Some(((i, consumed), current_state, rule, registers.clone()));
				}
			} else {
				break;
			}
		}

		let ((i, consumed), state, rule, mut registers): ((usize, usize), usize, usize, Vec<usize>) =
			maybe_backup.ok_or(last_successful_pos)?;

		self.apply_operations(
			&mut registers,
			&mut prefix_tree,
			consumed,
			&self.states[state].final_operations,
			&self.states[state].tag_for_register,
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
			let mut node: usize = registers[self.tags.len() + i];
			while node != 0 {
				offsets.push(prefix_tree.nodes[node].offset);
				node = prefix_tree.nodes[node].predecessor;
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

		Ok(MatchedRule {
			rule,
			lexeme: &input[..consumed],
		})
	}

	fn apply_operations(
		&self,
		registers: &mut [usize],
		prefix_tree: &mut PrefixTree,
		pos: usize,
		ops: &[RegisterOp],
		tag_for_register: &BTreeMap<usize, Tag>,
	) {
		debug!("tags: {tag_for_register:#?}");
		for o in ops.iter() {
			match &o.action {
				RegisterAction::CopyFrom { source } => {
					debug!(
						"copying {} ({:?}) <- {} ({:?})",
						o.target,
						tag_for_register.get(&o.target),
						*source,
						tag_for_register.get(source)
					);
					registers[o.target] = registers[*source];
				},
				RegisterAction::Append { source, history } => {
					debug!(
						"appending {} ({:?}) <- {} ({:?}) with history {:?}",
						o.target,
						tag_for_register.get(&o.target),
						*source,
						tag_for_register.get(source),
						history
					);
					registers[o.target] = registers[*source];
					for &symbolic in history.iter() {
						if symbolic == SymbolicPosition::Current {
							let node: usize = prefix_tree.add_node(registers[o.target], pos);
							registers[o.target] = node;
						}
					}
				},
			}
		}
	}
}

impl Dfa {
	pub fn capture_info(&self, i: usize) -> &AutomataCapture {
		let (Tag::StartCapture(capture) | Tag::StopCapture(capture)) = &self.tags[i];
		capture
	}
}

impl Dfa {
	/// - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
	/// - <https://arxiv.org/abs/2206.01398>
	///
	/// Algorithm 3.
	pub fn determinization(nfa: &Nfa) -> Self {
		let mut tag_ids: BTreeMap<Tag, DfaTag> = BTreeMap::new();
		for (i, tag) in nfa.tags().iter().enumerate() {
			tag_ids.insert(tag.clone(), DfaTag(i));
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

		// TODO explain why instead of stack
		let mut i: usize = 0;
		while i < dfa.states.len() {
			// TODO explain why need clone, borrowed in loop
			let kernel: Kernel = dfa.states[i].kernel.clone();
			debug!(
				"== On DFA state {i} {:?}",
				kernel.0.iter().map(|config| config.nfa_state).collect::<BTreeSet<_>>()
			);

			let mut register_action_tag: BTreeMap<(Tag, RegisterAction), usize> = BTreeMap::new();
			for &interval in kernel.nontrivial_transitions(nfa).iter() {
				let next: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> =
					Self::step_on_interval(nfa, kernel.0.clone(), interval);
				let next: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Self::epsilon_closure(nfa, next);

				debug!("dfa {i} on {interval:?} transition ops...");
				let (next, mut operations): (Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>, Vec<RegisterOp>) =
					dfa.transition_operations(next, &mut register_action_tag);

				let next: usize = dfa.add_state(next, &mut operations);
				debug!("stepping from {i} to {next} on {interval:?} has ops {operations:#?}");
				dfa.states[i].transitions.insert(
					interval,
					Transition {
						destination: next,
						operations,
					},
					|existing, new| {
						panic!(
							"State {i} transition on {interval:?} had existing target {:?}, trying to insert {:?}!",
							existing, new
						);
					},
				);
			}

			i += 1;
		}

		dfa
	}

	fn add_state(
		&mut self,
		configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
		ops: &mut Vec<RegisterOp>,
	) -> usize {
		let mut final_rule: Option<usize> = None;
		let mut final_operations: Vec<RegisterOp> = Vec::new();
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
				assert!(old.is_none() || (old == Some(self.tags[i].clone())));
			}
		}
		let idx: usize = self.states.len();
		debug!("- adding state {idx} ({ops:?}): {:#?}", &kernel.0);
		self.states.push(DfaState {
			kernel: kernel.clone(),
			transitions: IntervalTree::new(),
			final_rule,
			final_operations,
			tag_for_register,
		});
		self.kernels.insert(kernel, idx);
		idx
	}

	fn try_find_bijection(
		&self,
		lhs: &[Configuration],
		rhs: &[Configuration],
		mut ops: Vec<RegisterOp>,
	) -> Option<Vec<RegisterOp>> {
		// Do they contain the same NFA states with the same lookahead tags?
		for x in lhs.iter() {
			rhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tag_path_in_closure == y.tag_path_in_closure))?;
		}
		for x in rhs.iter() {
			lhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tag_path_in_closure == y.tag_path_in_closure))?;
		}

		let mut m1: BTreeMap<usize, usize> = BTreeMap::new();
		let mut m2: BTreeMap<usize, usize> = BTreeMap::new();

		for x in lhs.iter() {
			for y in rhs.iter() {
				if x.nfa_state != y.nfa_state {
					continue;
				}
				for tag in self.tag_ids.keys() {
					let i: usize = x.register_for_tag[self[tag]];
					let j: usize = y.register_for_tag[self[tag]];
					match (m1.entry(i), m2.entry(j)) {
						(Entry::Vacant(e1), Entry::Vacant(e2)) => {
							e1.insert(j);
							e2.insert(i);
						},
						(Entry::Occupied(e1), Entry::Occupied(e2)) => {
							if (*e1.get() != j) || (*e2.get() != i) {
								return None;
							}
						},
						_ => {
							return None;
						},
					}
				}
			}
		}

		for o in ops.iter_mut() {
			o.target = m1.remove(&o.target).unwrap();
		}
		let mut copies: Vec<RegisterOp> = Vec::new();
		for (&j, &i) in m1.iter() {
			copies.push(RegisterOp {
				target: i,
				action: RegisterAction::CopyFrom { source: j },
			});
		}

		ops.extend_from_slice(&copies[..]);

		Self::topological_sort(ops)
	}

	fn topological_sort(mut ops: Vec<RegisterOp>) -> Option<Vec<RegisterOp>> {
		let mut in_degree_register: BTreeMap<usize, usize> = BTreeMap::new();
		for o in ops.iter() {
			match &o.action {
				RegisterAction::CopyFrom { source } | RegisterAction::Append { source, .. } => {
					in_degree_register.insert(*source, 0);
					in_degree_register.insert(o.target, 0);
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

		let mut new_ops: Vec<RegisterOp> = Vec::new();
		let mut nontrivial_cycle: bool = false;

		let mut tmp_ops: Vec<RegisterOp> = Vec::new();
		while !ops.is_empty() {
			let mut anything_added: bool = false;
			for o in ops.drain(..) {
				if in_degree_register[&o.target] == 0 {
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
							if *source != o.target {
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
		nfa: &Nfa,
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

		assert_eq!(
			next_states.len(),
			next_states
				.iter()
				.map(|(config, _)| config.nfa_state)
				.collect::<BTreeSet<_>>()
				.len()
		);

		next_states
	}

	fn epsilon_closure(
		nfa: &Nfa,
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
						// println!(
						// 	"pushing positive {tag:?} in state {:?} to {:?}",
						// 	config.nfa_state, transition.target
						// );
						new_config
							.tag_path_in_closure
							.push((tag.clone(), SymbolicPosition::Current));
					},
					SpontaneousTransitionKind::Negative(tag) => {
						// println!(
						// 	"pushing negative {tag:?} in state {:?} to {:?}",
						// 	config.nfa_state, transition.target
						// );
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

		assert_eq!(
			closure.len(),
			closure
				.iter()
				.map(|(config, _)| config.nfa_state)
				.collect::<BTreeSet<_>>()
				.len()
		);

		closure
	}

	fn transition_operations(
		&mut self,
		configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>,
		register_action_tag: &mut BTreeMap<(Tag, RegisterAction), usize>,
	) -> (Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)>, Vec<RegisterOp>) {
		let mut new_configurations: Vec<(Configuration, Vec<(Tag, SymbolicPosition)>)> = Vec::new();
		let mut ops: BTreeSet<RegisterOp> = BTreeSet::new();

		for (mut config, inherited) in configurations.into_iter() {
			for tag in self.tag_ids.keys() {
				let history: Vec<SymbolicPosition> = Self::history(&inherited, tag);
				if history.is_empty() {
					continue;
				}
				let action: RegisterAction = self.operation_rhs(&config.register_for_tag, history, tag);
				let target: usize = *register_action_tag
					.entry((tag.clone(), action.clone()))
					.or_insert_with_key(|(_, action)| {
						let r: usize = self.number_of_registers;
						self.number_of_registers += 1;
						r
					});
				ops.insert(RegisterOp { target, action });
				config.register_for_tag[self[tag]] = target;
			}
			new_configurations.push((config, inherited));
		}

		(new_configurations, ops.into_iter().collect::<Vec<_>>())
	}

	fn final_operations(&self, registers: &[usize], history: &[(Tag, SymbolicPosition)]) -> Vec<RegisterOp> {
		let mut ops: Vec<RegisterOp> = Vec::new();

		for t in self.tags.iter() {
			let history: Vec<SymbolicPosition> = Self::history(history, t);
			let action: RegisterAction = if history.is_empty() {
				RegisterAction::CopyFrom {
					source: registers[self[t]],
				}
			} else {
				self.operation_rhs(registers, history, t)
			};
			ops.push(RegisterOp {
				target: self.tags.len() + self[t],
				action,
			});
		}

		ops
	}

	fn operation_rhs(&self, registers: &[usize], history: Vec<SymbolicPosition>, tag: &Tag) -> RegisterAction {
		RegisterAction::Append {
			source: registers[self[tag]],
			history,
		}
	}

	fn history(history: &[(Tag, SymbolicPosition)], tag1: &Tag) -> Vec<SymbolicPosition> {
		let mut sequence: Vec<SymbolicPosition> = Vec::new();
		for (tag2, sign) in history.iter() {
			if tag2 != tag1 {
				continue;
			}
			match sign {
				SymbolicPosition::Current => {
					sequence.push(SymbolicPosition::Current);
				},
				SymbolicPosition::Nil => {
					sequence.push(SymbolicPosition::Nil);
				},
			}
		}
		sequence
	}
}

// TODO this shows up publicly...
impl std::ops::Index<&Tag> for Dfa {
	type Output = usize;

	fn index(&self, tag: &Tag) -> &Self::Output {
		&self.tag_ids[tag].0
	}
}

impl std::ops::IndexMut<&Tag> for Dfa {
	fn index_mut(&mut self, tag: &Tag) -> &mut Self::Output {
		&mut self.tag_ids.get_mut(tag).unwrap().0
	}
}

impl Kernel {
	fn nontrivial_transitions(&self, nfa: &Nfa) -> Vec<Interval<u32>> {
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

impl PrefixTree {
	fn new() -> Self {
		Self {
			nodes: vec![PrefixTreeNode {
				predecessor: 0,
				offset: 0,
			}],
		}
	}

	fn add_node(&mut self, predecessor: usize, offset: usize) -> usize {
		assert!(predecessor < self.nodes.len());
		let len: usize = self.nodes.len();
		self.nodes.push(PrefixTreeNode { predecessor, offset });
		len
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
			let dfa: Dfa = schema.build_dfa();
			dbg!(&dfa);
			let b: bool = dfa.simulate("012a2b2c12z12zxyzxyzxyzworld");
			assert!(b);
		}
	}
}
