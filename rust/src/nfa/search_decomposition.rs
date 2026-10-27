use std::collections::BTreeSet;

use rustc_hash::FxHashSet;

use crate::graph::TarjanSccs;
use crate::nfa::NfaIdx;
use crate::nfa::NfaState;
use crate::nfa::Tnfa;
use crate::nfa::Transitions;
use crate::parsing_spec::CaptureRef;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::ResolvedCapture;
use crate::parsing_spec::RuleIdx;
use crate::search::SymbolicChar;

const PATH_TIMEOUT_MILLIS: u128 = 2 * 1000;

#[derive(Debug, Clone)]
pub struct Path {
	pub rule_idx: RuleIdx,
	pub components: Vec<PathComponent>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PathComponent {
	Literal(Vec<SymbolicChar>),
	Capture {
		capture: ResolvedCapture,
		contents: Vec<SymbolicChar>,
	},
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum PathEdge {
	Literal(char),
	Capture {
		capture: ResolvedCapture,
		is_open: bool,
	},
	Wildcard,
}

impl std::hash::Hash for PathEdge {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		std::mem::discriminant(self).hash(state);
		match self {
			Self::Literal(ch) => {
				ch.hash(state);
			},
			Self::Capture { capture, is_open } => {
				capture.hash(state);
				is_open.hash(state);
			},
			Self::Wildcard => (),
		}
	}
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
struct PartialPath {
	edges: Vec<PathEdge>,
}

impl std::fmt::Display for PathEdge {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Literal(ch) => {
				if matches!(ch, '(' | ')' | '*' | '\\') {
					fmt.write_str("\\")?;
				}
				ch.fmt(fmt)
			},
			Self::Capture { capture, is_open } => {
				if *is_open {
					fmt.write_fmt(format_args!("(?<{}>", capture.fully_qualified_name))
				} else {
					fmt.write_str(")")
				}
			},
			Self::Wildcard => '*'.fmt(fmt),
		}
	}
}

impl std::fmt::Display for PartialPath {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for edge in self.edges.iter() {
			edge.fmt(fmt)?;
		}
		Ok(())
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

	fn condense(&mut self) {
		let mut i: usize = 1;
		while i < self.edges.len() {
			let previous: &PathEdge = &self.edges[i - 1];
			let current: &PathEdge = &self.edges[i];
			if previous.is_wildcard() && current.is_wildcard() {
				self.edges.remove(i);
			} else {
				i += 1;
			}
		}
	}

	/// Renders this path's edges as [`PathComponent`]s.
	///
	/// The path runs to an accepting state of the intersection,
	/// which means it has consumed the whole shape as well as the whole search;
	/// nothing is appended or assumed.
	/// A capture is therefore always closed by an explicit closing tag,
	/// and its contents are exactly what the rule produced
	/// -- possibly nothing, when the search pins it to the empty string.
	fn finish(self, rule_idx: RuleIdx) -> Path {
		let mut components: Vec<PathComponent> = Vec::new();
		let mut maybe_active_capture: Option<ResolvedCapture> = None;
		let mut symbols: Vec<SymbolicChar> = Vec::new();

		for edge in self.edges.iter() {
			match edge {
				&PathEdge::Literal(ch) => {
					symbols.push(SymbolicChar::Literal(ch));
				},
				PathEdge::Capture { capture, is_open } => {
					if let Some(active_capture) = maybe_active_capture {
						// Closing capture.
						assert!(!is_open);
						assert_eq!(capture, &active_capture);
						// `symbols` may be empty: a rule such as `(?<leaf>[a-z]*)` can match nothing,
						// and an end-anchored query pins it to exactly that.
						// The empty capture is the precise answer
						// -- it says the rule produced no text --
						// so it is reported as-is rather than widened to `*`,
						// which would claim the opposite.

						components.push(PathComponent::Capture {
							capture: active_capture,
							contents: symbols,
						});
						symbols = Vec::new();
						maybe_active_capture = None;
					} else {
						// Opening capture.
						assert!(is_open);

						if !symbols.is_empty() {
							assert!(!matches!(
								components.last(),
								Some(PathComponent::Literal(_))
							));
							components.push(PathComponent::Literal(symbols));
						}
						symbols = Vec::new();
						maybe_active_capture = Some(capture.clone());
					}
				},
				PathEdge::Wildcard => {
					symbols.push(SymbolicChar::GlobStar);
				},
			}
		}
		// Every capture opened on this path was closed: the path reaches an accepting state,
		// so the automaton walked past each closing tag.
		// A capture left open would mean the path stopped mid-rule,
		// which no longer happens now that the intersection always runs to the end.
		assert!(
			maybe_active_capture.is_none(),
			"path ended inside a capture: {maybe_active_capture:?}"
		);
		if !symbols.is_empty() {
			assert!(!matches!(
				components.last(),
				Some(PathComponent::Literal(_))
			));
			components.push(PathComponent::Literal(symbols));
		}

		Path {
			rule_idx,
			components,
		}
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
							if matches!(ch, '(' | ')' | '*' | '\\') {
								fmt.write_str("\\")?;
							}
							ch.fmt(fmt)?;
						},
						SymbolicChar::GlobStar => {
							'*'.fmt(fmt)?;
						},
					}
				}
			},
			Self::Capture { capture, contents } => {
				// TODO
				fmt.write_fmt(format_args!(
					"(?<{:?}>{:?})",
					capture.fully_qualified_name, contents
				))?;
			},
		}
		Ok(())
	}
}

impl PathEdge {
	fn is_wildcard(&self) -> bool {
		*self == Self::Wildcard
	}
}

impl Tnfa {
	pub fn compute_paths(&self, spec: &ParsingSpec) -> Vec<Path> {
		let tarjan: TarjanSccs = self.sccs();

		trace!(
			"have {} states, have {} sccs",
			self.states.len(),
			tarjan.sccs.len()
		);

		if self[NfaIdx::BEGIN].transitions.len() == 0 {
			return Vec::new();
		}

		now!(t0);
		trace!("paths for each state...");
		let mut cache: Vec<Option<Vec<(PartialPath, NfaIdx)>>> = vec![None; self.states.len()];
		{
			let mut seen: Vec<bool> = vec![false; self.states.len()];
			seen[NfaIdx::BEGIN.0] = true;
			let mut stack: Vec<&NfaState> = vec![&self[NfaIdx::BEGIN]];
			while let Some(state) = stack.pop() {
				let paths: &[(PartialPath, NfaIdx)] =
					self.compute_path_for_vertex(state, spec, &tarjan, &mut cache);
				for (_path, next) in paths.iter() {
					if !seen[next.0] {
						seen[next.0] = true;
						stack.push(&self[*next]);
					}
				}
			}
		}
		now!(t1);
		trace!(
			"done paths for each state {}.",
			t1.duration_since(t0).as_millis()
		);

		now!(t2);
		trace!("paths...");
		let mut finished: Vec<Path> = Vec::new();
		// TODO no ::new?
		let mut seen_prefixes: FxHashSet<(PartialPath, NfaIdx)> = FxHashSet::default();
		{
			now!(t_start);
			let mut stack: Vec<(PartialPath, &NfaState)> =
				vec![(PartialPath::new(Vec::new()), &self[NfaIdx::BEGIN])];
			while let Some((prefix_path, current)) = stack.pop() {
				now!(t_current);

				if millis!(t_start, t_current) > PATH_TIMEOUT_MILLIS {
					eprintln!("timed out computing paths, potentially exponential input");
					todo!();
				}

				if let Some((rule_idx, _)) = current.maybe_accepting_data {
					finished.push(prefix_path.finish(rule_idx));
					continue;
				}
				for (next_path, next_idx) in cache[current.idx.0].as_ref().unwrap().iter() {
					let mut path: PartialPath = PartialPath::new(Vec::from_iter(
						prefix_path.iter().cloned().chain(next_path.iter().cloned()),
					));
					path.condense();
					if seen_prefixes.insert((path.clone(), *next_idx)) {
						stack.push((path, &self[*next_idx]));
					}
				}
			}
		}
		now!(t3);
		trace!("done {}.", t3.duration_since(t2).as_millis());

		finished.iter().for_each(Path::invariants);

		finished
	}

	fn compute_path_for_vertex<'a>(
		&self,
		entry: &NfaState,
		spec: &ParsingSpec,
		tarjan: &TarjanSccs,
		cache: &'a mut Vec<Option<Vec<(PartialPath, NfaIdx)>>>,
	) -> &'a [(PartialPath, NfaIdx)] {
		// Borrow checker chokes if we do `if let` here...
		if cache[entry.idx.0].is_some() {
			return cache[entry.idx.0].as_ref().unwrap();
		}

		let paths: &mut Vec<(PartialPath, NfaIdx)> = cache[entry.idx.0].insert(Vec::new());

		let scc: &[usize] = &tarjan.sccs[tarjan.vertices[entry.idx.0].scc];
		assert!(!scc.is_empty());

		if entry.is_accepting() {
			return paths;
		}

		if scc.len() == 1 {
			// Handled by reachability filter and early return for accepting states.
			assert!(entry.transitions.len() > 0);

			match &entry.transitions {
				Transitions::Interval(transitions) => {
					assert_eq!(transitions.len(), 1);

					#[allow(clippy::never_loop)]
					for (interval, &target) in transitions.iter() {
						assert!(
							tarjan.vertices[target.0].encountered_at
								> tarjan.vertices[entry.idx.0].encountered_at
						);
						assert!(tarjan.vertices[target.0].scc > tarjan.vertices[entry.idx.0].scc);

						let ch: PathEdge = if interval.start() == interval.end() {
							let ch: char = char::try_from(interval.start()).unwrap();
							PathEdge::Literal(ch)
						} else {
							PathEdge::Wildcard
						};
						paths.push((PartialPath::new(vec![ch]), target));
						break;
					}
				},
				Transitions::Spontaneous(transitions) => {
					for &target in transitions.iter() {
						assert!(tarjan.vertices[target.0].scc > tarjan.vertices[entry.idx.0].scc);

						paths.push((PartialPath::new(Vec::new()), target));
					}
				},
				Transitions::Tagged {
					tag,
					positive,
					target,
				} => {
					assert!(tarjan.vertices[target.0].scc > tarjan.vertices[entry.idx.0].scc);

					let maybe_capture: Option<ResolvedCapture> = if *positive {
						spec.resolve_capture(CaptureRef {
							rule_idx: tag.rule_idx,
							capture_id: tag.capture_id,
						})
						.filter(|(info, _)| info.is_leaf())
						.map(|(_, fully_qualified_name)| ResolvedCapture {
							capture: CaptureRef {
								rule_idx: tag.rule_idx,
								capture_id: tag.capture_id,
							},
							fully_qualified_name,
						})
					} else {
						None
					};

					if let Some(capture) = maybe_capture {
						let edge: PathEdge = PathEdge::Capture {
							capture,
							is_open: !tag.is_close,
						};
						paths.push((PartialPath::new(vec![edge]), *target));
					} else {
						paths.push((PartialPath::new(Vec::new()), *target));
					}
				},
			}
		} else {
			self.compute_scc_path(entry, spec, tarjan, paths);
			for (path, _next) in paths.iter_mut() {
				path.condense();
			}
		}
		paths.sort();
		paths.dedup();
		paths
	}

	fn compute_scc_path(
		&self,
		entry: &NfaState,
		spec: &ParsingSpec,
		tarjan: &TarjanSccs,
		finished: &mut Vec<(PartialPath, NfaIdx)>,
	) {
		// 2026-09-15: no noticeable improvement replacing `BTreeSet` with `FxHashSet` here.
		let mut stack: Vec<(&NfaState, PartialPath, BTreeSet<NfaIdx>)> = vec![(
			entry,
			PartialPath::new(vec![PathEdge::Wildcard]),
			BTreeSet::from([entry.idx]),
		)];

		while let Some((state, path, seen)) = stack.pop() {
			assert!(state.transitions.len() > 0);

			match &state.transitions {
				Transitions::Interval(transitions) => {
					// For search, the 2nd NFA (and consequently the intersection)
					// only ever has 1 interval for symbol transitions;
					// either a literal character or a wildcard.
					assert_eq!(transitions.len(), 1);

					// Because `transitions.len() == 1`,
					// this for-loop is unnecessary/could be simplified,
					// but it's written out regardless since the core idea
					// doesn't need to rely on the fact above
					// (and it mirrors the other cases below).
					for ((interval, &target), (mut path, mut seen)) in transitions
						.iter()
						.zip(std::iter::repeat_n((path, seen), transitions.len()))
					{
						assert_eq!(
							tarjan.vertices[target.0].scc,
							tarjan.vertices[entry.idx.0].scc
						);

						let inserted: bool = seen.insert(target);
						assert!(inserted);

						let ch: PathEdge = if interval.start() == interval.end() {
							let ch: char = char::try_from(interval.start()).unwrap();
							PathEdge::Literal(ch)
						} else if (interval.start() == 0) && (interval.end() == u32::MAX) {
							PathEdge::Wildcard
						} else {
							PathEdge::Wildcard
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
							finished.push((path, target));
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
				Transitions::Tagged {
					tag,
					positive,
					target,
				} => {
					assert_eq!(
						tarjan.vertices[target.0].scc,
						tarjan.vertices[entry.idx.0].scc
					);

					if seen.contains(target) {
						continue;
					}

					let mut path: PartialPath = path;
					let mut seen: BTreeSet<NfaIdx> = seen;

					let inserted: bool = seen.insert(*target);
					assert!(inserted);

					if *positive {
						let capture_ref: CaptureRef = CaptureRef {
							rule_idx: tag.rule_idx,
							capture_id: tag.capture_id,
						};
						if let Some((_, fully_qualified_name)) = spec
							.resolve_capture(capture_ref)
							.filter(|(info, _)| info.is_leaf())
						{
							path.push(PathEdge::Capture {
								capture: ResolvedCapture {
									capture: capture_ref,
									fully_qualified_name,
								},
								is_open: !tag.is_close,
							});
							stack.push((&self[*target], path, seen));
							continue;
						}
					}
					stack.push((&self[*target], path, seen));
				},
			}
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::parsing_spec::ParsingSpec;
	use crate::parsing_spec::ParsingSpecBuilder;
	use crate::regex::Regex;

	#[test]
	fn nfa_decomp() {
		let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
		builder
			.add_rule("rule", r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)")
			.unwrap();
		let spec: ParsingSpec = builder.build();

		let rule: &crate::parsing_spec::RootRule = &spec.rules[0];
		let nfa: Tnfa = Tnfa::for_single_rule(rule.idx, &rule.regex.regex, &[]);
		let search: Tnfa = nfa_for("abc@.*mail.*example.*");

		let paths = nfa.intersect::<true>(&search).compute_paths(&spec);
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
		let regex: Regex = Regex::from_pattern(pattern).unwrap();
		Tnfa::for_regex(&regex)
	}
}
