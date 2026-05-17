use std::collections::BTreeMap;

use super::*;
use crate::search::SymbolicChar;

#[derive(Debug, Clone)]
pub struct Path(Vec<PathComponent>);

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PathComponent {
	Literal(String),
	Capture {
		rule_idx: RuleIdx,
		qualified_name: String,
		contents: Vec<Self>,
	},
	QueryWildcard,
	PatternWildcard,
}

#[derive(Debug, Clone, Copy)]
pub struct TarjanSccData {
	pub index: Option<usize>,
	pub low_link: usize,
	pub on_stack: bool,
	pub scc: usize,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum PathEdge {
	Literal(char),
	Capture { sub_rule: SubRule, start: bool },
	QueryWildcard,
	PatternWildcard,
}

type PartialPath = Vec<PathEdge>;

#[derive(Debug, Clone, Copy)]
struct StatePair<'a> {
	nfas: (&'a Tnfa, &'a Tnfa),
	states: (NfaIdx, NfaIdx),
}

impl Eq for StatePair<'_> {}

impl Ord for StatePair<'_> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.states.cmp(&other.states)
	}
}

impl PartialEq for StatePair<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other).is_eq()
	}
}

impl PartialOrd for StatePair<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl<'a> StatePair<'a> {
	fn new(nfa1: &'a Tnfa, nfa2: &'a Tnfa, state1: NfaIdx, state2: NfaIdx) -> Self {
		Self {
			nfas: (nfa1, nfa2),
			states: (state1, state2),
		}
	}

	fn state1(&self) -> &NfaState {
		&self.nfas.0[self.states.0]
	}

	fn state2(&self) -> &NfaState {
		&self.nfas.1[self.states.1]
	}
}

impl std::fmt::Display for Path {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for part in self.0.iter() {
			part.fmt(fmt)?;
		}
		Ok(())
	}
}

impl PathEdge {
	fn condense(edges: &mut Vec<PathEdge>) {
		let mut i: usize = 1;
		while i < edges.len() {
			let previous: &PathEdge = &edges[i - 1];
			let current: &PathEdge = &edges[i];
			match (previous, current) {
				(
					PathEdge::QueryWildcard | PathEdge::PatternWildcard,
					PathEdge::QueryWildcard | PathEdge::PatternWildcard,
				) => {
					edges[i - 1] = PathEdge::QueryWildcard;
					edges.remove(i);
				},
				_ => {
					i += 1;
				},
			}
		}
	}
}

impl Path {
	pub fn iter(&self) -> std::slice::Iter<'_, PathComponent> {
		self.0.iter()
	}

	fn new(mut parts: Vec<PathComponent>) -> Self {
		Self::condense(&mut parts);
		Self(parts)
	}

	fn condense(parts: &mut Vec<PathComponent>) {
		for part in parts.iter_mut() {
			match part {
				PathComponent::Capture { contents, .. } => {
					Self::condense(contents);
				},
				_ => (),
			}
		}
		let mut i: usize = 1;
		while i < parts.len() {
			let previous: &PathComponent = &parts[i - 1];
			let current: &PathComponent = &parts[i];
			match (previous, current) {
				(PathComponent::Literal(s1), PathComponent::Literal(s2)) => {
					let s: String = s1.to_owned() + s2;
					parts[i - 1] = PathComponent::Literal(s);
					parts.remove(i);
				},
				(
					PathComponent::QueryWildcard | PathComponent::PatternWildcard,
					PathComponent::QueryWildcard | PathComponent::PatternWildcard,
				) => {
					parts.remove(i);
				},
				_ => {
					i += 1;
				},
			}
		}
	}
}

impl std::fmt::Display for PathComponent {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Literal(s) => {
				for ch in s.chars() {
					match ch {
						'(' | ')' | '*' | '\\' => {
							fmt.write_str("\\")?;
						},
						_ => (),
					}
					ch.fmt(fmt)?;
				}
			},
			Self::Capture {
				rule_idx: _,
				qualified_name,
				contents,
			} => {
				fmt.write_fmt(format_args!("(?<{qualified_name}>"))?;
				for part in contents.iter() {
					part.fmt(fmt)?;
				}
				fmt.write_str(")")?;
			},
			Self::QueryWildcard | Self::PatternWildcard => {
				fmt.write_str("*")?;
			},
		}
		Ok(())
	}
}

// impl PathComponent {
// 	fn is_wildcard(&self) -> bool {
// 		match self {
// 			Self::Literal(_) => false,
// 			Self::QueryWildcard => true,
// 			Self::PatternWildcard => false,
// 			Self::Capture { contents, .. } => contents.iter().all(Self::is_wildcard),
// 		}
// 	}
// }

// impl PathEdge {
// 	fn is_wildcard(&self) -> bool {
// 		match self {
// 			Self::QueryWildcard => true,
// 			_ => false,
// 		}
// 	}
// }

impl Tnfa {
	pub fn intersect(&self, other: &Self) -> Self {
		let begin: NfaIdx = NfaIdx::BEGIN;

		let mut stack: Vec<(StatePair<'_>, NfaIdx)> = vec![(StatePair::new(self, other, begin, begin), begin)];
		let mut seen: BTreeMap<StatePair<'_>, NfaIdx> = BTreeMap::from_iter(stack.iter().copied());

		let mut intersection: Self = Self {
			states: vec![NfaState {
				idx: NfaIdx::BEGIN,
				name: Cow::Borrowed("begin"),
				transitions: Transitions::Spontaneous(Vec::new()),
				maybe_accepts_for_rule: None,
			}],
			tags: Vec::new(),
		};

		while let Some((pair, state)) = stack.pop() {
			if let Some(rule) = pair.state1().maybe_accepts_for_rule
				&& pair.state2().is_accepting()
			{
				intersection[state].maybe_accepts_for_rule = Some(rule);
				assert_eq!(intersection[state].transitions.len(), 0);
				continue;
			}

			let mut lookup_state = |target, combined: &mut Self| {
				*seen.entry(target).or_insert_with(|| {
					let next: NfaIdx = combined.new_state(format!("({}, {})", target.states.0, target.states.1));
					stack.push((target, next));
					next
				})
			};

			// TODO: nicer way to combine the branches?
			match (&pair.state1().transitions, &pair.state2().transitions) {
				(Transitions::Spontaneous(transitions1), Transitions::Spontaneous(transitions2)) => {
					if !transitions1.is_empty() {
						intersection[state].transitions = Transitions::Spontaneous(
							transitions1
								.iter()
								.map(|spontaneous1| {
									let next: NfaIdx = lookup_state(
										StatePair::new(self, other, spontaneous1.target, pair.states.1),
										&mut intersection,
									);
									SpontaneousTransition {
										kind: spontaneous1.kind.clone(),
										target: next,
									}
								})
								.collect::<Vec<_>>(),
						);
					} else {
						intersection[state].transitions = Transitions::Spontaneous(
							transitions2
								.iter()
								.map(|spontaneous2| {
									assert_eq!(spontaneous2.kind, SpontaneousTransitionKind::Epsilon);
									let target: NfaIdx = lookup_state(
										StatePair::new(self, other, pair.states.0, spontaneous2.target),
										&mut intersection,
									);
									SpontaneousTransition {
										kind: SpontaneousTransitionKind::Epsilon,
										target,
									}
								})
								.collect::<Vec<_>>(),
						);
					}
				},
				(Transitions::Spontaneous(transitions1), _) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions1
							.iter()
							.map(|spontaneous1| {
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, spontaneous1.target, pair.states.1),
									&mut intersection,
								);
								SpontaneousTransition {
									kind: spontaneous1.kind.clone(),
									target: next,
								}
							})
							.collect::<Vec<_>>(),
					);
				},
				(_, Transitions::Spontaneous(transitions2)) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions2
							.iter()
							.map(|spontaneous2| {
								assert_eq!(spontaneous2.kind, SpontaneousTransitionKind::Epsilon);
								let target: NfaIdx = lookup_state(
									StatePair::new(self, other, pair.states.0, spontaneous2.target),
									&mut intersection,
								);
								SpontaneousTransition {
									kind: SpontaneousTransitionKind::Epsilon,
									target,
								}
							})
							.collect::<Vec<_>>(),
					);
				},
				(Transitions::Interval(transitions1), Transitions::Interval(transitions2)) => {
					let mut combined: IntervalTree<u32, NfaIdx> = IntervalTree::new();
					for (interval1, &target1) in transitions1.iter() {
						for (interval2, &target2) in transitions2.iter() {
							if interval2.start() != interval2.end() {
								// Query wildcard.
								assert_eq!((interval2.start(), interval2.end()), (0, u32::from(char::MAX)));
								let next: NfaIdx =
									lookup_state(StatePair::new(self, other, target1, target2), &mut intersection);
								combined.insert(Interval::new(0, u32::MAX), next, PolicyUnique);
							} else {
								// Query literal character.
								assert_eq!(interval2.start(), interval2.end());
								let Some(overlap): Option<Interval<u32>> = interval1.overlap(&interval2) else {
									continue;
								};
								assert_eq!(overlap, interval2);
								let next: NfaIdx =
									lookup_state(StatePair::new(self, other, target1, target2), &mut intersection);
								combined.insert(overlap, next, PolicyUnique);
							}
						}
					}
					intersection[state].transitions = Transitions::Interval(combined);
				},
			}
		}

		let can_accept: Vec<bool> = intersection.compute_live_states();
		for state in intersection.states.iter_mut() {
			match &mut state.transitions {
				Transitions::Interval(transitions) => {
					transitions.retain(|&(_interval, target)| can_accept[target.0]);
				},
				Transitions::Spontaneous(transitions) => {
					transitions.retain(|transition| can_accept[transition.target.0]);
				},
			}
		}

		intersection
	}

	pub fn can_accept(&self) -> bool {
		self.compute_live_states()[0]
	}

	fn compute_live_states(&self) -> Vec<bool> {
		let mut acceptable: Vec<bool> = vec![false; self.states.len()];

		for state in self.states.iter() {
			if state.is_accepting() {
				acceptable[state.idx.0] = true;
			}
		}

		let mut changed: bool = true;
		while changed {
			changed = false;
			for state in self.states.iter() {
				for target_idx in state.transitions.successors() {
					if acceptable[target_idx.0] {
						let old_can_accept: bool = std::mem::replace(&mut acceptable[state.idx.0], true);
						if !old_can_accept {
							changed = true;
						}
					}
				}
			}
		}

		acceptable
	}

	pub fn tarjan_scc(&self) -> (Vec<Vec<NfaIdx>>, Vec<TarjanSccData>, Vec<NfaIdx>) {
		let mut data: Vec<TarjanSccData> = vec![
			TarjanSccData {
				index: None,
				low_link: 0,
				on_stack: false,
				scc: usize::MAX,
			};
			self.states.len()
		];

		let mut indices: Vec<NfaIdx> = Vec::with_capacity(self.states.len());
		let mut stack: Vec<NfaIdx> = Vec::new();

		let mut sccs: Vec<Vec<NfaIdx>> = Vec::new();

		for state in self.states.iter() {
			self.strong_connect(&mut data, &mut stack, &mut indices, state.idx, &mut sccs);
		}

		sccs.reverse();
		for data in data.iter_mut() {
			data.scc = sccs.len() - data.scc - 1;
		}

		(sccs, data, indices)
	}

	fn strong_connect(
		&self,
		data: &mut [TarjanSccData],
		stack: &mut Vec<NfaIdx>,
		indices: &mut Vec<NfaIdx>,
		idx: NfaIdx,
		sccs: &mut Vec<Vec<NfaIdx>>,
	) {
		if data[idx.0].index.is_some() {
			return;
		}

		let i: usize = indices.len();
		data[idx.0] = TarjanSccData {
			index: Some(i),
			low_link: i,
			on_stack: true,
			scc: usize::MAX,
		};
		stack.push(idx);
		indices.push(idx);

		let state: &NfaState = &self[idx];
		for jdx in state.transitions.successors() {
			if let Some(j) = data[jdx.0].index {
				if data[jdx.0].on_stack {
					data[idx.0].low_link = std::cmp::min(data[idx.0].low_link, j);
				}
			} else {
				self.strong_connect(data, stack, indices, jdx, sccs);
				data[idx.0].low_link = std::cmp::min(data[idx.0].low_link, data[jdx.0].low_link);
			}
		}

		if data[idx.0].low_link == i {
			let mut scc: Vec<NfaIdx> = Vec::new();
			loop {
				let jdx: NfaIdx = stack.pop().unwrap();
				data[jdx.0].on_stack = false;
				data[jdx.0].scc = sccs.len();
				scc.push(jdx);
				if jdx == idx {
					break;
				}
			}
			sccs.push(scc);
		}
	}

	pub fn compute_paths(&self) -> Vec<Path> {
		let (sccs, data, _indices): (Vec<Vec<NfaIdx>>, Vec<TarjanSccData>, Vec<NfaIdx>) = self.tarjan_scc();

		let mut cache: Vec<Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>> = vec![None; self.states.len()];

		if self[NfaIdx::BEGIN].transitions.len() == 0 {
			return Vec::new();
		}

		let prefix: PartialPath = Vec::new();
		let mut seen_prefix: BTreeMap<NfaIdx, BTreeSet<PartialPath>> = BTreeMap::new();
		let mut finished: Vec<(PartialPath, RuleIdx)> = Vec::new();
		self.compute_paths_internal(
			&self[NfaIdx::BEGIN],
			&prefix,
			&mut seen_prefix,
			&sccs,
			&data,
			&mut cache,
			&mut finished,
		);
		let mut finished: Vec<Vec<PathComponent>> = Vec::new();
		let paths: &Vec<(NfaIdx, PartialPath, RuleIdx)> = cache[0].as_ref().unwrap();

		for (_, edges, rule_idx) in paths.iter() {
			let mut path: Vec<PathComponent> = Vec::new();
			let mut maybe_capture: Option<(SubRule, Vec<PathComponent>)> = None;
			// let all_wildcards: bool = scc_path.iter().all(PathEdge::is_wildcard);

			// TODO code duplication
			for edge in edges.iter().rev() {
				match edge {
					PathEdge::Literal(ch) => {
						// TODO
						if let Some((_sub_rule, capture_path)) = maybe_capture.as_mut() {
							capture_path.push(PathComponent::Literal(ch.to_string()));
						} else {
							path.push(PathComponent::Literal(ch.to_string()));
						}
					},
					PathEdge::Capture { sub_rule, start } => {
						if let Some((current_rule, capture_path)) = maybe_capture {
							assert!(!start);
							assert_eq!(sub_rule, &current_rule);
							path.push(PathComponent::Capture {
								rule_idx: *rule_idx,
								qualified_name: sub_rule.qualified_name.clone(),
								contents: capture_path,
							});
							maybe_capture = None;
						} else {
							assert!(start);
							maybe_capture = Some((sub_rule.clone(), Vec::new()));
						}
					},
					PathEdge::QueryWildcard => {
						if let Some((_sub_rule, capture_path)) = maybe_capture.as_mut() {
							capture_path.push(PathComponent::QueryWildcard);
						} else {
							path.push(PathComponent::QueryWildcard);
						}
					},
					PathEdge::PatternWildcard => {
						if let Some((_sub_rule, capture_path)) = maybe_capture.as_mut() {
							capture_path.push(PathComponent::PatternWildcard);
						} else {
							path.push(PathComponent::PatternWildcard);
						}
					},
				}
			}

			finished.push(path);
		}
		finished.iter_mut().for_each(Path::condense);
		return finished.into_iter().map(Path::new).collect::<Vec<_>>();
	}

	fn compute_paths_internal<'a>(
		&self,
		entry: &NfaState,
		prefix: &PartialPath,
		seen_prefix: &mut BTreeMap<NfaIdx, BTreeSet<PartialPath>>,
		sccs: &[Vec<NfaIdx>],
		data: &[TarjanSccData],
		cache: &'a mut [Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>],
		finished: &mut Vec<(PartialPath, RuleIdx)>,
	) -> Vec<(NfaIdx, PartialPath, RuleIdx)> {
		// ) {
		if seen_prefix
			.entry(entry.idx)
			.or_insert_with(BTreeSet::new)
			.contains(prefix)
		{
			return Vec::new();
		}
		seen_prefix.get_mut(&entry.idx).unwrap().insert(prefix.clone());
		if let Some(cached) = cache[entry.idx.0].as_ref() {
			return cached.clone();
			// return;
		}

		let mut paths: Vec<(NfaIdx, PartialPath, RuleIdx)> = Vec::new();
		let scc: &Vec<NfaIdx> = &sccs[data[entry.idx.0].scc];
		assert!(!scc.is_empty());
		if let Some(rule) = entry.maybe_accepts_for_rule {
			finished.push((prefix.clone(), rule));
			return cache[entry.idx.0].insert(vec![(entry.idx, Vec::new(), rule)]).clone();
			// cache[entry.idx.0].insert(vec![(entry.idx, Vec::new(), rule)]);
			// return;
		}
		if scc.len() == 1 {
			assert!(entry.transitions.len() > 0);
			match &entry.transitions {
				Transitions::Interval(transitions) => {
					assert_eq!(transitions.len(), 1);
					// println!("- state {} has {} interval transitions", entry.idx, transitions.len());
					for ((interval, &target), mut prefix) in transitions
						.iter()
						.zip(std::iter::repeat_n(prefix.clone(), transitions.len()))
					{
						assert!(data[target.0].index > data[entry.idx.0].index);
						assert!(data[target.0].scc > data[entry.idx.0].scc);
						let ch: PathEdge = if interval.start() == interval.end() {
							let ch: char = char::try_from(interval.start()).unwrap();
							PathEdge::Literal(ch)
						} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
							PathEdge::QueryWildcard
						} else {
							PathEdge::PatternWildcard
						};
						prefix.push(ch.clone());
						PathEdge::condense(&mut prefix);
						// self.compute_paths_internal(&self[target], &prefix, seen_prefix, sccs, data, cache, finished);

						let mut partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self
							.compute_paths_internal(&self[target], &prefix, seen_prefix, sccs, data, cache, finished);
						for (_state, path, _rule) in partials.iter_mut() {
							path.push(ch.clone());
							PathEdge::condense(path);
						}
						partials.sort();
						partials.dedup();
						// println!("- did interval for {}: {}", entry.idx, partials.len());
						// cache[entry.idx.0] = Some(partials);
						// paths.push((&self[target], vec![ch]));
						assert!(cache[entry.idx.0].is_none());
						return cache[entry.idx.0].insert(partials).clone();
						// cache[entry.idx.0].insert(vec![(target, vec![ch], RuleIdx::NIL)]);
						// return;
					}
					unreachable!();
				},
				Transitions::Spontaneous(transitions) => {
					// println!(
					// 	"- state {} has {} spontaneous transitions",
					// 	entry.idx,
					// 	transitions.len()
					// );
					for transition in transitions.iter() {
						assert!(data[transition.target.0].scc > data[entry.idx.0].scc);
						match &transition.kind {
							SpontaneousTransitionKind::Positive(
								tag @ (Tag::StartCapture(sub_rule) | Tag::StopCapture(sub_rule)),
							) => {
								if sub_rule.is_leaf() {
									// assert_eq!(maybe_capture, None);
									let start: bool = matches!(tag, Tag::StartCapture(_));
									// paths.push((
									// 	&self[transition.target],
									// 	vec![PathEdge::Capture {
									// 		sub_rule: sub_rule.clone(),
									// 		start,
									// 	}],
									// ));
									let mut prefix: PartialPath = prefix.clone();
									let edge: PathEdge = PathEdge::Capture {
										sub_rule: sub_rule.clone(),
										start,
									};
									prefix.push(edge.clone());
									let mut partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self
										.compute_paths_internal(
											&self[transition.target],
											&prefix,
											seen_prefix,
											sccs,
											data,
											cache,
											finished,
										);
									for (_state, path, _rule) in partials.iter_mut() {
										path.push(edge.clone());
									}
									paths.extend(partials.into_iter());
									continue;
								}
							},
							SpontaneousTransitionKind::Negative(_) => (),
							SpontaneousTransitionKind::Epsilon => (),
						}
						let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self.compute_paths_internal(
							&self[transition.target],
							prefix,
							seen_prefix,
							sccs,
							data,
							cache,
							finished,
						);
						paths.extend(partials.into_iter());
						// paths.push((&self[transition.target], Vec::new()));
					}
					paths.sort();
					paths.dedup();
					assert!(cache[entry.idx.0].is_none());
					return cache[entry.idx.0].insert(paths).clone();
				},
			}
		} else {
			return self.compute_scc_path(entry, prefix, seen_prefix, &sccs, &data, cache, finished);
		}
	}

	fn compute_scc_path<'a>(
		&self,
		entry: &NfaState,
		prefix: &PartialPath,
		seen_prefix: &mut BTreeMap<NfaIdx, BTreeSet<PartialPath>>,
		sccs: &[Vec<NfaIdx>],
		data: &[TarjanSccData],
		cache: &'a mut [Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>],
		finished2: &mut Vec<(PartialPath, RuleIdx)>,
	) -> Vec<(NfaIdx, PartialPath, RuleIdx)> {
		let mut finished: Vec<(NfaIdx, PartialPath, RuleIdx)> = Vec::new();

		let mut stack: Vec<(&NfaState, PartialPath, BTreeSet<NfaIdx>)> =
			vec![(entry, vec![PathEdge::PatternWildcard], BTreeSet::from([entry.idx]))];

		while let Some((state, path, seen)) = stack.pop() {
			assert!(state.transitions.len() > 0);
			match &state.transitions {
				Transitions::Interval(transitions) => {
					assert_eq!(transitions.len(), 1);
					for ((interval, &target), (mut path, mut seen)) in transitions
						.iter()
						.zip(std::iter::repeat_n((path, seen), transitions.len()))
					{
						assert_eq!(data[target.0].scc, data[entry.idx.0].scc);
						let inserted: bool = seen.insert(target);
						assert!(inserted);
						let ch: PathEdge = if interval.start() == interval.end() {
							let ch: char = char::try_from(interval.start()).unwrap();
							PathEdge::Literal(ch)
						} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
							PathEdge::QueryWildcard
						} else {
							PathEdge::PatternWildcard
						};
						path.push(ch);
						stack.push((&self[target], path, seen));
					}
				},
				Transitions::Spontaneous(transitions) => {
					for (transition, (mut path, mut seen)) in transitions
						.iter()
						.zip(std::iter::repeat_n((path, seen), transitions.len()))
					{
						assert!(data[transition.target.0].scc >= data[entry.idx.0].scc);
						if data[transition.target.0].scc != data[entry.idx.0].scc {
							let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self.compute_paths_internal(
								&self[transition.target],
								prefix,
								seen_prefix,
								sccs,
								data,
								cache,
								finished2,
							);
							for (end, mut remaining, rule) in partials.into_iter() {
								remaining.extend(path.iter().rev().cloned());
								PathEdge::condense(&mut remaining);
								finished.push((end, remaining, rule));
							}
							continue;
						}
						if seen.contains(&transition.target) {
							continue;
						}
						let inserted: bool = seen.insert(transition.target);
						assert!(inserted);
						match &transition.kind {
							SpontaneousTransitionKind::Positive(
								tag @ (Tag::StartCapture(sub_rule) | Tag::StopCapture(sub_rule)),
							) => {
								if sub_rule.is_leaf() {
									// assert_eq!(maybe_capture, None);
									let start: bool = matches!(tag, Tag::StartCapture(_));
									path.push(PathEdge::Capture {
										sub_rule: sub_rule.clone(),
										start,
									});
									stack.push((&self[transition.target], path, seen));
									continue;
								}
							},
							SpontaneousTransitionKind::Negative(_) => (),
							SpontaneousTransitionKind::Epsilon => (),
						}
						stack.push((&self[transition.target], path, seen));
					}
				},
			}
		}
		finished.sort();
		finished.dedup();
		assert!(cache[entry.idx.0].is_none());
		cache[entry.idx.0].insert(finished).clone()
	}
}

impl PathComponent {
	pub fn to_query_string(&self) -> String {
		let mut buf: String = String::new();
		match self {
			Self::Literal(s) => {
				for ch in s.chars() {
					match ch {
						'*' | '\\' => {
							buf.push('\\');
						},
						_ => (),
					}
					buf.push(ch);
				}
			},
			Self::QueryWildcard | Self::PatternWildcard => {
				buf.push('*');
			},
			Self::Capture { contents, .. } => {
				for part in contents.iter() {
					buf.push_str(&part.to_query_string());
				}
			},
		}
		buf
	}

	pub fn to_symbolic_chars(&self) -> Vec<SymbolicChar> {
		let mut buf: Vec<SymbolicChar> = Vec::new();
		match self {
			Self::Literal(s) => {
				for ch in s.chars() {
					buf.push(SymbolicChar::Literal(ch));
				}
			},
			Self::QueryWildcard | Self::PatternWildcard => {
				buf.push(SymbolicChar::WildcardStar);
			},
			Self::Capture { contents, .. } => {
				for part in contents.iter() {
					buf.extend(part.to_symbolic_chars().into_iter());
				}
			},
		}
		buf
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn nfa_decomp() {
		// let paths = nfa_for("abc(?<hello>(foo*|bar|ba*z)(qux|quux))def").to_wildcard_string();
		// let paths = nfa_for("abc((?<var>foo*|bar|ba*z)0(qux|quux)*)def").to_wildcard_string();
		let nfa = nfa_for(r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)");
		let search = nfa_for("abc@.*mail.*example.*");
		// nfa.to_wildcard_string();
		// search.to_wildcard_string();
		// let nfa = nfa_for(r"@((?<parts>\w+)\.)*(?<tld>\w+)");
		// let nfa = nfa_for(r"@\.(?<tld>\w+)");
		// let search = nfa_for("@.*mail.*example.*");
		// let nfa = nfa_for(r"(?<hello>a.)*");
		// let search = nfa_for("(ab)*");
		// println!("{}", nfa.intersect(&search).to_dot_output());
		// return;
		let paths = nfa.intersect(&search).compute_paths();
		let mut paths = paths.iter().map(ToString::to_string).collect::<Vec<_>>();
		paths.sort();
		paths.dedup();
		println!("===");
		for path in paths.iter() {
			println!("- {path}");
		}
		println!("=== {} paths", paths.len());
	}

	fn nfa_for(pattern: &str) -> Tnfa {
		let regex: Regex = Regex::from_pattern(pattern).unwrap().inner;
		Tnfa::for_regex(&regex)
	}
}
