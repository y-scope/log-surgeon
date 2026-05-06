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
	// Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct TarjanSccData {
	pub index: Option<usize>,
	pub low_link: usize,
	pub on_stack: bool,
	pub scc: usize,
}

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

impl PathComponent {
	fn is_wildcard(&self) -> bool {
		match self {
			Self::Literal(_) => false,
			Self::QueryWildcard => true,
			Self::PatternWildcard => false,
			Self::Capture { contents, .. } => contents.iter().all(Self::is_wildcard),
		}
	}
}

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
				(Transitions::Spontaneous(spontaneous1), Transitions::Spontaneous(spontaneous2)) => {
					if !spontaneous1.is_empty() {
						intersection[state].transitions = Transitions::Spontaneous(
							spontaneous1
								.iter()
								.map(|transition1| {
									let next: NfaIdx = lookup_state(
										StatePair::new(self, other, transition1.target, pair.states.1),
										&mut intersection,
									);
									SpontaneousTransition {
										kind: transition1.kind.clone(),
										target: next,
									}
								})
								.collect::<Vec<_>>(),
						);
					} else {
						assert!(!spontaneous2.is_empty());
						intersection[state].transitions = Transitions::Spontaneous(
							spontaneous2
								.iter()
								.map(|transition2| {
									assert_eq!(transition2.kind, SpontaneousTransitionKind::Epsilon);
									let next: NfaIdx = lookup_state(
										StatePair::new(self, other, pair.states.0, transition2.target),
										&mut intersection,
									);
									SpontaneousTransition {
										kind: SpontaneousTransitionKind::Epsilon,
										target: next,
									}
								})
								.collect::<Vec<_>>(),
						);
					}
				},
				(Transitions::Spontaneous(spontaneous), _) => {
					intersection[state].transitions = Transitions::Spontaneous(
						spontaneous
							.iter()
							.map(|transition1| {
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, transition1.target, pair.states.1),
									&mut intersection,
								);
								SpontaneousTransition {
									kind: transition1.kind.clone(),
									target: next,
								}
							})
							.collect::<Vec<_>>(),
					);
				},
				(_, Transitions::Spontaneous(spontaneous)) => {
					intersection[state].transitions = Transitions::Spontaneous(
						spontaneous
							.iter()
							.map(|transition2| {
								assert_eq!(transition2.kind, SpontaneousTransitionKind::Epsilon);
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, pair.states.0, transition2.target),
									&mut intersection,
								);
								SpontaneousTransition {
									kind: SpontaneousTransitionKind::Epsilon,
									target: next,
								}
							})
							.collect::<Vec<_>>(),
					);
				},
				// (Transitions::Spontaneous(spontaneous), _, _, other_state)
				// | (_, Transitions::Spontaneous(spontaneous), other_state, _) => {
				// 	intersection[state].transitions = Transitions::Spontaneous(
				// 		spontaneous
				// 			.iter()
				// 			.map(|transition1| {
				// 				let next: NfaIdx = lookup_state(
				// 					StatePair::new(self, other, transition1.target, other_state),
				// 					&mut intersection,
				// 				);
				// 				SpontaneousTransition {
				// 					kind: transition1.kind.clone(),
				// 					target: next,
				// 				}
				// 			})
				// 			.collect::<Vec<_>>(),
				// 	);
				// },
				(Transitions::Interval(transitions1), Transitions::Interval(transitions2)) => {
					let mut combined: IntervalTree<u32, NfaIdx> = IntervalTree::new();
					for (interval1, &target1) in transitions1.iter() {
						for (interval2, &target2) in transitions2.iter() {
							// if interval1.start() == interval1.end() {
							if interval2.start() != interval2.end() {
								// Wildcard search
								assert_eq!(interval2.start(), 0);
								assert_eq!(interval2.end(), u32::from(char::MAX));
								let next: NfaIdx =
									lookup_state(StatePair::new(self, other, target1, target2), &mut intersection);
								combined.insert(Interval::new(0, u32::MAX), next, PolicyUnique);
							} else {
								let Some(overlap): Option<Interval<u32>> = interval1.overlap(&interval2) else {
									continue;
								};
								let next: NfaIdx =
									lookup_state(StatePair::new(self, other, target1, target2), &mut intersection);
								// println!("inserting interval {overlap:?}");
								combined.insert(overlap, next, PolicyUnique);
							}
						}
					}
					intersection[state].transitions = Transitions::Interval(combined);
				},
			}
		}

		intersection
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

		let mut finished: Vec<Vec<PathComponent>> = Vec::new();

		let mut stack: Vec<(&NfaState, Option<(SubRule, Vec<PathComponent>)>, Vec<PathComponent>)> =
			vec![(&self[NfaIdx::BEGIN], None, Vec::new())];

		while let Some((entry, mut maybe_capture, mut path)) = stack.pop() {
			let scc: &Vec<NfaIdx> = &sccs[data[entry.idx.0].scc];
			assert!(!scc.is_empty());
			if let Some(accepting_rule) = entry.maybe_accepts_for_rule {
				assert_eq!(scc.len(), 1);
				assert_eq!(entry.transitions.len(), 0);
				assert_eq!(maybe_capture, None);
				for part in path.iter_mut() {
					if let PathComponent::Capture { rule_idx, .. } = part {
						*rule_idx = accepting_rule;
					}
				}
				finished.push(path);
				continue;
			}
			if scc.len() == 1 {
				match &entry.transitions {
					Transitions::Interval(transitions) => {
						for (interval, &target) in transitions.iter() {
							assert!(data[target.0].index > data[entry.idx.0].index);
							assert!(data[target.0].scc > data[entry.idx.0].scc);
							let ch: PathComponent = if interval.start() == interval.end() {
								let ch: char = char::try_from(interval.start()).unwrap();
								PathComponent::Literal(ch.to_string())
							} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
								PathComponent::QueryWildcard
							} else {
								PathComponent::PatternWildcard
							};
							let mut path: Vec<PathComponent> = path.clone();
							if let Some((capture, mut capture_path)) = maybe_capture.clone() {
								capture_path.push(ch);
								stack.push((&self[target], Some((capture, capture_path)), path));
							} else {
								path.push(ch);
								stack.push((&self[target], None, path));
							}
						}
					},
					Transitions::Spontaneous(transitions) => {
						for transition in transitions.iter() {
							assert!(data[transition.target.0].scc > data[entry.idx.0].scc);
							let mut path: Vec<PathComponent> = path.clone();
							match &transition.kind {
								SpontaneousTransitionKind::Positive(Tag::StartCapture(sub_rule)) => {
									if sub_rule.is_leaf() {
										assert_eq!(maybe_capture, None);
										stack.push((
											&self[transition.target],
											Some((sub_rule.clone(), Vec::new())),
											path,
										));
										continue;
									}
								},
								SpontaneousTransitionKind::Positive(Tag::StopCapture(sub_rule)) => {
									if sub_rule.is_leaf() {
										let (current_capture, capture_path): (SubRule, Vec<PathComponent>) =
											maybe_capture.clone().unwrap();
										assert_eq!(&current_capture, sub_rule);
										path.push(PathComponent::Capture {
											rule_idx: RuleIdx::NIL,
											qualified_name: sub_rule.qualified_name.clone(),
											contents: capture_path,
										});
										stack.push((&self[transition.target], None, path));
										continue;
									}
								},
								SpontaneousTransitionKind::Negative(_) => (),
								SpontaneousTransitionKind::Epsilon => (),
							}
							stack.push((&self[transition.target], maybe_capture.clone(), path));
						}
					},
				}
			} else {
				if let Some((_capture, capture_path)) = &mut maybe_capture {
					capture_path.push(PathComponent::PatternWildcard);
				} else {
					path.push(PathComponent::PatternWildcard);
				};
				let mut finished: Vec<(NfaIdx, Option<(SubRule, Vec<PathComponent>)>, Vec<PathComponent>)> = Vec::new();
				{
					let mut stack: Vec<(
						&NfaState,
						Option<(SubRule, Vec<PathComponent>)>,
						Vec<PathComponent>,
						BTreeSet<NfaIdx>,
					)> = vec![(entry, maybe_capture, Vec::new(), BTreeSet::from([entry.idx]))];
					while let Some((state, maybe_capture, path, seen)) = stack.pop() {
						match &state.transitions {
							Transitions::Interval(transitions) => {
								for (interval, &target) in transitions.iter() {
									assert_eq!(data[target.0].scc, data[entry.idx.0].scc);
									let mut seen: BTreeSet<NfaIdx> = seen.clone();
									let inserted: bool = seen.insert(target);
									assert!(inserted);
									let ch: PathComponent = if interval.start() == interval.end() {
										let ch: char = char::try_from(interval.start()).unwrap();
										PathComponent::Literal(ch.to_string())
									} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
										PathComponent::QueryWildcard
									} else {
										PathComponent::PatternWildcard
									};
									let mut path: Vec<PathComponent> = path.clone();
									if let Some((capture, mut capture_path)) = maybe_capture.clone() {
										capture_path.push(ch);
										stack.push((&self[target], Some((capture, capture_path)), path, seen));
									} else {
										path.push(ch);
										stack.push((&self[target], None, path, seen));
									}
								}
							},
							Transitions::Spontaneous(transitions) => {
								for transition in transitions.iter() {
									assert!(data[transition.target.0].scc >= data[entry.idx.0].scc);
									if data[transition.target.0].scc != data[entry.idx.0].scc {
										finished.push((transition.target, maybe_capture.clone(), path.clone()));
										continue;
									}
									let mut seen: BTreeSet<NfaIdx> = seen.clone();
									if !seen.insert(transition.target) {
										continue;
									}
									let mut path: Vec<PathComponent> = path.clone();
									match &transition.kind {
										SpontaneousTransitionKind::Positive(Tag::StartCapture(sub_rule)) => {
											if sub_rule.is_leaf() {
												assert_eq!(maybe_capture, None);
												stack.push((
													&self[transition.target],
													Some((sub_rule.clone(), Vec::new())),
													path,
													seen,
												));
												continue;
											}
										},
										SpontaneousTransitionKind::Positive(Tag::StopCapture(sub_rule)) => {
											if sub_rule.is_leaf() {
												let (current_capture, capture_path): (SubRule, Vec<PathComponent>) =
													maybe_capture.clone().unwrap();
												assert_eq!(&current_capture, sub_rule);
												path.push(PathComponent::Capture {
													rule_idx: RuleIdx::NIL,
													qualified_name: sub_rule.qualified_name.clone(),
													contents: capture_path,
												});
												stack.push((&self[transition.target], None, path, seen));
												continue;
											}
										},
										SpontaneousTransitionKind::Negative(_) => (),
										SpontaneousTransitionKind::Epsilon => (),
									}
									stack.push((&self[transition.target], maybe_capture.clone(), path, seen));
								}
							},
						}
					}
				}

				for (exit, maybe_capture, scc_path) in finished.into_iter() {
					let mut path: Vec<PathComponent> = path.clone();
					let all_wildcards: bool = scc_path.iter().all(PathComponent::is_wildcard);
					// && maybe_capture
					// 	.as_ref()
					// 	.map_or(&[] as &[PathComponent], |(_, capture_path)| &capture_path[..])
					// 	.iter()
					// 	.all(PathComponent::is_wildcard);
					if let Some((capture, mut capture_path)) = maybe_capture {
						if all_wildcards {
							if capture_path.iter().all(PathComponent::is_wildcard) {
								capture_path.clear();
								// capture_path.push(PathComponent::Unknown);
							}
							// path.push(PathComponent::PatternWildcard);
						} else {
							path.extend(scc_path.into_iter());
						}
						// path.extend(scc_path.into_iter());
						capture_path.push(PathComponent::PatternWildcard);
						stack.push((&self[exit], Some((capture, capture_path)), path));
					} else {
						if !all_wildcards {
							path.extend(scc_path.into_iter());
						}
						path.push(PathComponent::PatternWildcard);
						stack.push((&self[exit], None, path));
					}
				}
			}
		}

		finished.into_iter().map(Path::new).collect::<Vec<_>>()
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
