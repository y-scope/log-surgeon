//! Search results: an [`Interpretation`] is a sequence of [`LeafQuery`]s, one per shape part
//! or capture, plus the rules for rendering a tail ([`Tail`]) and for dropping
//! interpretations another covers ([`LeafQuery::covers`]).

use std::sync::Arc;

use crate::nfa::Path;
use crate::nfa::PathComponent;
use crate::search::SymbolicChar;
use crate::search::decompose::ShapePart;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Interpretation {
	pub leaf_queries: Vec<LeafQuery>,
}

impl std::fmt::Debug for Interpretation {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.leaf_queries
			.iter()
			.fold(&mut fmt.debug_list(), |list, query| list.entry(query))
			.finish()
	}
}

#[derive(Clone)]
pub struct LeafQuery {
	pub fully_qualified_name: Arc<str>,
	pub symbolic_value: Vec<SymbolicChar>,
	pub string_value: String,
}

impl std::fmt::Debug for LeafQuery {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if !self.is_static_text() {
			fmt.write_fmt(format_args!(
				"(?<{}>{:?})",
				self.fully_qualified_name, self.string_value,
			))
		} else {
			fmt.write_str(&self.string_value)
		}
	}
}

impl Eq for LeafQuery {}

impl Ord for LeafQuery {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		(&self.fully_qualified_name, &self.symbolic_value)
			.cmp(&(&other.fully_qualified_name, &other.symbolic_value))
	}
}

impl PartialOrd for LeafQuery {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialEq for LeafQuery {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other).is_eq()
	}
}

/// What the parts after the last rendered one consist of.
///
/// An unanchored query's trailing wildcard absorbs them,
/// but the *rendering* still depends on what they are:
/// static text must be reported (as a `'*'` on the preceding static sub-query,
/// or one of its own), while variables are unconstrained captures, which are omitted.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Tail {
	/// Nothing follows.
	Empty,
	/// Only rule references follow.
	OnlyVariables,
	/// Some static text follows.
	HasStatic,
}

impl Tail {
	/// Classifies `parts`, a slice of everything after the last rendered part.
	pub(crate) fn of_parts(parts: &[ShapePart]) -> Self {
		if parts.is_empty() {
			Self::Empty
		} else if parts.iter().all(ShapePart::is_variable) {
			Self::OnlyVariables
		} else {
			Self::HasStatic
		}
	}

	/// Classifies a suffix of *rendered* sub-queries, the ones the engine's path dropped.
	pub(super) fn of_leaves(leaf_queries: &[LeafQuery]) -> Self {
		if leaf_queries.is_empty() {
			Self::Empty
		} else if leaf_queries.iter().any(LeafQuery::is_static_text) {
			Self::HasStatic
		} else {
			Self::OnlyVariables
		}
	}

	/// Combines two tails over disjoint regions: static text in either dominates.
	pub(super) fn or(self, other: Self) -> Self {
		std::cmp::max(self, other)
	}

	/// Accounts for `tail` in a rendered interpretation covering parts up to it,
	/// for an unanchored query. One rule, shared by composition and the engine:
	///
	/// - nothing rendered at all -> the whole query was unconstrained: a single `'*'`;
	/// - the last sub-query is static -> it must permit *some*thing after it,
	///   whenever anything follows (its bare text would assert the message ends there);
	/// - the last sub-query is a capture -> a `'*'` sub-query is added,
	///   but only to stand for dropped *static text*;
	///   dropped variables are unconstrained captures, which are omitted.
	pub(crate) fn finish_unanchored(self, leaf_queries: &mut Vec<LeafQuery>) {
		if leaf_queries.is_empty() {
			leaf_queries.push(LeafQuery::new_static_text(vec![SymbolicChar::GlobStar]));
			return;
		}
		let last: &mut LeafQuery = leaf_queries.last_mut().expect("non-empty");
		if last.is_static_text() {
			if Self::Empty != self {
				last.append_wildcard();
			}
		} else if Self::HasStatic == self {
			leaf_queries.push(LeafQuery::new_static_text(vec![SymbolicChar::GlobStar]));
		}
	}
}

impl Interpretation {
	/// Whether every sub-query of `self` covers the corresponding one of `other`,
	/// so `other` describes nothing `self` does not already describe.
	///
	/// Positional, so it presumes both decompose the shape the same way;
	/// a difference in length means they are different decompositions and neither covers the other.
	pub(super) fn covers(&self, other: &Self) -> bool {
		(self.leaf_queries.len() == other.leaf_queries.len())
			&& std::iter::zip(self.leaf_queries.iter(), other.leaf_queries.iter())
				.all(|(mine, theirs)| mine.covers(theirs))
	}

	/// Drops interpretations that another already covers.
	///
	/// [`LeafQuery::covers`] is exact glob containment,
	/// so the result is a minimal antichain up to interpretations with equal languages.
	///
	/// Exact duplicates are removed first (by sorting, so callers need not).
	/// Order is not preserved: nothing downstream depends on it,
	/// and not preserving it keeps this a single pass over each candidate.
	pub(super) fn dedup_covered_interpretations(interpretations: &mut Vec<Self>) {
		interpretations.sort();
		interpretations.dedup();

		let mut kept: Vec<Self> = Vec::with_capacity(interpretations.len());

		for candidate in interpretations.drain(..) {
			if kept.iter().any(|keeper| keeper.covers(&candidate)) {
				continue;
			}
			// The candidate survives, so anything it covers is now redundant. `swap_remove` is used
			// rather than `retain` only because the order is already known not to matter.
			let mut index: usize = 0;
			while index < kept.len() {
				if candidate.covers(&kept[index]) {
					kept.swap_remove(index);
				} else {
					index += 1;
				}
			}
			kept.push(candidate);
		}

		*interpretations = kept;
	}

	pub(super) fn invariants(&self) {
		let mut last_was_static_text: bool = false;
		for query in self.leaf_queries.iter() {
			if query.is_static_text() {
				assert!(!last_was_static_text);
				last_was_static_text = true;
			} else {
				last_was_static_text = false;
			}

			// No value carries adjacent wildcards: `**` says exactly what `*` does,
			// so the canonical output never has it. Nothing depends on this for correctness
			// any more ([`LeafQuery::covers`] is exact containment, `**` included);
			// it is asserted to keep every producer emitting one canonical form.
			assert!(
				!query
					.symbolic_value
					.windows(2)
					.any(|pair| pair.iter().all(SymbolicChar::is_wildcard)),
				"adjacent wildcards in {:?}",
				query.string_value
			);
		}
	}
}

impl LeafQuery {
	pub(crate) fn new_static_text(symbolic_value: Vec<SymbolicChar>) -> Self {
		// `Arc::default()` special-cases ZSTs; no allocation needed.
		Self::new(Arc::<str>::default(), symbolic_value)
	}

	pub(crate) fn new_rule(
		fully_qualified_name: Arc<str>,
		symbolic_value: Vec<SymbolicChar>,
	) -> Self {
		assert!(!fully_qualified_name.is_empty());
		Self::new(fully_qualified_name, symbolic_value)
	}

	fn new(fully_qualified_name: Arc<str>, symbolic_value: Vec<SymbolicChar>) -> Self {
		let string_value: String =
			String::from_iter(symbolic_value.iter().map(SymbolicChar::to_string));
		Self {
			fully_qualified_name,
			symbolic_value,
			string_value,
		}
	}

	/// One sub-query per path component, in order.
	pub(super) fn from_path(path: &Path) -> Vec<Self> {
		assert!(!path.components.is_empty());
		Vec::from_iter(path.components.iter().map(|token| match token {
			PathComponent::Literal(contents) => Self::new_static_text(contents.clone()),
			PathComponent::Capture { capture, contents } => {
				Self::new_rule(capture.fully_qualified_name.clone(), contents.clone())
			},
		}))
	}

	pub fn is_static_text(&self) -> bool {
		self.fully_qualified_name.is_empty()
	}

	/// Whether `self` describes everything `other` does:
	/// `true` iff every string matching `other`'s value also matches `self`'s,
	/// and both name the same capture (or are both static text).
	///
	/// This is exact glob containment; see [`glob_covers`].
	/// It is therefore reflexive and transitive on any values.
	pub(super) fn covers(&self, other: &Self) -> bool {
		if self.fully_qualified_name != other.fully_qualified_name {
			return false;
		}
		glob_covers(&self.symbolic_value, &other.symbolic_value)
	}

	/// Appends a trailing wildcard, unless the value already ends in one.
	///
	/// Used where a value sits at the point the query's trailing wildcard applies,
	/// so it must permit anything after it.
	/// An empty value is left alone: it means the part is pinned to producing nothing,
	/// and padding it would claim the opposite.
	fn append_wildcard(&mut self) {
		if self.symbolic_value.is_empty()
			|| self
				.symbolic_value
				.last()
				.is_some_and(SymbolicChar::is_wildcard)
		{
			return;
		}
		self.symbolic_value.push(SymbolicChar::GlobStar);
		self.string_value.push('*');
	}

	pub(super) fn surround_with_wildcards(&mut self) {
		// An empty value means the rule is pinned to producing nothing;
		// padding it with wildcards would turn that into "anything",
		// which is the opposite claim.
		if self.symbolic_value.is_empty() {
			return;
		}
		if *self.symbolic_value.first().expect("non-empty") != SymbolicChar::GlobStar {
			self.symbolic_value.insert(0, SymbolicChar::GlobStar);
			self.string_value.insert(0, '*');
		}
		self.append_wildcard();
	}
}

/// Whether every string matching the glob `other` also matches the glob `pattern`.
///
/// For globs whose only wildcard is `*`, containment holds iff `pattern` matches the
/// string made from `other` by replacing each `*` with a fresh symbol that no literal
/// equals:
///
/// - if containment holds, that string is itself in `other`'s language;
/// - conversely, a literal never matches a fresh symbol, so each one is absorbed by
///   some `*` of `pattern`, which could equally absorb any string in its place.
///
/// So this is an ordinary glob match in which a `*` of `other` is a single token that
/// only a `*` of `pattern` can consume. `O(n x m)` time, `O(m)` space.
fn glob_covers(pattern: &[SymbolicChar], other: &[SymbolicChar]) -> bool {
	// `matches[j]`: whether `pattern[i..]` matches `other[j..]`, for the current `i`.
	let mut matches: Vec<bool> = vec![false; other.len() + 1];
	matches[other.len()] = true;
	for symbol in pattern.iter().rev() {
		match symbol {
			SymbolicChar::GlobStar => {
				// Absorb nothing (`matches[j]` as is) or `other[j]` and continue.
				for j in (0..other.len()).rev() {
					matches[j] = matches[j] || matches[j + 1];
				}
			},
			SymbolicChar::Literal(_) => {
				for j in 0..other.len() {
					matches[j] = (*symbol == other[j]) && matches[j + 1];
				}
				matches[other.len()] = false;
			},
		}
	}
	matches[0]
}
