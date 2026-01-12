use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZero;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::log_event::LogComponent;
use crate::regex::Regex;
use crate::schema::Schema;

#[derive(Debug)]
pub struct Nfa<'schema> {
	states: Vec<NfaState<'schema>>,
	tags: Vec<Tag<'schema>>,
}

#[derive(Debug)]
pub struct NfaError {
	hi: u32,
}

#[derive(Debug)]
pub struct Dfa<'schema> {
	states: Vec<(DfaState<'schema>, IntervalTree<u32, usize>)>,
	state_map: BTreeMap<DfaState<'schema>, usize>,
	// Bijection between `tags` and `{ 0..tags.len() }`.
	tags: BTreeMap<Tag<'schema>, usize>,
	// ("Current") number of registers used.
	registers: usize,
}

#[derive(Debug)]
struct NfaState<'schema> {
	pub idx: usize,
	pub transitions: IntervalTree<u32, BTreeSet<usize>>,
	pub spontaneous: Vec<(SpontaneousTransition<'schema>, usize)>,
	pub wildcard: usize,
	pub maybe_final: Option<&'schema str>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct DfaState<'schema> {
	configurations: BTreeSet<Configuration<'schema>>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct Configuration<'schema> {
	nfa_state: usize,
	registers: BTreeMap<Tag<'schema>, usize>,
	tags: Vec<(Tag<'schema>, Sign)>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum SpontaneousTransition<'schema> {
	Epsilon,
	Positive(Tag<'schema>),
	Negative(Tag<'schema>),
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Tag<'schema> {
	StartCapture(Variable<'schema>),
	EndCapture(Variable<'schema>),
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Variable<'schema> {
	rule: usize,
	id: Option<NonZero<u32>>,
	capture: &'schema str,
}

impl std::fmt::Debug for Variable<'_> {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if !fmt.alternate() {
			fmt.debug_tuple("Variable").field(&self.capture).finish()
		} else {
			fmt.debug_struct("Variable")
				.field("rule", &self.rule)
				.field("id", &self.id)
				.field("capture", &self.capture)
				.finish()
		}
	}
}

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
		head: usize,
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

impl<'schema> Nfa<'schema> {
	pub fn for_schema(schema: &'schema Schema) -> Result<Self, NfaError> {
		let mut nfa: Self = Nfa {
			states: Vec::new(),
			tags: Vec::new(),
		};
		let start: usize = nfa.new_state();
		let end: usize = nfa.new_state();

		let mut builder: NfaBuilder = NfaBuilder {
			nfa: &mut nfa,
			current: start,
			begins: 0,
		};
		builder.current = start;
		let tags: BTreeSet<Tag<'_>> = builder.alternate(
			schema.rules.iter().enumerate().map(|(i, rule)| {
				move |builder: &mut NfaBuilder<'_, 'schema>, target| {
					builder.capture(i, None, &*rule.name, &rule.regex, target)
				}
			}),
			end,
		)?;
		nfa.tags = tags.into_iter().collect::<Vec<_>>();
		Ok(nfa)
	}

	// https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf
	// - Algorithm 1
	pub fn simulate<'input>(&self, input: &'input str) -> Option<(LogComponent<'schema, 'input>, usize)> {
		let start: (usize, Vec<TagMatches>) = (
			0,
			vec![TagMatches {
				offsets: BTreeMap::new(),
			}],
		);

		let mut state_set: BTreeSet<(usize, Vec<TagMatches>)> = BTreeSet::new();
		state_set.insert(start);
		state_set = self.epsilon_closure(state_set, 0);

		let mut maybe_last_match: Option<(usize, Vec<TagMatches>)> = None;

		for (i, ch) in input.char_indices() {
			println!("=== step {i}, ch {}", u32::from(ch));
			println!("state set is {state_set:#?}");
			state_set = self.step_on_symbol(&state_set, ch);
			println!("after step is {state_set:#?}");
			state_set = self.epsilon_closure(state_set, i + 1);
			println!("after closure is {state_set:#?}");
			if state_set.is_empty() {
				break;
			}
			for (state, matches) in state_set.iter() {
				// TODO more general
				if *state == 1 {
					println!("=== got match!");
					assert_eq!(matches.len(), 1);
					maybe_last_match = Some((i, matches.clone()));
				}
			}
		}

		for (state, matches) in state_set.iter() {
			// TODO more general
			if *state == 1 {
				println!("=== got match!");
				assert_eq!(matches.len(), 1);
				let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
				for (variable, (starts, ends)) in matches.last().unwrap().offsets.iter() {
					println!("[matched variable {variable:?} {starts:?} {ends:?}");
					for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
						m.push((variable.rule, variable.capture, x, y));
					}
				}
				return Some((
					LogComponent {
						full_text: input,
						matches: m,
					},
					input.len(),
				));
			}
		}

		if let Some((pos, last_match)) = maybe_last_match {
			let mut m: Vec<(usize, &str, usize, usize)> = Vec::new();
			for (variable, (starts, ends)) in last_match.last().unwrap().offsets.iter() {
				println!("[matched variable {variable:?} {starts:?} {ends:?}");
				for (&x, &y) in std::iter::zip(starts.iter(), ends.iter()) {
					m.push((variable.rule, variable.capture, x, y));
				}
			}
			return Some((
				LogComponent {
					full_text: &input[0..pos],
					matches: m,
				},
				pos,
			));
		}

		None
	}

	pub fn to_dfa(&self) -> Dfa<'schema> {
		println!("============================ HERE");
		Dfa::determinization(self)
	}

	fn epsilon_closure(
		&self,
		state_set: BTreeSet<(usize, Vec<TagMatches<'schema>>)>,
		pos: usize,
	) -> BTreeSet<(usize, Vec<TagMatches<'schema>>)> {
		let mut closure: BTreeSet<(usize, Vec<TagMatches>)> = BTreeSet::new();

		let mut stack: Vec<(usize, Vec<TagMatches>)> = state_set.into_iter().collect::<Vec<_>>();

		while let Some((state, matches)) = stack.pop() {
			closure.insert((state, matches.clone()));
			let state: &NfaState<'_> = &self.states[state];

			for (transition, target) in state.spontaneous.iter() {
				let mut matches: Vec<TagMatches> = matches.clone();
				match transition {
					SpontaneousTransition::Positive(tag) => match tag {
						Tag::StartCapture(variable) => {
							matches
								.last_mut()
								.unwrap()
								.offsets
								.entry(*variable)
								.or_insert((Vec::new(), Vec::new()))
								.0
								.push(pos);
						},
						Tag::EndCapture(variable) => {
							matches
								.last_mut()
								.unwrap()
								.offsets
								.entry(*variable)
								.or_insert((Vec::new(), Vec::new()))
								.1
								.push(pos);
						},
					},
					SpontaneousTransition::Negative(_) => (),
					SpontaneousTransition::Epsilon => (),
				}
				stack.push((*target, matches));
			}
		}

		closure
	}

	fn step_on_symbol(
		&self,
		state_set: &BTreeSet<(usize, Vec<TagMatches<'schema>>)>,
		ch: char,
	) -> BTreeSet<(usize, Vec<TagMatches<'schema>>)> {
		let mut next_states: BTreeSet<(usize, Vec<TagMatches<'schema>>)> = BTreeSet::new();

		for (state, matches) in state_set.iter() {
			if let Some(targets) = self.states[*state].transitions.lookup(u32::from(ch)) {
				for &next in targets.iter() {
					next_states.insert((next, matches.clone()));
				}
			}
		}

		next_states
	}

	fn new_state(&mut self) -> usize {
		let n: usize = self.states.len();
		let state: NfaState = NfaState {
			idx: n,
			transitions: IntervalTree::new(),
			spontaneous: Vec::new(),
			wildcard: 0,
			maybe_final: None,
		};
		self.states.push(state);
		n
	}
}

impl<'schema> Dfa<'schema> {
	pub fn simulate(&self, input: &str) -> bool {
		println!("=== DEBUGGING DFA");
		dbg!(self);
		todo!();
	}

	// https://re2c.org/2022_borsotti_trofimovich_a_closer_look_at_tdfa.pdf
	// - Algorithm 3
	fn determinization(nfa: &Nfa<'schema>) -> Self {
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
				registers: registers_initial.clone(),
				tags: Vec::new(),
			},
			Vec::new(),
		);

		let initial: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> =
			Self::epsilon_closure(nfa, BTreeSet::from([initial]));

		println!("epsilon closure of initial state is: {initial:#?}");

		dfa.add_state(initial, Vec::new());

		// TODO explain why instead of stack
		let mut i: usize = 0;
		while i < dfa.states.len() {
			// TODO explain why need clone, borrowed in loop
			let dfa_state: DfaState<'_> = dfa.states[i].0.clone();
			println!("== On DFA state {i}");
			dfa_state.debug(nfa);
			i += 1;

			let mut register_action_tag: BTreeMap<(Tag<'_>, RegisterAction), usize> = BTreeMap::new();

			for (interval, targets) in dfa_state.nontrivial_transitions(nfa).iter() {
				println!("==== INTERVAL {interval:?} => {targets:?}");
				let next: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> =
					Self::step_on_interval(nfa, &dfa_state.configurations, targets);
				let next: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> = Self::epsilon_closure(nfa, next);

				let (next, operations): (BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)>, Vec<RegisterOp>) =
					dfa.transition_operations(next, &mut register_action_tag);

				dfa.add_state(next, operations);
			}
		}

		dfa
	}

	#[tracing::instrument(skip(self))]
	fn add_state(
		&mut self,
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)>,
		ops: Vec<RegisterOp>,
	) -> usize {
		let dfa_state: DfaState = DfaState {
			configurations: configurations.into_iter().map(|(config, _)| config).collect::<_>(),
		};

		if let Some(&idx) = self.state_map.get(&dfa_state) {
			return idx;
		}

		let idx: usize = self.states.len();
		self.states.push((dfa_state.clone(), IntervalTree::new()));
		self.state_map.insert(dfa_state, idx);
		// TODO final states
		idx
	}

	#[tracing::instrument(skip(nfa))]
	fn step_on_interval(
		nfa: &Nfa<'schema>,
		configurations: &BTreeSet<Configuration<'schema>>,
		targets: &BTreeSet<usize>,
	) -> BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)> {
		let mut next_states: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)> = BTreeSet::new();

		for config in configurations.iter() {
			let nfa_state: &NfaState<'_> = &nfa.states[config.nfa_state];
			for &next in targets.iter() {
				next_states.insert((
					Configuration {
						nfa_state: next,
						registers: config.registers.clone(),
						tags: Vec::new(),
					},
					config.tags.clone(),
				));
			}
		}

		next_states
	}

	#[tracing::instrument(skip(nfa))]
	fn epsilon_closure(
		nfa: &Nfa<'schema>,
		configurations: BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)>,
	) -> BTreeSet<(Configuration<'schema>, Vec<(Tag<'schema>, Sign)>)> {
		let mut closure: BTreeSet<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> = BTreeSet::new();

		let mut stack: Vec<(Configuration<'_>, Vec<(Tag<'_>, Sign)>)> = configurations.into_iter().collect::<_>();

		while let Some((config, inherited)) = stack.pop() {
			closure.insert((config.clone(), inherited.clone()));

			for (transition, target) in nfa.states[config.nfa_state].spontaneous.iter() {
				let mut tags: Vec<(Tag<'_>, Sign)> = config.tags.clone();
				match transition {
					&SpontaneousTransition::Positive(tag) => {
						tags.push((tag, Sign::Positive));
					},
					&SpontaneousTransition::Negative(tag) => {
						tags.push((tag, Sign::Negative));
					},
					SpontaneousTransition::Epsilon => (),
				}
				let new_config: Configuration<'_> = Configuration {
					nfa_state: *target,
					registers: config.registers.clone(),
					tags,
				};

				let key: (Configuration<'_>, Vec<(Tag<'_>, Sign)>) = (new_config, inherited.clone());
				if closure.contains(&key) {
					continue;
				}
				stack.push(key);
			}
		}

		closure
	}

	#[tracing::instrument]
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
				let history: Vec<SymbolicPosition> = self.history(&inherited, tag);
				if history.is_empty() {
					continue;
				}
				let action: RegisterAction = self.operation_rhs(&config.registers, history, tag);
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
				config.registers.insert(tag, r);
			}
			new_configurations.insert((config, inherited));
		}

		(new_configurations, ops)
	}

	fn operation_rhs(
		&self,
		registers: &BTreeMap<Tag<'schema>, usize>,
		history: Vec<SymbolicPosition>,
		tag: Tag<'schema>,
	) -> RegisterAction {
		RegisterAction::Append {
			head: registers[&tag],
			history,
		}
	}

	// TODO replace Sign with SymbolicPosition (or vice versa)
	fn history(&self, history: &[(Tag<'schema>, Sign)], tag1: Tag<'schema>) -> Vec<SymbolicPosition> {
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

impl<'schema> NfaBuilder<'_, 'schema> {
	fn build(&mut self, rule: usize, regex: &'schema Regex, target: usize) -> Result<BTreeSet<Tag<'schema>>, NfaError> {
		match regex {
			Regex::AnyChar => {
				assert_eq!(self.current().wildcard, 0);
				self.current().wildcard = target;
				Ok(BTreeSet::new())
			},
			&Regex::Literal(ch) => {
				self.current().transitions.insert(
					Interval::new(u32::from(ch), u32::from(ch)),
					BTreeSet::from([target]),
					merge_sets,
				);
				Ok(BTreeSet::new())
			},
			Regex::Capture { name, item, meta } => {
				// TODO report unwrap fails if number_captures not called
				self.capture(rule, Some(meta.id.unwrap()), name, item, target)
			},
			Regex::Group { negated, items } => {
				if *negated {
					let mut intervals: Vec<Interval<u32>> = Vec::with_capacity(items.len());
					for &(start, end) in items.iter() {
						if start > end {
							todo!("warn invalid range");
						}
						intervals.push(Interval::new(u32::from(start), u32::from(end)));
					}
					for interval in Interval::complement(&mut intervals).into_iter() {
						self.current()
							.transitions
							.insert(interval, BTreeSet::from([target]), merge_sets);
					}
				} else {
					for &(start, end) in items.iter() {
						if start > end {
							todo!("warn invalid range");
						}
						self.current().transitions.insert(
							Interval::new(u32::from(start), u32::from(end)),
							BTreeSet::from([target]),
							merge_sets,
						);
					}
				}
				Ok(BTreeSet::new())
			},
			Regex::KleeneClosure(item) => {
				let item_start: usize = self.nfa.new_state();
				let item_end: usize = self.nfa.new_state();
				let item_skip: usize = self.nfa.new_state();

				let n: usize = self.begins;
				self.begins += 1;

				self.current()
					.spontaneous
					.push((SpontaneousTransition::Epsilon, item_start));
				// .push((SpontaneousTransition::BeginContext(n), item_start));
				self.current()
					.spontaneous
					.push((SpontaneousTransition::Epsilon, item_skip));

				self.current = item_start;
				let tags: BTreeSet<Tag<'_>> = self.build(rule, item, item_end)?;

				self.current = item_end;
				self.current()
					.spontaneous
					.push((SpontaneousTransition::Epsilon, item_start));
				self.current()
					.spontaneous
					.push((SpontaneousTransition::Epsilon, target));
				// .push((SpontaneousTransition::CommitContext(tags.clone(), n), target));

				self.current = item_skip;
				self.negative_tags(tags.iter().copied(), target);

				Ok(tags)
			},
			Regex::BoundedRepetition { start, end, item } => {
				if start > end {
					todo!("warn invalid repetition");
				}
				let mut tags: BTreeSet<Tag<'_>> = BTreeSet::new();
				for i in 0..*start {
					let sub_target: usize = if i + 1 < *start { self.nfa.new_state() } else { target };
					tags.append(&mut self.build(rule, item, sub_target)?);
					self.current = sub_target;
				}
				for i in *start..*end {
					let sub_target: usize = if i < *end { self.nfa.new_state() } else { target };
					self.current()
						.spontaneous
						.push((SpontaneousTransition::Epsilon, target));
					tags.append(&mut self.build(rule, item, target)?);
					self.current = sub_target;
				}
				Ok(tags)
			},
			Regex::Sequence(items) => {
				let mut tags: BTreeSet<Tag<'_>> = BTreeSet::new();
				for (i, sub_item) in items.iter().enumerate() {
					let sub_target: usize = if i + 1 < items.len() {
						self.nfa.new_state()
					} else {
						target
					};
					tags.append(&mut self.build(rule, sub_item, sub_target)?);
					self.current = sub_target;
				}
				Ok(tags)
			},
			Regex::Alternation(items) => self.alternate(
				items
					.iter()
					.map(|sub_item| |builder: &mut Self, sub_target| builder.build(rule, sub_item, sub_target)),
				target,
			),
		}
	}

	fn capture(
		&mut self,
		rule: usize,
		id: Option<NonZero<u32>>,
		name: &'schema str,
		item: &'schema Regex,
		target: usize,
	) -> Result<BTreeSet<Tag<'schema>>, NfaError> {
		let variable: Variable<'_> = Variable {
			rule,
			id,
			capture: name,
		};

		let start_capture: Tag<'_> = Tag::StartCapture(variable);
		let end_capture: Tag<'_> = Tag::EndCapture(variable);

		let sub_start: usize = self.nfa.new_state();
		let sub_end: usize = self.nfa.new_state();

		self.current()
			.spontaneous
			.push((SpontaneousTransition::Positive(start_capture), sub_start));

		self.current = sub_start;
		let mut tags: BTreeSet<Tag<'_>> = self.build(rule, item, sub_end)?;

		self.current = sub_end;
		self.current()
			.spontaneous
			.push((SpontaneousTransition::Positive(end_capture), target));

		tags.insert(start_capture);
		tags.insert(end_capture);

		Ok(tags)
	}

	fn alternate<I, F>(&mut self, items: I, target: usize) -> Result<BTreeSet<Tag<'schema>>, NfaError>
	where
		I: Iterator<Item = F>,
		F: Fn(&mut Self, usize) -> Result<BTreeSet<Tag<'schema>>, NfaError>,
	{
		let orig_start: usize = self.current;

		let mut tags: BTreeSet<Tag<'_>> = BTreeSet::new();
		let mut intermediate_states: Vec<(usize, BTreeSet<Tag<'_>>)> = Vec::new();

		// let begin_context: usize = self.nfa.new_state();

		let n: usize = self.begins;
		self.begins += 1;
		// self.current()
		// 	.spontaneous
		// 	.push((SpontaneousTransition::BeginContext(n), begin_context));

		for (i, sub_item) in items.enumerate() {
			let sub_start: usize = self.nfa.new_state();
			let sub_target: usize = self.nfa.new_state();

			self.current = orig_start;
			self.current()
				.spontaneous
				.push((SpontaneousTransition::Epsilon, sub_start));

			self.current = sub_start;
			intermediate_states.push((sub_target, sub_item(self, sub_target)?));
			// let mut sub_tags: BTreeSet<Tag<'schema>> = sub_item(self, sub_target)?;

			// self.current = sub_target;
			// self.current()
			// 	.spontaneous
			// 	.push((SpontaneousTransition::CommitContext(sub_tags.clone(), n), target));

			// tags.append(&mut sub_tags);
		}

		for (i, (sub_state, sub_tags)) in intermediate_states.iter().enumerate() {
			let sub_target: usize = self.nfa.new_state();

			self.current = *sub_state;
			for (other, (_, other_tags)) in intermediate_states.iter().enumerate() {
				if other == i {
					continue;
				}
				self.negative_tags(other_tags.iter().copied(), sub_target);
				self.current = sub_target;
			}

			self.current()
				.spontaneous
				.push((SpontaneousTransition::Epsilon, target));

			sub_tags.iter().for_each(|&tag| {
				tags.insert(tag);
			});
		}

		Ok(tags)
	}

	fn negative_tags(&mut self, tags: impl Iterator<Item = Tag<'schema>>, target: usize) {
		for tag in tags {
			let next: usize = self.nfa.new_state();
			self.current()
				.spontaneous
				.push((SpontaneousTransition::Negative(tag), next));
			self.current = next;
		}
		self.current()
			.spontaneous
			.push((SpontaneousTransition::Epsilon, target));
	}

	fn current(&mut self) -> &mut NfaState<'schema> {
		&mut self.nfa.states[self.current]
	}
}

impl<'schema> DfaState<'schema> {
	fn nontrivial_transitions(&self, nfa: &Nfa<'schema>) -> IntervalTree<u32, BTreeSet<usize>> {
		let mut transitions: IntervalTree<u32, BTreeSet<usize>> = IntervalTree::new();

		for config in self.configurations.iter() {
			let nfa_state: &NfaState<'_> = &nfa.states[config.nfa_state];
			for (interval, targets) in nfa_state.transitions.iter() {
				transitions.insert(*interval, targets.clone(), merge_sets);
			}
		}

		transitions
	}

	fn debug(&self, nfa: &Nfa<'schema>) {
		for config in self.configurations.iter() {
			println!("- NFA state {}", config.nfa_state);
			println!("\t- {:?}", nfa.states[config.nfa_state]);
			for (tag, register) in config.registers.iter() {
				println!("\t- {tag:?}: {register}");
			}
			println!("- History: {:?}", config.tags);
		}
	}
}

/*
impl<'schema> std::ops::Index<usize> for Nfa<'schema> {
	type Output = NfaState<'schema>;

	fn index(&self, i: usize) -> &Self::Output {
		&self.states[i]
	}
}

impl<'schema> std::ops::IndexMut<usize> for Nfa<'schema> {
	fn index_mut(&mut self, i: usize) -> &mut Self::Output {
		&mut self.states[i]
	}
}
*/

fn merge_sets(v1: &BTreeSet<usize>, v2: &BTreeSet<usize>) -> BTreeSet<usize> {
	v1 | v2
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn stuff() {
		let r: Regex = Regex::from_pattern("0((?<foobar>1(2[a-zA-Z])*)|(?<baz>xyz))world").unwrap();
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", r);
		let nfa: Nfa<'_> = Nfa::for_schema(&schema).unwrap();
		let b: bool = nfa.simulate("012a2b2cworld").is_some();
		assert!(b);
		let b: bool = nfa.simulate("0xyzworld").is_some();
		assert!(b);
	}
}
