use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::nfa::Nfa;
use crate::nfa::NfaState;
use crate::nfa::SpontaneousTransitionKind;
use crate::nfa::Tag;
use crate::nfa::Variable;

#[derive(Debug)]
pub struct Dfa<'schema> {
	states: Vec<(DfaState<'schema>, IntervalTree<u32, usize>)>,
	state_map: BTreeMap<DfaState<'schema>, usize>,
	/// Bijection between `tags` and `{ 0..tags.len() }`.
	tags: BTreeMap<Tag<'schema>, usize>,
	/// ("Current") number of registers used.
	registers: usize,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct DfaState<'schema> {
	configurations: BTreeSet<Configuration<'schema>>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Configuration<'schema> {
	nfa_state: usize,
	register_for_tag: Vec<usize>,
	tags: Vec<(Tag<'schema>, Sign)>,
}

/*
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum DfaTag {
	Positive(u32),
	Negative(u32),
}

impl std::fmt::Debug for DfaTag {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// TODO explain lifetime
		match self {
			Self::Positive(t) => fmt.write_fmt(format_args!("DfaTag({t})")),
			Self::Negative(t) => fmt.write_fmt(format_args!("DfaTag(-{t})")),
		}
	}
}
*/

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct RegisterOp {
	target: usize,
	action: RegisterAction,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum RegisterAction {
	Set {
		position: SymbolicPosition,
	},
	// "Copy" in the paper.
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

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Sign {
	Positive,
	Negative,
}

struct NfaBuilder<'a, 'schema> {
	nfa: &'a mut Nfa<'schema>,
	current: usize,
	begins: usize,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct TagMatches<'schema> {
	offsets: BTreeMap<Variable<'schema>, (Vec<usize>, Vec<usize>)>,
}

#[derive(Debug)]
pub struct PrefixTree<'schema> {
	nodes: Vec<PrefixTreeNode<'schema>>,
}

#[derive(Debug)]
struct PrefixTreeNode<'schema> {
	predecessor: usize,
	tag: (Tag<'schema>, bool),
}

impl<'schema> Dfa<'schema> {
	pub fn simulate(&self, input: &str) -> bool {
		let mut current_state: usize = 0;

		for (i, ch) in input.char_indices() {
			println!("=== step {i} (state {current_state}), ch {ch:?} ({})", u32::from(ch));
			if let Some(&next) = self.states[current_state].1.lookup(u32::from(ch)) {
				current_state = next;
			} else {
				println!("nope!");
				return false;
			}
		}

		println!("ended at {current_state}");
		for config in self.states[current_state].0.configurations.iter() {
			if config.nfa_state == 1 {
				return true;
			}
		}

		println!("nope2");
		false
	}

	// https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf
	// - Algorithm 3
	pub fn determinization(nfa: &Nfa<'schema>) -> Self {
		let tags: BTreeMap<Tag<'_>, usize> = nfa
			.tags
			.iter()
			.scan(0, |i, tag| Some((*tag, std::mem::replace(i, *i + 1))))
			.collect::<_>();

		let mut dfa: Self = Self {
			states: Vec::new(),
			state_map: BTreeMap::new(),
			tags,
			registers: nfa.tags.len() * 2,
		};

		let registers_initial: BTreeMap<Tag<'_>, usize> = dfa.tags.clone();

		let initial: (Configuration<'_>, Vec<(Tag<'_>, Sign)>) = (
			Configuration {
				nfa_state: 0,
				register_for_tag: (0..dfa.tags.len()).collect::<_>(),
				tags: Vec::new(),
			},
			Vec::new(),
		);

		let initial: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> =
			Self::epsilon_closure(nfa, BTreeSet::from([initial]));

		println!("epsilon closure of initial state is: {initial:#?}");
		println!("===== DONE EPSILON CLOSURE 0");

		dfa.add_state(initial, &mut Vec::new());

		// TODO explain why instead of stack
		let mut i: usize = 0;
		while i < dfa.states.len() {
			// TODO explain why need clone, borrowed in loop
			let dfa_state: DfaState<'_> = dfa.states[i].0.clone();
			println!("== On DFA state {i}");

			let mut register_action_tag: BTreeMap<(Tag<'_>, RegisterAction), usize> = BTreeMap::new();

			for interval in dfa_state.nontrivial_transitions(nfa).iter() {
				let next: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> =
					Self::step_on_interval(nfa, &dfa_state.configurations, *interval);
				let next: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> = Self::epsilon_closure(nfa, next);

				let (next, mut operations): (BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)>, Vec<RegisterOp>) =
					dfa.transition_operations(next, &mut register_action_tag);

				let next: usize = dfa.add_state(next, &mut operations);
				dfa.states[i].1.insert(*interval, next, |existing, new| {
					panic!(
						"State {i} transition on {interval:?} had existing target {existing}, trying to insert {new}!"
					);
				});
			}

			i += 1;
		}

		dfa
	}

	fn add_state(
		&mut self,
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)>,
		ops: &mut Vec<RegisterOp>,
	) -> usize {
		let dfa_state: DfaState = DfaState {
			configurations: configurations.into_iter().map(|(config, _)| config).collect::<_>(),
		};
		println!(
			"ADD STATE looking for {:#?}",
			dfa_state // .configurations
			          // .iter()
			          // .map(|config| config.nfa_state)
			          // .collect::<BTreeSet<_>>()
		);

		if let Some(&idx) = self.state_map.get(&dfa_state) {
			println!("- found {idx}");
			return idx;
		}

		for (idx, (state, _)) in self.states.iter().enumerate() {
			if let Some(new_ops) =
				self.try_find_bijection(&dfa_state.configurations, &state.configurations, ops.clone())
			{
				*ops = new_ops;
				return idx;
			}
		}

		let idx: usize = self.states.len();
		println!("- adding state {idx}: {:#?}", &dfa_state.configurations);
		self.states.push((dfa_state.clone(), IntervalTree::new()));
		self.state_map.insert(dfa_state, idx);
		// TODO final states
		idx
	}

	fn try_find_bijection(
		&self,
		lhs: &BTreeSet<Configuration<'schema>>,
		rhs: &BTreeSet<Configuration<'schema>>,
		mut ops: Vec<RegisterOp>,
	) -> Option<Vec<RegisterOp>> {
		// println!("here {lhs:?}, {rhs:?}");
		println!("trying bijection");
		println!(
			"- {:?}",
			lhs.iter().map(|config| config.nfa_state).collect::<BTreeSet<_>>()
		);
		println!(
			"- {:?}",
			rhs.iter().map(|config| config.nfa_state).collect::<BTreeSet<_>>()
		);
		// Do they contain the same NFA states with the same lookahead tags?
		for x in lhs.iter() {
			rhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tags == y.tags))?;
		}
		println!("- left <= right");
		for x in rhs.iter() {
			lhs.iter()
				.find(|y| (x.nfa_state == y.nfa_state) && (x.tags == y.tags))?;
		}
		println!("- right <= left");

		let mut m1: BTreeMap<usize, usize> = BTreeMap::new();
		let mut m2: BTreeMap<usize, usize> = BTreeMap::new();

		for x in lhs.iter() {
			for y in rhs.iter() {
				if x.nfa_state != y.nfa_state {
					continue;
				}
				println!("== PAIR {}", x.nfa_state);
				println!("- {:?}", &x.register_for_tag);
				println!("- {:?}", &y.register_for_tag);
				for &tag in self.tags.keys() {
					let i: usize = x.register_for_tag[self[tag]];
					let j: usize = y.register_for_tag[self[tag]];
					match (m1.entry(i), m2.entry(j)) {
						(Entry::Vacant(e1), Entry::Vacant(e2)) => {
							println!("inserting {i} <-> {j}");
							e1.insert(j);
							e2.insert(i);
						},
						(Entry::Occupied(e1), Entry::Occupied(e2)) => {
							if (*e1.get() != j) || (*e2.get() != i) {
								println!("occupied entry different: {} {}", e1.get(), e2.get());
								return None;
							}
						},
						_ => {
							println!("unluck!");
							return None;
						}, // (Entry::Vacant(e1), Entry::Occupied(e2)) => {
						   // 	if i == *e2.get() {
						   // 		e1.insert(j);
						   // 	} else {
						   // 		println!("none1");
						   // 		return None;
						   // 	}
						   // },
						   // (Entry::Occupied(e1), Entry::Vacant(e2)) => {
						   // 	if j == *e1.get() {
						   // 		e2.insert(i);
						   // 	} else {
						   // 		println!("none2");
						   // 		return None;
						   // 	}
						   // },
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
				_ => (),
			}
		}
		for o in ops.iter() {
			match &o.action {
				RegisterAction::CopyFrom { source } | RegisterAction::Append { source, .. } => {
					*in_degree_register.get_mut(source).unwrap() += 1;
				},
				_ => (),
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
						_ => (),
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
						_ => (),
					}
				}
				new_ops.extend(ops.drain(..));
			}
		}

		println!("found match: {nontrivial_cycle}");
		if nontrivial_cycle { None } else { Some(new_ops) }
	}

	fn step_on_interval(
		nfa: &Nfa<'schema>,
		configurations: &BTreeSet<Configuration<'schema>>,
		interval: Interval<u32>,
	) -> BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)> {
		let mut next_states: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)> = BTreeSet::new();

		for config in configurations.iter() {
			let nfa_state: &NfaState<'_> = &nfa.states[config.nfa_state];
			if let Some(targets) = nfa_state.transitions.lookup(interval.start()) {
				for &next in targets.iter() {
					next_states.insert((
						Configuration {
							nfa_state: next,
							register_for_tag: config.register_for_tag.clone(),
							tags: Vec::new(),
						},
						config.tags.clone(),
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
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)>,
	) -> BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)> {
		let mut closure: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> = BTreeSet::new();
		let mut nfa_states_in_closure: BTreeSet<usize> = BTreeSet::new();

		let mut i: usize = 0;
		let mut stack: Vec<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> = configurations.into_iter().collect::<_>();

		while i < stack.len() {
			// TODO why cloned
			let (config, inherited): (Configuration<'_>, Vec<(Tag<'_>, Sign)>) = stack[i].clone();

			closure.insert((config.clone(), inherited.clone()));
			nfa_states_in_closure.insert(config.nfa_state);

			for transition in nfa.states[config.nfa_state].spontaneous.iter() {
				if nfa_states_in_closure.contains(&transition.target) {
					continue;
				}

				let mut new_config: Configuration<'_> = Configuration {
					nfa_state: transition.target,
					..config.clone()
				};

				match transition.kind {
					SpontaneousTransitionKind::Positive(tag) => {
						new_config.tags.push((tag, Sign::Positive));
					},
					SpontaneousTransitionKind::Negative(tag) => {
						new_config.tags.push((tag, Sign::Negative));
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
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)>,
		register_action_tag: &mut BTreeMap<(Tag<'schema>, RegisterAction), usize>,
	) -> (
		BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)>,
		Vec<RegisterOp>,
	) {
		let mut new_configurations: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> = BTreeSet::new();
		let mut ops: Vec<RegisterOp> = Vec::new();

		for (mut config, inherited) in configurations.into_iter() {
			for &tag in self.tags.keys() {
				let history: Vec<SymbolicPosition> = Self::history(&inherited, tag);
				if history.is_empty() {
					continue;
				}
				let action: RegisterAction = self.operation_rhs(&config.register_for_tag, history, tag);
				println!("looking for entry {tag:?}, {action:?}...");
				let r: usize = *register_action_tag
					.entry((tag, action))
					.or_insert_with_key(|(_, action)| {
						let r: usize = self.registers;
						println!("- inserting new register {r}");
						self.registers += 1;
						ops.push(RegisterOp {
							target: r,
							action: action.clone(),
						});
						r
					});
				config.register_for_tag[self[tag]] = r;
			}
			new_configurations.insert((config, inherited));
		}

		(new_configurations, ops)
	}

	fn operation_rhs(&self, registers: &[usize], history: Vec<SymbolicPosition>, tag: Tag<'schema>) -> RegisterAction {
		RegisterAction::Append {
			source: registers[self[tag]],
			history,
		}
	}

	// TODO replace Sign with SymbolicPosition (or vice versa)
	fn history(history: &[(Tag<'schema>, Sign)], tag1: Tag<'schema>) -> Vec<SymbolicPosition> {
		let mut sequence: Vec<SymbolicPosition> = Vec::new();
		for &(tag2, sign) in history.iter() {
			if tag2 != tag1 {
				continue;
			}
			match sign {
				Sign::Positive => {
					sequence.push(SymbolicPosition::Current);
				},
				Sign::Negative => {
					sequence.push(SymbolicPosition::Nil);
				},
			}
		}
		sequence
	}
}

impl<'schema> DfaState<'schema> {
	fn nontrivial_transitions(&self, nfa: &Nfa<'schema>) -> Vec<Interval<u32>> {
		let mut transitions: IntervalTree<u32, ()> = IntervalTree::new();

		for config in self.configurations.iter() {
			let nfa_state: &NfaState<'_> = &nfa.states[config.nfa_state];
			for (interval, _) in nfa_state.transitions.iter() {
				transitions.insert(*interval, (), |(), ()| ());
			}
		}

		transitions.iter().map(|(interval, _)| *interval).collect::<_>()
	}
}

impl<'schema> std::ops::Index<Tag<'schema>> for Dfa<'schema> {
	type Output = usize;

	fn index(&self, tag: Tag<'schema>) -> &Self::Output {
		&self.tags[&tag]
	}
}

impl<'schema> std::ops::IndexMut<Tag<'schema>> for Dfa<'schema> {
	fn index_mut(&mut self, tag: Tag<'schema>) -> &mut Self::Output {
		self.tags.get_mut(&tag).unwrap()
	}
}
