use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::num::NonZero;
use std::sync::Arc;

use crate::interval_tree::Interval;
use crate::interval_tree::IntervalTree;
use crate::interval_tree::PolicyUnique;
use crate::nfa::CaptureTag;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::Tnfa;
use crate::nfa::Transitions;
use crate::parsing_spec::RuleIdx;
use crate::parsing_spec::SubRule;
use crate::search::SymbolicChar;
use crate::utils::TarjanSccs;

#[derive(Debug, Clone)]
pub struct Path {
	pub rule_idx: RuleIdx,
	pub components: Vec<PathComponent>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PathComponent {
	Literal(Vec<SymbolicChar>),
	Capture {
		maybe_sub_rule_id: Option<NonZero<u16>>,
		contents: Vec<SymbolicChar>,
	},
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum PathEdge {
	Literal(char),
	Capture {
		sub_rule_id: NonZero<u16>,
		/// Exists for debugging.
		qualified_name: Arc<str>,
		is_start: bool,
	},
	/// Search query allows for any character.
	/// TODO... explain
	QueryWildcard,
	/// Parsing spec rule pattern allows for any character.
	PatternWildcard,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct PartialPath {
	edges: Vec<PathEdge>,
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
		for part in self.components.iter() {
			part.fmt(fmt)?;
		}
		Ok(())
	}
}

impl PartialPath {
	fn new(edges: Vec<PathEdge>) -> Self {
		Self { edges }
	}

	fn push(&mut self, edge: PathEdge) {
		if edge.is_wildcard()
			&& let Some(last) = self.edges.last()
			&& last.is_wildcard()
		{
			return;
		}
		self.edges.push(edge);
	}

	fn iter(&self) -> std::slice::Iter<'_, PathEdge> {
		self.edges.iter()
	}
}

impl Extend<PathEdge> for PartialPath {
	fn extend<T>(&mut self, iter: T)
	where
		T: IntoIterator<Item = PathEdge>,
	{
		let iter: T::IntoIter = iter.into_iter();
		self.edges.reserve(match iter.size_hint() {
			(_, Some(n)) => n,
			(n, None) => n,
		});
		for edge in iter {
			self.push(edge);
		}
	}
}

impl Path {
	pub fn iter(&self) -> std::slice::Iter<'_, PathComponent> {
		self.components.iter()
	}

	pub fn len(&self) -> usize {
		self.components.len()
	}

	pub fn first(&self) -> Option<&PathComponent> {
		self.components.first()
	}

	pub fn last(&self) -> Option<&PathComponent> {
		self.components.last()
	}

	fn invariants(&self) {
		#[derive(Debug, Eq, PartialEq)]
		enum Kind {
			Start,
			Literal,
			Capture,
		}

		let mut last: Kind = Kind::Start;
		for component in self.components.iter() {
			last = match component {
				PathComponent::Literal(_) => {
					assert_ne!(last, Kind::Literal);
					Kind::Literal
				},
				PathComponent::Capture { .. } => Kind::Capture,
			};

			match component {
				PathComponent::Literal(contents) | PathComponent::Capture { contents, .. } => {
					let mut last_was_star: bool = false;
					for &ch in contents.iter() {
						if ch == SymbolicChar::GlobStar {
							assert!(!last_was_star);
							last_was_star = true;
						} else {
							last_was_star = false;
						}
					}
				},
			}
		}
	}
}

impl std::fmt::Display for PathComponent {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Literal(symbols) => {
				for &ch in symbols.iter() {
					match ch {
						SymbolicChar::Literal(ch) => {
							match ch {
								'(' | ')' | '*' | '\\' => {
									fmt.write_str("\\")?;
								},
								_ => (),
							}
							ch.fmt(fmt)?;
						},
						SymbolicChar::GlobStar => {
							'*'.fmt(fmt)?;
						},
						SymbolicChar::GlobOne => {
							unreachable!();
						},
					}
				}
			},
			Self::Capture {
				maybe_sub_rule_id,
				contents,
			} => {
				// TODO
				fmt.write_fmt(format_args!("(?<{maybe_sub_rule_id:?}>{contents:?})"))?;
			},
		}
		Ok(())
	}
}

impl PathEdge {
	fn is_wildcard(&self) -> bool {
		matches!(self, Self::QueryWildcard | Self::PatternWildcard)
	}
}

impl Tnfa {
	pub fn intersect<const FOR_SEARCH: bool>(&self, other: &Self) -> Self {
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
			tags: BTreeSet::new(),
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
				(_, Transitions::Tagged { .. }) => {
					panic!("the right hand side of `Tnfa::intersect` should not contain tags");
				},
				(Transitions::Tagged { tag, positive, target }, _) => {
					let next: NfaIdx =
						lookup_state(StatePair::new(self, other, *target, pair.states.1), &mut intersection);
					intersection[state].transitions = Transitions::Tagged {
						tag: tag.clone(),
						positive: *positive,
						target: next,
					};
				},
				(Transitions::Spontaneous(transitions1), Transitions::Spontaneous(transitions2)) => {
					if !transitions1.is_empty() {
						intersection[state].transitions = Transitions::Spontaneous(
							transitions1
								.iter()
								.map(|&target1| {
									let next: NfaIdx = lookup_state(
										StatePair::new(self, other, target1, pair.states.1),
										&mut intersection,
									);
									next
								})
								.collect::<Vec<_>>(),
						);
					} else {
						intersection[state].transitions = Transitions::Spontaneous(
							transitions2
								.iter()
								.map(|&target2| {
									let next: NfaIdx = lookup_state(
										StatePair::new(self, other, pair.states.0, target2),
										&mut intersection,
									);
									next
								})
								.collect::<Vec<_>>(),
						);
					}
				},
				(Transitions::Spontaneous(transitions1), _) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions1
							.iter()
							.map(|&target1| {
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, target1, pair.states.1),
									&mut intersection,
								);
								next
							})
							.collect::<Vec<_>>(),
					);
				},
				(_, Transitions::Spontaneous(transitions2)) => {
					intersection[state].transitions = Transitions::Spontaneous(
						transitions2
							.iter()
							.map(|&target2| {
								let next: NfaIdx = lookup_state(
									StatePair::new(self, other, pair.states.0, target2),
									&mut intersection,
								);
								next
							})
							.collect::<Vec<_>>(),
					);
				},
				(Transitions::Interval(transitions1), Transitions::Interval(transitions2)) => {
					let mut combined: IntervalTree<u32, NfaIdx> = IntervalTree::new();
					for (interval1, &target1) in transitions1.iter() {
						for (interval2, &target2) in transitions2.iter() {
							let Some(overlap): Option<Interval<u32>> = interval1.overlap(&interval2) else {
								continue;
							};
							if interval2.start() != interval2.end() {
								// Query wildcard.
								// TODO not true for arbitrary intersections - e.g. encodings
								// assert_eq!((interval2.start(), interval2.end()), (0, u32::from(char::MAX)));
								let next: NfaIdx =
									lookup_state(StatePair::new(self, other, target1, target2), &mut intersection);
								combined.insert(
									if FOR_SEARCH {
										Interval::new(0, u32::MAX)
									} else {
										overlap
									},
									next,
									PolicyUnique,
								);
							} else {
								// Query literal character.
								assert_eq!(interval2.start(), interval2.end());
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
					transitions.retain(|&target| can_accept[target.0]);
				},
				Transitions::Tagged { target, .. } => {
					assert_eq!(can_accept[state.idx.0], can_accept[target.0]);
				},
			}
		}

		intersection
	}

	pub fn can_accept(&self) -> bool {
		self.compute_live_states()[0]
	}

	/// States that can reach an accepting state.
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

	pub fn compute_paths<const WITH_ANCHORS: bool>(&self) -> Vec<Path> {
		let tarjan: TarjanSccs =
			TarjanSccs::tarjan_scc(&self.states, |state| state.transitions.successors().map(|idx| idx.0));

		let mut cache: Vec<Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>> = vec![None; self.states.len()];

		if self[NfaIdx::BEGIN].transitions.len() == 0 {
			return Vec::new();
		}

		let prefix: PartialPath = PartialPath::new(Vec::new());
		let mut seen_prefix: BTreeMap<NfaIdx, BTreeSet<PartialPath>> = BTreeMap::new();
		let mut finished: Vec<(PartialPath, RuleIdx)> = Vec::new();
		self.compute_paths_internal(
			&self[NfaIdx::BEGIN],
			&prefix,
			&mut seen_prefix,
			&tarjan,
			&mut cache,
			&mut finished,
		);
		let mut finished: Vec<Path> = Vec::new();
		let paths: &Vec<(NfaIdx, PartialPath, RuleIdx)> = cache[0].as_ref().unwrap();

		for (_, edges, rule_idx) in paths.iter() {
			let edges: &[PathEdge] = &edges.edges;

			let skip: usize = if WITH_ANCHORS {
				assert!(!edges.is_empty());
				assert_ne!(edges.len(), 2);

				1
			} else {
				0
			};

			let mut path: Vec<PathComponent> = Vec::new();
			let mut maybe_capture: Option<NonZero<u16>> = None;
			let mut symbols: Vec<SymbolicChar> = Vec::new();

			for edge in edges[skip..].iter().rev().skip(skip) {
				match edge {
					&PathEdge::Literal(ch) => {
						symbols.push(SymbolicChar::Literal(ch));
					},
					PathEdge::Capture {
						sub_rule_id, is_start, ..
					} => {
						if let Some(current_rule) = maybe_capture {
							// Closing capture.
							assert!(!is_start);
							assert_eq!(sub_rule_id, &current_rule);
							assert!(!symbols.is_empty());

							path.push(PathComponent::Capture {
								maybe_sub_rule_id: Some(*sub_rule_id),
								contents: symbols,
							});
							symbols = Vec::new();
							maybe_capture = None;
						} else {
							// Starting capture.
							assert!(is_start);

							if !symbols.is_empty() {
								if let Some(PathComponent::Literal(last)) = path.last_mut() {
									if (last.last() == Some(&SymbolicChar::GlobStar))
										&& (symbols.first() == Some(&SymbolicChar::GlobStar))
									{
										last.extend(symbols.drain(1..));
									} else {
										last.extend(symbols.into_iter());
									}
								} else {
									path.push(PathComponent::Literal(symbols));
								}
							}
							symbols = Vec::new();
							maybe_capture = Some(*sub_rule_id);
						}
					},
					PathEdge::QueryWildcard | PathEdge::PatternWildcard => {
						symbols.push(SymbolicChar::GlobStar);
					},
				}
			}
			assert_eq!(maybe_capture, None);

			if let Some(symbols_first) = symbols.first().copied() {
				if let Some(last) = path.last_mut() {
					match last {
						PathComponent::Literal(last) => {
							if symbols_first == SymbolicChar::GlobStar {
								assert_ne!(*last.last().unwrap(), SymbolicChar::GlobStar);
							}
							last.extend(symbols.into_iter());
						},
						PathComponent::Capture { .. } => {
							path.push(PathComponent::Literal(symbols));
						},
					}
				} else {
					assert_eq!(path, []);
					path.push(PathComponent::Literal(symbols));
				}
			}

			assert!(!path.is_empty());
			finished.push(Path {
				components: path,
				rule_idx: *rule_idx,
			});
		}
		finished.iter().for_each(Path::invariants);

		finished
	}

	fn compute_paths_internal(
		&self,
		entry: &NfaState,
		prefix: &PartialPath,
		seen_prefix: &mut BTreeMap<NfaIdx, BTreeSet<PartialPath>>,
		tarjan: &TarjanSccs,
		cache: &mut [Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>],
		finished: &mut Vec<(PartialPath, RuleIdx)>,
	) -> Vec<(NfaIdx, PartialPath, RuleIdx)> {
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
		}

		let mut paths: Vec<(NfaIdx, PartialPath, RuleIdx)> = Vec::new();

		let scc: &[usize] = &tarjan.sccs[tarjan.vertices[entry.idx.0].scc];
		assert!(!scc.is_empty());

		if let Some(rule) = entry.maybe_accepts_for_rule {
			finished.push((prefix.clone(), rule));
			return cache[entry.idx.0]
				.insert(vec![(entry.idx, PartialPath::new(Vec::new()), rule)])
				.clone();
		}
		if scc.len() == 1 {
			assert!(entry.transitions.len() > 0);

			match &entry.transitions {
				Transitions::Interval(transitions) => {
					assert_eq!(transitions.len(), 1);

					#[allow(clippy::never_loop)]
					for ((interval, &target), mut prefix) in transitions
						.iter()
						.zip(std::iter::repeat_n(prefix.clone(), transitions.len()))
					{
						assert!(tarjan.vertices[target.0].encountered_at > tarjan.vertices[entry.idx.0].encountered_at);
						assert!(tarjan.vertices[target.0].scc > tarjan.vertices[entry.idx.0].scc);

						let ch: PathEdge = if interval.start() == interval.end() {
							let ch: char = char::try_from(interval.start()).unwrap();
							PathEdge::Literal(ch)
						} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
							PathEdge::QueryWildcard
						} else {
							PathEdge::PatternWildcard
						};
						prefix.push(ch.clone());

						let mut partials: Vec<(NfaIdx, PartialPath, RuleIdx)> =
							self.compute_paths_internal(&self[target], &prefix, seen_prefix, tarjan, cache, finished);
						for (_state, path, _rule) in partials.iter_mut() {
							path.push(ch.clone());
						}
						partials.sort();
						partials.dedup();
						assert!(cache[entry.idx.0].is_none());
						return cache[entry.idx.0].insert(partials).clone();
					}
					unreachable!();
				},
				Transitions::Spontaneous(transitions) => {
					for &target in transitions.iter() {
						assert!(tarjan.vertices[target.0].scc > tarjan.vertices[entry.idx.0].scc);

						let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> =
							self.compute_paths_internal(&self[target], prefix, seen_prefix, tarjan, cache, finished);
						paths.extend(partials.into_iter());
					}
					paths.sort();
					paths.dedup();

					assert!(cache[entry.idx.0].is_none());
					cache[entry.idx.0].insert(paths).clone()
				},
				Transitions::Tagged { tag, positive, target } => {
					assert!(tarjan.vertices[target.0].scc > tarjan.vertices[entry.idx.0].scc);

					let sub_rule: &SubRule = tag.sub_rule();
					if *positive && sub_rule.is_leaf() {
						let is_start: bool = matches!(tag, CaptureTag::Start(_));
						let mut prefix: PartialPath = prefix.clone();
						let edge: PathEdge = PathEdge::Capture {
							sub_rule_id: sub_rule.id,
							qualified_name: sub_rule.qualified_name.clone(),
							is_start,
						};
						prefix.push(edge.clone());
						let mut partials: Vec<(NfaIdx, PartialPath, RuleIdx)> =
							self.compute_paths_internal(&self[*target], &prefix, seen_prefix, tarjan, cache, finished);
						for (_state, path, _rule) in partials.iter_mut() {
							path.push(edge.clone());
						}
						paths.extend(partials.into_iter());
					} else {
						let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> =
							self.compute_paths_internal(&self[*target], prefix, seen_prefix, tarjan, cache, finished);
						paths.extend(partials.into_iter());
					}

					paths.sort();
					paths.dedup();

					assert!(cache[entry.idx.0].is_none());
					cache[entry.idx.0].insert(paths).clone()
				},
			}
		} else {
			self.compute_scc_path(entry, prefix, seen_prefix, tarjan, cache, finished)
		}
	}

	fn compute_scc_path(
		&self,
		entry: &NfaState,
		prefix: &PartialPath,
		seen_prefix: &mut BTreeMap<NfaIdx, BTreeSet<PartialPath>>,
		tarjan: &TarjanSccs,
		cache: &mut [Option<Vec<(NfaIdx, PartialPath, RuleIdx)>>],
		finished: &mut Vec<(PartialPath, RuleIdx)>,
	) -> Vec<(NfaIdx, PartialPath, RuleIdx)> {
		let mut scc_finished: Vec<(NfaIdx, PartialPath, RuleIdx)> = Vec::new();

		let mut stack: Vec<(&NfaState, PartialPath, BTreeSet<NfaIdx>)> = vec![(
			entry,
			PartialPath::new(vec![PathEdge::PatternWildcard]),
			BTreeSet::from([entry.idx]),
		)];

		while let Some((state, path, seen)) = stack.pop() {
			assert!(state.transitions.len() > 0);

			match &state.transitions {
				Transitions::Interval(transitions) => {
					assert_eq!(transitions.len(), 1);

					for ((interval, &target), (mut path, mut seen)) in transitions
						.iter()
						.zip(std::iter::repeat_n((path, seen), transitions.len()))
					{
						assert_eq!(tarjan.vertices[target.0].scc, tarjan.vertices[entry.idx.0].scc);

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
					for (&target, (path, mut seen)) in transitions
						.iter()
						.zip(std::iter::repeat_n((path, seen), transitions.len()))
					{
						assert!(tarjan.vertices[target.0].scc >= tarjan.vertices[entry.idx.0].scc);

						if tarjan.vertices[target.0].scc != tarjan.vertices[entry.idx.0].scc {
							let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> = self
								.compute_paths_internal(&self[target], prefix, seen_prefix, tarjan, cache, finished);
							for (end, mut remaining, rule) in partials.into_iter() {
								remaining.push(PathEdge::PatternWildcard);
								remaining.extend(path.iter().rev().cloned());
								scc_finished.push((end, remaining, rule));
							}
							continue;
						}

						if seen.contains(&target) {
							continue;
						}

						let inserted: bool = seen.insert(target);
						assert!(inserted);

						stack.push((&self[target], path, seen));
					}
				},
				Transitions::Tagged { tag, positive, target } => {
					assert!(tarjan.vertices[target.0].scc >= tarjan.vertices[entry.idx.0].scc);

					if tarjan.vertices[target.0].scc != tarjan.vertices[entry.idx.0].scc {
						let partials: Vec<(NfaIdx, PartialPath, RuleIdx)> =
							self.compute_paths_internal(&self[*target], prefix, seen_prefix, tarjan, cache, finished);
						for (end, mut remaining, rule) in partials.into_iter() {
							remaining.push(PathEdge::PatternWildcard);
							remaining.extend(path.iter().rev().cloned());
							scc_finished.push((end, remaining, rule));
						}
						continue;
					}

					if seen.contains(&target) {
						continue;
					}

					let mut path: PartialPath = path;
					let mut seen: BTreeSet<NfaIdx> = seen;

					let inserted: bool = seen.insert(*target);
					assert!(inserted);

					if *positive {
						let sub_rule: &SubRule = tag.sub_rule();
						if sub_rule.is_leaf() {
							path.push(PathEdge::Capture {
								sub_rule_id: sub_rule.id,
								qualified_name: sub_rule.qualified_name.clone(),
								is_start: matches!(tag, CaptureTag::Start(_)),
							});
							stack.push((&self[*target], path, seen));
							continue;
						}
					}
					stack.push((&self[*target], path, seen));
				},
			}
		}

		scc_finished.sort();
		scc_finished.dedup();

		assert!(cache[entry.idx.0].is_none());
		cache[entry.idx.0].insert(scc_finished).clone()
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::regex::Regex;

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
		let paths = nfa.intersect::<true>(&search).compute_paths::<false>();
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
		let regex: Regex = Regex::from_pattern(pattern).unwrap().regex;
		Tnfa::from_regex(&regex)
	}
}
