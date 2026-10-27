//! Answers "can the rule(s) named `name` satisfy this piece of a query?" for one
//! `(name, piece, pinning)` triple, memoized across shapes.
//!
//! # Why this exists
//!
//! Searching a log shape by intersecting automata costs the size of the *whole shape*.
//! Real shapes are dominated by static text --
//! in the HDFS corpus, 403 shapes are 22KB of banner text wrapped around two small rules --
//! so that cost is paid on material that a plain substring search could handle.
//!
//! Placement instead asks the question *per rule*, per piece of the query:
//! can the rule begin with, end with, contain, or exactly match this piece?
//! That cost is the size of the rule (tens of states), not the shape (tens of thousands),
//! and the answer is **independent of the shape**,
//! so it is cached and reused by every shape that mentions the rule.
//! In the same corpus there are 14,501 rule references but only 111 distinct rule names.
//!
//! Each question is a search query against the rule: `*piece*`, `piece*`, `*piece`,
//! or `piece` -- so it is answered by
//! [`crate::search::SearchString::search_by_name`],
//! and the same four-pinning generalisation in [`Pinned`] is what every placement case asks.

#[cfg(test)]
mod test;

use crate::nfa::Tnfa;
use crate::parsing_spec::ParsingSpec;
use crate::regex::Regex;
use crate::search::SearchString;
use crate::search::SymbolicChar;
use crate::search::decompose::memo::Memo;

/// Which ends of the rule's match the piece pins.
///
/// A piece of a query pins an end it abuts:
/// the start when the query is anchored there or the preceding part runs
/// into this one, the end when the query is anchored there or the piece
/// straddles into what follows. A run wholly inside a rule pins neither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pinned {
	/// The rule's match must *begin* with the piece.
	pub start: bool,
	/// The rule's match must *end* with the piece.
	pub end: bool,
}

impl Pinned {
	/// Wraps the *literal* piece in the wildcards that express the pinning.
	///
	/// The piece is escaped, so it round-trips through
	/// [`SearchString::parse`] as literal text even when it contains `*` or `\`.
	pub fn wrap(&self, piece: &str) -> String {
		let mut query: String = String::with_capacity(piece.len() + 2);
		if !self.start {
			query.push('*');
		}
		for c in piece.chars() {
			if matches!(c, '*' | '\\') {
				query.push('\\');
			}
			query.push(c);
		}
		if !self.end {
			query.push('*');
		}
		query
	}

	/// Whether both ends are pinned, i.e. the rule must match `piece` exactly.
	pub fn exact(&self) -> bool {
		self.start && self.end
	}
}

/// Which side of a rendered value a padding wildcard sits on; see [`RunFitCache::has_slack`].
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Side {
	/// Text the rule emits before the value.
	Before,
	/// Text the rule emits after the value.
	After,
}

/// A memoized answer for one `(name, query)` pair, on the same cache as [`matches_piece`].
///
/// The query string is the cache key: escaped literals and wildcard-bearing
/// values cannot collide, because an escaped `\*` is never bare `*`.
#[derive(Clone, Debug, Default)]
pub struct RunFitCache {
	matched: Memo<(Box<str>, Box<str>), bool>,
	/// [`RunFitCache::has_slack`], keyed by `(name, value as rendered, side)`.
	slack: Memo<(Box<str>, Box<str>, Side), bool>,
}

impl RunFitCache {
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Whether the rule(s) named `name` can match `query`, which may contain wildcards.
	///
	/// Used where one rule reference holds pieces of *several* runs.
	/// Placement validates a run at a time,
	/// but a rule that admits each run separately need not admit them together:
	/// an alternation such as `INFO|WARN` matches either alone and neither pair.
	/// `query` is the value that will be reported for the capture,
	/// so this asks precisely the question the reported answer claims.
	#[must_use]
	pub fn matches_query(&self, spec: &ParsingSpec, name: &str, query: &str) -> bool {
		self.get(spec, name, query)
	}

	/// Whether the rule(s) named `name` can match `piece`, pinned as `pinned`.
	///
	/// `piece` is literal text; it is escaped before being wrapped.
	#[must_use]
	pub fn matches_piece(
		&self,
		spec: &ParsingSpec,
		name: &str,
		piece: &str,
		pinned: Pinned,
	) -> bool {
		self.get(spec, name, &pinned.wrap(piece))
	}

	/// Whether some match of the rule(s) named `name` is `value` with at least one more
	/// character on `side`: i.e. whether `L(rule)` meets `L(.+ value)` (or `L(value .+)`).
	///
	/// This decides whether a padding `*` on that side of a reported capture can be dropped.
	/// `L(*v) = L(v) | L(.+ v)`, so if the rule has no match in `L(.+ v)`,
	/// then `L(rule) & L(*v) = L(rule) & L(v)`, and reporting `v` loses nothing.
	/// The converse need not hold -- a match in `L(.+ v)` may also lie in `L(v)`
	/// (rule `aa`, `v = a*`) -- so a `true` keeps a `*` that was not strictly needed:
	/// conservative, never unsound.
	///
	/// It cannot be asked as a glob query through [`Self::matches_query`]:
	/// that decides only whether `L(rule) & L(*v)` is non-empty,
	/// which says nothing about whether the `*` is used.
	/// With rule `a|ba` and `v = a*`, `a*` is satisfiable, yet `ba` is a match `a*` would drop.
	/// So it is a direct intersection of the rule's automaton with `.+ v`,
	/// built from the same primitives the engine uses.
	///
	/// An unresolved name is answered `true`, keeping the padding.
	#[must_use]
	pub fn has_slack(
		&self,
		spec: &ParsingSpec,
		name: &str,
		value: &[SymbolicChar],
		side: Side,
	) -> bool {
		let rendered: String = String::from_iter(value.iter().map(ToString::to_string));
		let key: (Box<str>, Box<str>, Side) = (Box::from(name), Box::from(rendered), side);
		self.slack.get_or_insert_with(&key, || {
			let rows = spec.rules_for_name(name);
			if rows.is_empty() {
				return true;
			}

			let more: [Regex; 2] = [
				Regex::AnyChar,
				Regex::KleeneClosure(Box::new(Regex::AnyChar)),
			];
			let mut items: Vec<Regex> = Vec::with_capacity(value.len() + 2);
			if Side::Before == side {
				items.extend(more.iter().cloned());
			}
			items.extend(value.iter().map(SymbolicChar::to_regex));
			if Side::After == side {
				items.extend(more.iter().cloned());
			}
			let pattern: Tnfa = Tnfa::for_regex(&Regex::Sequence(items));

			rows.iter().any(|&(info, regex)| {
				Tnfa::for_single_rule(info.root_idx, regex, &[])
					.intersect::<false>(&pattern)
					.can_accept()
			})
		})
	}

	/// The memoized answer for `(name, query)`, computing on a miss.
	fn get(&self, spec: &ParsingSpec, name: &str, query: &str) -> bool {
		let key: (Box<str>, Box<str>) = (Box::from(name), Box::from(query));
		self.matched.get_or_insert_with(&key, || {
			SearchString::parse(query)
				.map(|parsed| !parsed.search_by_name(spec, name).is_empty())
				.unwrap_or(false)
		})
	}

	/// The number of cached `(name, query)` pairs.
	#[must_use]
	pub fn len(&self) -> usize {
		self.matched.len()
	}

	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.matched.is_empty() && self.slack.is_empty()
	}

	pub fn clear(&self) {
		self.matched.clear();
		self.slack.clear();
	}
}
