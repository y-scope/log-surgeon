//! A parsed query, with its literal runs split out and its anchoring read off once.
//!
//! Three views of the same query appear downstream, and were previously re-derived
//! (and re-checked for agreement) at every use site:
//!
//! - the raw [`SymbolicChar`]s, exactly as parsed, which the prefilter walks and the engine's
//!   automaton consumes;
//! - the literal [`Run`]s between wildcards, which placement and composition reason about;
//! - the anchoring predicates. Runs alone cannot tell the empty query (anchored at both
//!   ends) from `*` (anchored nowhere): neither has any runs. Deriving the predicates once,
//!   from the symbols, is what keeps them from disagreeing with the runs.
//!
//! [`Query::new`] puts all three in one place, built once per query
//! and shared by every shape the query is searched against.

use crate::search::SearchString;
use crate::search::SymbolicChar;

/// A parsed query, as the decomposer sees it.
#[derive(Debug)]
pub struct Query<'a> {
	/// The symbols exactly as parsed; wildcards included.
	pub symbols: &'a [SymbolicChar],
	/// The maximal stretches of literal characters, with no wildcards.
	pub runs: Vec<Run>,
	/// Whether a match must begin at the start of the message (no leading wildcard).
	pub anchored_start: bool,
	/// Whether a match must run through to the end of the message (no trailing wildcard).
	pub anchored_end: bool,
}

impl Query<'_> {
	/// Builds the decomposer's view of `query`.
	#[must_use]
	pub fn new(query: &SearchString) -> Query<'_> {
		let symbols: &[SymbolicChar] = query.as_slice();
		let anchored_start: bool = query.anchored_start();
		let anchored_end: bool = query.anchored_end();
		let runs: Vec<Run> = runs_of(symbols, anchored_start, anchored_end);
		Query {
			symbols,
			runs,
			anchored_start,
			anchored_end,
		}
	}
}

/// A maximal stretch of literal characters from a query, with no wildcards.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Run {
	/// The literal text.
	pub text: String,
	/// Whether the run is pinned to the start of the message (no preceding wildcard).
	pub anchored_start: bool,
	/// Whether the run is pinned to the end of the message (no following wildcard).
	pub anchored_end: bool,
}

/// Splits a query's symbols into its literal runs.
///
/// Wildcards are separators and are not themselves runs;
/// `anchored_start`/`anchored_end` come from the query's boundaries,
/// not from the symbols, because the runs of a runless query cannot represent them.
fn runs_of(symbols: &[SymbolicChar], anchored_start: bool, anchored_end: bool) -> Vec<Run> {
	let mut runs: Vec<Run> = Vec::new();
	let mut current: String = String::new();
	let mut run_anchored_start: bool = anchored_start;

	for &symbol in symbols.iter() {
		match symbol {
			SymbolicChar::Literal(c) => current.push(c),
			SymbolicChar::GlobStar => {
				if !current.is_empty() {
					runs.push(Run {
						text: std::mem::take(&mut current),
						anchored_start: run_anchored_start,
						anchored_end: false,
					});
				}
				run_anchored_start = false;
			},
		}
	}

	if !current.is_empty() {
		runs.push(Run {
			text: current,
			anchored_start: run_anchored_start,
			anchored_end,
		});
	}

	runs
}
