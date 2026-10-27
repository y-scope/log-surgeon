//! Proves or disproves that a query could match a message of a given [`ShapeModel`].
//!
//! # The model
//!
//! A message of a given shape is produced by walking the shape's parts left to right and
//! emitting, for each part, either its static text verbatim or some string the part's rule can
//! match. A query matches when its symbols consume the *whole* of such a message.
//!
//! The shape is flattened into [`Atom`]s -- one per static character, one per variable --
//! so a position in the walk is just a pair of indices:
//! how much of the query has been consumed, and how much of the shape has been produced.
//!
//! Anchoring needs no special handling at either end.
//! A query that is not anchored at the start simply *begins* with a [`SymbolicChar::GlobStar`],
//! which is what lets the shape emit unconsumed text;
//! one that is not anchored at the end *ends* with one, which absorbs the rest of the message.
//! The query is taken exactly as written, wildcards included.
//!
//! # Soundness
//!
//! Variables are approximated by a [`crate::search::decompose::Charset`],
//! and are permitted to match the empty string
//! (the model does not know a rule's minimum length).
//! Both widen the set of accepted alignments,
//! so a `false` from [`can_match`] **proves** no message of the shape can match,
//! while `true` means only "maybe".

#[cfg(test)]
mod test;

use crate::search::SymbolicChar;
use crate::search::decompose::ShapeModel;
use crate::search::decompose::ShapePart;
use crate::search::decompose::Variable;

/// A shape flattened to one symbol-producing unit.
///
/// Flattening removes the need to track a character offset inside a static run,
/// and makes indexed access `O(1)` (indexing a `String` by character position is not).
#[derive(Clone, Copy, Debug)]
enum Atom<'a> {
	/// A single static character, which a match must reproduce verbatim.
	Char(char),
	/// A rule reference, standing for any string the rule can match.
	Variable(&'a Variable),
}

impl ShapeModel {
	/// Flattens this shape into [`Atom`]s.
	fn atoms(&self) -> Vec<Atom<'_>> {
		let mut atoms: Vec<Atom<'_>> = Vec::new();
		for part in self.parts.iter() {
			match part {
				ShapePart::Static(text) => atoms.extend(text.chars().map(Atom::Char)),
				ShapePart::Variable(variable) => atoms.push(Atom::Variable(variable)),
			}
		}
		atoms
	}
}

/// Whether any message of this shape could match the query.
///
/// Only reachability is solved: `false` **proves** no message of the shape can match,
/// while `true` means "not ruled out".
/// It allocates one bit per state and nothing per alignment,
/// so rejecting a shape is cheap.
///
/// `symbols` is the query exactly as parsed, trailing wildcard and all;
/// see [`crate::search::SearchString::parse`] for the canonical form this relies on.
#[must_use]
pub fn can_match(model: &ShapeModel, symbols: &[SymbolicChar]) -> bool {
	if is_obviously_not_ruled_out(model, symbols) {
		return true;
	}

	let atoms: Vec<Atom<'_>> = model.atoms();
	let solver: Solver<'_> = Solver::new(symbols, &atoms);

	solver.can_reach_acceptance()
}

/// A cheap *sufficient* condition for "not ruled out", used to skip the full walk.
///
/// If the query may float freely (it starts with a wildcard)
/// and some single variable's charset admits every literal the query contains,
/// then as far as this model knows that variable alone could emit the entire query text,
/// so the shape cannot be rejected.
///
/// When the query is anchored at the end the variable must additionally be able to be *last*,
/// since the query has to consume the message through to its end;
/// a variable with only nullable parts after it qualifies.
///
/// This only ever returns `true`, i.e. only ever causes a shape to be *kept*,
/// so it cannot make the prefilter unsound.
/// It matters because it is `O(variables + query)` against the walk's `O(atoms x query)`:
/// real log shapes are long (thousands of characters) and usually contain a permissive variable,
/// so this is the common case,
/// and paying for the full table there is what made the prefilter cost more than it saved.
fn is_obviously_not_ruled_out(model: &ShapeModel, symbols: &[SymbolicChar]) -> bool {
	if !symbols.first().is_some_and(SymbolicChar::is_wildcard) {
		return false;
	}
	let anchored_at_end: bool = !symbols.last().is_some_and(SymbolicChar::is_wildcard);

	model.parts.iter().enumerate().any(|(index, part)| {
		let ShapePart::Variable(variable) = part else {
			return false;
		};
		// The query must be able to finish here, or it could not reach the end of the message.
		if anchored_at_end && !model.can_end_at(index) {
			return false;
		}
		variable.charset.is_universal()
			|| symbols.iter().all(|symbol| match symbol {
				SymbolicChar::Literal(c) => variable.charset.contains(*c),
				SymbolicChar::GlobStar => true,
			})
	})
}

/// The alignment problem for one (query, shape) pair.
struct Solver<'a> {
	symbols: &'a [SymbolicChar],
	atoms: &'a [Atom<'a>],
	/// Index of the last static character in `atoms`, for [`Self::can_stop`].
	last_static: Option<usize>,
}

impl<'a> Solver<'a> {
	fn new(symbols: &'a [SymbolicChar], atoms: &'a [Atom<'a>]) -> Self {
		Self {
			symbols,
			atoms,
			last_static: atoms.iter().rposition(|atom| matches!(atom, Atom::Char(_))),
		}
	}

	/// The number of query cursor positions (`0..=len`).
	fn num_query_positions(&self) -> usize {
		self.symbols.len() + 1
	}

	/// Whether the shape, having produced `atom..`, can stop without emitting anything more.
	///
	/// Remaining static characters must be produced, so they block acceptance;
	/// a variable is conservatively assumed to be able to match the empty string.
	/// So this is just "no static character at or after `atom`",
	/// answered in `O(1)` from the precomputed last one.
	///
	/// A query ending in a wildcard never gets here with shape left over:
	/// the wildcard's transitions walk the remaining atoms first, consuming them,
	/// so anchoring at the end falls out of the query itself.
	fn can_stop(&self, atom: usize) -> bool {
		self.last_static.is_none_or(|last| last < atom)
	}

	/// Whether the query is fully consumed and the shape may stop here.
	fn is_accepting(&self, query: usize, atom: usize) -> bool {
		(query == self.symbols.len()) && self.can_stop(atom)
	}

	/// The longest block of query symbols, starting at `query`, that `variable` could emit.
	///
	/// A wildcard stands for arbitrary rule output, so it never bounds the block;
	/// a literal does unless the rule's charset admits it.
	fn max_capture_length(&self, variable: &Variable, query: usize) -> usize {
		let mut length: usize = 0;
		while let Some(&symbol) = self.symbols.get(query + length) {
			match symbol {
				SymbolicChar::Literal(c) => {
					if !variable.charset.contains(c) {
						break;
					}
				},
				SymbolicChar::GlobStar => (),
			}
			length += 1;
		}
		length
	}

	/// Whether the start state can reach acceptance.
	///
	/// Solved in one reverse sweep: every transition either advances the atom cursor,
	/// or advances the query cursor while leaving the atom cursor alone,
	/// so visiting atoms descending (and, within an atom, query positions descending)
	/// always visits successors first.
	/// Only the two atom columns actually needed are kept
	/// (the current one and its successor).
	fn can_reach_acceptance(&self) -> bool {
		let width: usize = self.num_query_positions();
		// `next` is the column for `atom + 1`; `current` is the column being filled.
		let mut next: Vec<bool> = vec![false; width];
		let mut current: Vec<bool> = vec![false; width];

		for atom in (0..=self.atoms.len()).rev() {
			for query in (0..width).rev() {
				current[query] = if self.is_accepting(query, atom) {
					true
				} else if query == self.symbols.len() {
					// Query consumed,
					// but the shape must still produce static text.
					false
				} else {
					let symbol: SymbolicChar = self.symbols[query];
					match self.atoms.get(atom) {
						// Past the end of the shape: only a wildcard can remain, matching nothing.
						None => (SymbolicChar::GlobStar == symbol) && current[query + 1],
						Some(Atom::Char(expected)) => match symbol {
							SymbolicChar::Literal(c) => (c == *expected) && next[query + 1],
							SymbolicChar::GlobStar => next[query] || current[query + 1],
						},
						Some(Atom::Variable(variable)) => {
							let longest: usize = self.max_capture_length(variable, query);
							(0..=longest).any(|length| next[query + length])
						},
					}
				};
			}

			if 0 == atom {
				return current[0];
			}
			std::mem::swap(&mut next, &mut current);
		}

		// Unreachable: the loop returns at `atom == 0`.
		false
	}
}
