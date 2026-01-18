use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::nfa::Nfa;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::SpontaneousTransitionKind;
use crate::nfa::Tag;
use crate::nfa::Variable;

#[derive(Debug)]
pub struct Dfa<'schema> {
	states: Vec<DfaState<'schema>>,
	kernels: BTreeMap<Kernel<'schema>, usize>,
	/// Bijection between `tags` and `{ 0..tags.len() }`.
	tags: Vec<Tag<'schema>>,
	tag_ids: BTreeMap<Tag<'schema>, DfaTag>,
	/// During construction, this is the "current" count;
	/// after construction, this is the "total required".
	number_of_registers: usize,
}

#[derive(Debug, Clone)]
pub struct GotRule<'input> {
	pub rule: usize,
	pub lexeme: &'input str,
}

#[derive(Debug)]
struct DfaState<'schema> {
	kernel: Kernel<'schema>,
	transitions: IntervalTree<u32, Transition>,
	is_final: bool,
	final_operations: Vec<RegisterOp>,
	tag_for_register: BTreeMap<usize, Tag<'schema>>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Kernel<'schema>(BTreeSet<Configuration<'schema>>);

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Configuration<'schema> {
	nfa_state: NfaIdx,
	register_for_tag: Vec<usize>,
	tag_path_in_closure: Vec<(Tag<'schema>, SymbolicPosition)>,
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

impl<'schema> Dfa<'schema> {
	pub fn simulate(&self, input: &str) -> bool {
		self.simulate_with_captures(input, |name, lexeme| {
			println!("- captured {name}, {lexeme}");
		})
		.is_ok()
	}

	pub fn simulate_with_captures<'input, F>(
		&self,
		input: &'input str,
		mut on_capture: F,
	) -> Result<GotRule<'input>, usize>
	where
		F: FnMut(&'schema str, &'input str),
	{
		let mut current_state: usize = 0;

		// Vector of prefix tree node indices.
		let mut registers: Vec<usize> = vec![0; self.number_of_registers];
		let mut prefix_tree: PrefixTree = PrefixTree::new();

		let mut maybe_backup: Option<(usize, usize, Vec<usize>)> = None;

		let mut consumed: usize = 0;

		for (i, ch) in input.char_indices() {
			println!("=== step {i} (state {current_state}), ch {ch:?} ({})", u32::from(ch));
			/*
			let mut seen: BTreeSet<usize> = BTreeSet::new();
			for config in self.states[current_state].kernel.0.iter() {
				for (i, &r) in config.register_for_tag.iter().enumerate() {
					if seen.insert(r) {
						println!("- tag ({:?}) -> {r} -> {:?}", self.tags[i], registers[r]);
					}
				}
			}
			*/
			if let Some(transition) = self.states[current_state].transitions.lookup(u32::from(ch)) {
				consumed = i;
				current_state = transition.destination;
				self.apply_operations(
					&mut registers,
					&mut prefix_tree,
					i,
					&transition.operations,
					&self.states[current_state].tag_for_register,
				);
				if self.states[current_state].is_final {
					maybe_backup = Some((consumed, current_state, registers.clone()));
				}
			} else {
				break;
			}
		}

		let (consumed, state, mut registers): (usize, usize, Vec<usize>) = maybe_backup.ok_or(consumed)?;

		self.apply_operations(
			&mut registers,
			&mut prefix_tree,
			consumed,
			&self.states[state].final_operations,
			&self.states[state].tag_for_register,
		);

		let mut variables: BTreeMap<Variable<'_>, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
		for i in 0..self.tags.len() {
			let (Tag::StartCapture(variable) | Tag::EndCapture(variable)) = self.tags[i];
			let (starts, ends): &mut (Vec<usize>, Vec<usize>) =
				variables.entry(variable).or_insert((Vec::new(), Vec::new()));
			let offsets: &mut Vec<usize> = match self.tags[i] {
				Tag::StartCapture(_) => starts,
				Tag::EndCapture(_) => ends,
			};
			let mut node: usize = registers[self.tags.len() + i];
			while node != 0 {
				offsets.push(prefix_tree.nodes[node].offset);
				node = prefix_tree.nodes[node].predecessor;
			}
			offsets.reverse();
		}

		let mut maybe_rule: Option<usize> = None;

		for (var, (starts, ends)) in variables.into_iter() {
			assert_eq!(starts.len(), ends.len());
			if starts.is_empty() {
				continue;
			}
			if let Some(rule) = maybe_rule {
				assert_eq!(var.rule, rule);
			} else {
				maybe_rule = Some(var.rule);
			}
			for (&i, &j) in std::iter::zip(starts.iter(), ends.iter()) {
				on_capture(var.name, &input[i..=j]);
			}
		}

		Ok(GotRule {
			rule: maybe_rule.unwrap(),
			lexeme: &input[..=consumed],
		})
	}

	fn apply_operations(
		&self,
		registers: &mut [usize],
		prefix_tree: &mut PrefixTree,
		pos: usize,
		ops: &[RegisterOp],
		tag_for_register: &BTreeMap<usize, Tag<'schema>>,
	) {
		println!("tags: {tag_for_register:#?}");
		for o in ops.iter() {
			match &o.action {
				RegisterAction::CopyFrom { source } => {
					println!(
						"copying {} ({:?}) <- {} ({:?})",
						o.target,
						tag_for_register.get(&o.target),
						*source,
						tag_for_register.get(source)
					);
					registers[o.target] = registers[*source];
				},
				RegisterAction::Append { source, history } => {
					println!(
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

impl<'schema> Dfa<'schema> {
	/// - <https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf>
	/// - <https://arxiv.org/abs/2206.01398>
	///
	/// Algorithm 3.
	pub fn determinization(nfa: &Nfa<'schema>) -> Self {
		let mut tag_ids: BTreeMap<Tag<'_>, DfaTag> = BTreeMap::new();
		for (i, &tag) in nfa.tags().iter().enumerate() {
			tag_ids.insert(tag, DfaTag(i));
		}

		let mut dfa: Self = Self {
			states: Vec::new(),
			kernels: BTreeMap::new(),
			tags: nfa.tags().to_owned(),
			tag_ids,
			number_of_registers: 2 * nfa.tags().len(),
		};

		let initial: (Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>) = (
			Configuration {
				nfa_state: nfa.start(),
				register_for_tag: (0..dfa.tag_ids.len()).collect::<Vec<_>>(),
				tag_path_in_closure: Vec::new(),
			},
			Vec::new(),
		);

		let initial: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>)> =
			Self::epsilon_closure(nfa, BTreeSet::from([initial]));

		dfa.add_state(initial, &mut Vec::new());

		// TODO explain why instead of stack
		let mut i: usize = 0;
		while i < dfa.states.len() {
			// TODO explain why need clone, borrowed in loop
			let kernel: Kernel<'_> = dfa.states[i].kernel.clone();
			println!(
				"== On DFA state {i} {:?}",
				kernel.0.iter().map(|config| config.nfa_state).collect::<BTreeSet<_>>()
			);

			let mut register_action_tag: BTreeMap<(Tag<'_>, RegisterAction), usize> = BTreeMap::new();
			for interval in kernel.nontrivial_transitions(nfa).iter() {
				let next: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>)> =
					Self::step_on_interval(nfa, &kernel.0, *interval);
				let next: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>)> =
					Self::epsilon_closure(nfa, next);

				println!("dfa {i} on {interval:?} transition ops...");
				let (next, mut operations): (
					BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>)>,
					Vec<RegisterOp>,
				) = dfa.transition_operations(next, &mut register_action_tag);

				let next: usize = dfa.add_state(next, &mut operations);
				println!("stepping from {i} to {next} on {interval:?} has ops {operations:#?}");
				dfa.states[i].transitions.insert(
					*interval,
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
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, SymbolicPosition)>)>,
		ops: &mut Vec<RegisterOp>,
	) -> usize {
		let mut is_final: bool = false;
		let mut final_operations: Vec<RegisterOp> = Vec::new();
		let mut tag_for_register: BTreeMap<usize, Tag<'_>> = BTreeMap::new();
		let configurations: BTreeSet<Configuration<'_>> = configurations
			.into_iter()
			.map(|(config, _)| {
				if config.nfa_state.is_end() {
					assert!(!is_final);
					is_final = true;
					final_operations = self.final_operations(&config.register_for_tag, &config.tag_path_in_closure);
				}
				config
			})
			.collect::<BTreeSet<_>>();
		let kernel: Kernel<'_> = Kernel(configurations);

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
				let old: Option<Tag<'_>> = tag_for_register.insert(r, self.tags[i]);
				assert!(old.is_none() || (old == Some(self.tags[i])));
			}
		}
		let idx: usize = self.states.len();
		println!("- adding state {idx} ({ops:?}): {:#?}", &kernel.0);
		self.states.push(DfaState {
			kernel: kernel.clone(),
			transitions: IntervalTree::new(),
			is_final,
			final_operations,
			tag_for_register,
		});
		self.kernels.insert(kernel, idx);
		idx
	}

	fn try_find_bijection(
		&self,
		lhs: &BTreeSet<Configuration<'schema>>,
		rhs: &BTreeSet<Configuration<'schema>>,
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
				for &tag in self.tag_ids.keys() {
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
		nfa: &Nfa<'schema>,
		configurations: &BTreeSet<Configuration<'schema>>,
		interval: Interval<u32>,
	) -> BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, SymbolicPosition)>)> {
		let mut next_states: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, SymbolicPosition)>)> =
			BTreeSet::new();

		for config in configurations.iter() {
			let nfa_state: &NfaState<'_> = &nfa[config.nfa_state];
			if let Some(targets) = nfa_state.transitions().lookup(interval.start()) {
				assert!(
					nfa_state
						.transitions()
						.lookup_interval(interval.start())
						.unwrap()
						.contains(&interval)
				);
				for &next in targets.iter() {
					next_states.insert((
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
		nfa: &Nfa<'schema>,
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, SymbolicPosition)>)>,
	) -> BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, SymbolicPosition)>)> {
		let mut closure: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>)> = BTreeSet::new();
		let mut nfa_states_in_closure: BTreeSet<NfaIdx> = BTreeSet::new();

		let mut i: usize = 0;
		let mut stack: Vec<(Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>)> =
			configurations.into_iter().collect::<Vec<_>>();

		while i < stack.len() {
			// TODO explain why cloned
			let (config, inherited): (Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>) = stack[i].clone();

			closure.insert((config.clone(), inherited.clone()));
			nfa_states_in_closure.insert(config.nfa_state);

			for transition in nfa[config.nfa_state].spontaneous().iter() {
				if nfa_states_in_closure.contains(&transition.target) {
					// TODO shouldn't happen..?
					continue;
				}

				let mut new_config: Configuration<'_> = Configuration {
					nfa_state: transition.target,
					..config.clone()
				};

				match transition.kind {
					SpontaneousTransitionKind::Positive(tag) => {
						new_config.tag_path_in_closure.push((tag, SymbolicPosition::Current));
					},
					SpontaneousTransitionKind::Negative(tag) => {
						new_config.tag_path_in_closure.push((tag, SymbolicPosition::Nil));
					},
					SpontaneousTransitionKind::Epsilon => (),
				}

				stack.push((new_config, inherited.clone()));
			}

			i += 1;
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
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, SymbolicPosition)>)>,
		register_action_tag: &mut BTreeMap<(Tag<'schema>, RegisterAction), usize>,
	) -> (
		BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, SymbolicPosition)>)>,
		Vec<RegisterOp>,
	) {
		let mut new_configurations: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, SymbolicPosition)>)> = BTreeSet::new();
		let mut ops: BTreeSet<RegisterOp> = BTreeSet::new();

		for (mut config, inherited) in configurations.into_iter() {
			for &tag in self.tag_ids.keys() {
				let history: Vec<SymbolicPosition> = Self::history(&inherited, tag);
				if history.is_empty() {
					continue;
				}
				let action: RegisterAction = self.operation_rhs(&config.register_for_tag, history, tag);
				let target: usize =
					*register_action_tag
						.entry((tag, action.clone()))
						.or_insert_with_key(|(_, action)| {
							let r: usize = self.number_of_registers;
							self.number_of_registers += 1;
							r
						});
				ops.insert(RegisterOp { target, action });
				config.register_for_tag[self[tag]] = target;
			}
			new_configurations.insert((config, inherited));
		}

		(new_configurations, ops.into_iter().collect::<Vec<_>>())
	}

	fn final_operations(&self, registers: &[usize], history: &[(Tag<'schema>, SymbolicPosition)]) -> Vec<RegisterOp> {
		let mut ops: Vec<RegisterOp> = Vec::new();

		for &t in self.tags.iter() {
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

	fn operation_rhs(&self, registers: &[usize], history: Vec<SymbolicPosition>, tag: Tag<'schema>) -> RegisterAction {
		RegisterAction::Append {
			source: registers[self[tag]],
			history,
		}
	}

	fn history(history: &[(Tag<'schema>, SymbolicPosition)], tag1: Tag<'schema>) -> Vec<SymbolicPosition> {
		let mut sequence: Vec<SymbolicPosition> = Vec::new();
		for &(tag2, sign) in history.iter() {
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
impl<'schema> std::ops::Index<Tag<'schema>> for Dfa<'schema> {
	type Output = usize;

	fn index(&self, tag: Tag<'schema>) -> &Self::Output {
		&self.tag_ids[&tag].0
	}
}

impl<'schema> std::ops::IndexMut<Tag<'schema>> for Dfa<'schema> {
	fn index_mut(&mut self, tag: Tag<'schema>) -> &mut Self::Output {
		&mut self.tag_ids.get_mut(&tag).unwrap().0
	}
}

impl<'schema> Kernel<'schema> {
	fn nontrivial_transitions(&self, nfa: &Nfa<'schema>) -> Vec<Interval<u32>> {
		let mut transitions: IntervalTree<u32, ()> = IntervalTree::new();

		for config in self.0.iter() {
			let nfa_state: &NfaState<'_> = &nfa[config.nfa_state];
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
			let nfa: Nfa<'_> = Nfa::for_schema(&schema).unwrap();
			let dfa: Dfa<'_> = Dfa::determinization(&nfa);
			dbg!(&dfa);
			let b: bool = dfa.simulate("012a2b2c12z12zxyzxyzxyzworld");
			assert!(b);
		}
	}
}
