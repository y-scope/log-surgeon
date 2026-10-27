//! Places a query's literal runs into a log shape, and composes those placements into
//! decompositions.
//!
//! # The two stages
//!
//! **Placement** (per run, per position): where can this run go? A run must be produced in full,
//! and there are only three possibilities -- inside one rule, inside the shape's static text,
//! or straddling a boundary between a rule and what sits beside it. Each possibility is decided
//! through a cached [`matches_piece`][crate::search::decompose::RunFitCache::matches_piece] call
//! (rule side) or a substring search (static-text side), so the cost
//! is independent of how much static text the shape contains.
//! Rows are computed left to right: a run's placements in a static part are relative
//! to where the previous run could leave off there,
//! which is the invariant [`PlacementTable::compute`] documents.
//!
//! If any run has no placement anywhere in the shape, the shape **cannot** match and is rejected
//! immediately, without touching an automaton.
//!
//! **Composition**: choose one placement per run such that the placements occur left to right
//! and do not overlap. This is a reachability problem over `(run index, shape position)` --
//! every placement advances both -- so it is solved by the same kind of reverse DP sweep as
//! [`crate::search::decompose::prefilter`], in `O(runs x positions x placements)`, rather than
//! by enumerating choices.
//!
//! # Positional identity
//!
//! A decomposition records the **index of the shape part** that produced each piece, not just
//! the rule name. Two references to the same rule in one shape (`A%foo%B%foo%C`) are therefore
//! distinguishable, which the engine's own output is not: it reports both as `foo` with the
//! same group, leaving the instance to be inferred from position among vacuous captures.

#[cfg(test)]
mod test;

use std::collections::BTreeMap;

use tracing::info;

use crate::parsing_spec::ParsingSpec;
use crate::search::decompose::Pinned;
use crate::search::decompose::Query;
use crate::search::decompose::Run;
use crate::search::decompose::RunFitCache;
use crate::search::decompose::ShapeModel;
use crate::search::decompose::ShapePart;

/// Where one run sits in a shape.
///
/// Positions are indices into [`ShapeModel::parts`]. A placement spans `start_part..=end_part`;
/// the common cases are a single part, or a rule plus one neighbour.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Placement {
	/// The first shape part this run touches.
	pub start_part: usize,
	/// The last shape part this run touches.
	pub end_part: usize,
	/// Where in `start_part`'s static text this run begins, in characters.
	///
	/// Zero for a run beginning inside a rule, where an offset is meaningless:
	/// a rule's output is not fixed, so there is no position within it to speak of.
	pub start_offset: usize,
	/// Where in `end_part`'s static text this run ends, in characters,
	/// counted just past the last character consumed. Zero when the run ends inside a rule.
	///
	/// This is what lets several runs share one static part: `a*b*c` against the text `abc`
	/// places three runs in the same part, distinguished only by their offsets.
	pub end_offset: usize,
	/// How the run is divided among the parts it touches.
	pub pieces: Vec<Piece>,
}

/// One part's contribution to a run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Piece {
	/// Index into [`ShapeModel::parts`].
	pub part: usize,
	/// Where in `part`'s static text this piece begins, in characters.
	///
	/// Zero and meaningless when `is_rule`:
	/// a rule's output is not fixed, so there is no position within it.
	/// For static text this is what tells rendering whether text of the part remains *before*
	/// the piece, and so whether the reported value must be free to skip it.
	pub offset: usize,
	/// The characters of the run this part supplies.
	pub text: String,
	/// Whether `part` is a rule (so this is a capture) rather than static text.
	pub is_rule: bool,
}

impl Placement {
	/// A run produced wholly by one part.
	fn within_part(
		part: usize,
		start_offset: usize,
		end_offset: usize,
		text: String,
		is_rule: bool,
	) -> Self {
		Self {
			start_part: part,
			end_part: part,
			start_offset,
			end_offset,
			pieces: vec![Piece {
				part,
				offset: start_offset,
				text,
				is_rule,
			}],
		}
	}

	/// Where this run begins.
	#[must_use]
	pub fn start(&self) -> Position {
		(self.start_part, self.start_offset)
	}

	/// The position a following run must start at or after.
	///
	/// A run that ends inside a rule leaves that rule available to a later run
	/// (a rule can produce more text after the run), and so does a run ending part-way through
	/// static text -- the rest of that text is still to come.
	/// The two differ only in the offset: it is meaningful only where the placement ends in
	/// static text, since a rule may emit arbitrarily much more text before the next run begins.
	#[must_use]
	pub fn next_available(&self) -> Position {
		let offset: usize = if self.ends_in_rule() {
			0
		} else {
			self.end_offset
		};
		(self.end_part, offset)
	}

	/// Whether this placement ends inside a rule, leaving no fixed position within the part.
	#[must_use]
	pub fn ends_in_rule(&self) -> bool {
		self.pieces.last().is_some_and(|piece| piece.is_rule)
	}
}

/// The placements available to each run of a query, for one shape.
#[derive(Clone, Debug)]
pub struct PlacementTable {
	/// `placements[i]` are the ways run `i` can sit in the shape.
	pub placements: Vec<Vec<Placement>>,
}

/// Cap on placements recorded per run.
///
/// A shape with many rule instances can offer very many placements for one run.
/// Exceeding this means the table is incomplete, so no conclusion may be drawn;
/// [`PlacementTable::compute`] returns `None` and the caller falls back to the engine.
/// A cap is necessary because placement enumeration is not polynomial in general:
/// a chain of back-to-back rules can split a run in exponentially many ways.
const MAX_PLACEMENTS_PER_RUN: usize = 2_048;

impl PlacementTable {
	/// Whether some run cannot be placed anywhere, proving the shape cannot match.
	#[must_use]
	pub fn is_impossible(&self) -> bool {
		self.placements.iter().any(Vec::is_empty)
	}

	/// The last shape part the query's literal text can reach.
	///
	/// This is the max over the placements of the **last** run alone, not over every run:
	/// the runs of a composition are laid down left to right,
	/// so whatever the last run ends at bounds the whole query.
	/// Taking the union over all runs would be needlessly pessimistic,
	/// since an early run that *could* sit late in the shape never does
	/// in a composition that also places the runs after it.
	///
	/// Everything past this part is covered by the query's trailing wildcard,
	/// so it constrains nothing and is dropped from the reported interpretation anyway.
	/// That makes it exactly the point at which the shape's automaton can be cut off; see
	/// [`crate::search::SearchString::search_by_log_shapes`].
	///
	/// Returns `None` when the query has no runs (all wildcards, so nothing is constrained)
	/// or the last run has no placement at all (the shape cannot match).
	#[must_use]
	pub fn last_reachable_part(&self) -> Option<usize> {
		let last_run: &Vec<Placement> = self.placements.last()?;
		last_run.iter().map(|placement| placement.end_part).max()
	}

	/// Computes the placements for every run of the query against `model`.
	///
	/// Rows are computed left to right, and row `i` depends on row `i-1`:
	/// the placements of a run in static text are recorded relative to where the
	/// previous run can leave off in the same part. A row is therefore complete
	/// *relative to its predecessor's endings*, not a standalone "everywhere the
	/// run can sit" -- `Reachability`, `compose`, and `last_reachable_part` all
	/// consume rows in order and never revisit an earlier one.
	/// That restriction is what keeps a run in a banner of repeated characters
	/// (say `*a*a*` against `aaaa`) polynomial instead of one placement per offset.
	///
	/// Returns `None` if the table would be **incomplete** because a resource cap was exceeded --
	/// either too many placements for one run, or too long a run between back-to-back rules.
	/// The caller must then not draw a conclusion and should fall back to the engine.
	/// An empty placement list for a run is a real answer: the shape cannot match.
	#[must_use]
	pub fn compute(
		spec: &ParsingSpec,
		model: &ShapeModel,
		query: &Query<'_>,
		fits: &RunFitCache,
	) -> Option<Self> {
		let runs: &[Run] = &query.runs;
		let mut placements: Vec<Vec<Placement>> = Vec::with_capacity(runs.len());
		// Where the previous run can leave off, per static part: built once per run,
		// rather than rescanning the previous row per part.
		let mut previous_exits: BTreeMap<usize, Vec<usize>> = BTreeMap::new();

		for (run_index, run) in runs.iter().enumerate() {
			let mut for_run: Vec<Placement> = Vec::new();

			for (index, part) in model.parts.iter().enumerate() {
				match part {
					ShapePart::Static(text) => {
						// The run must appear verbatim in this static text.
						for start_offset in usable_occurrences(
							text,
							&run.text,
							previous_exits.get(&index).map(Vec::as_slice).unwrap_or(&[]),
						) {
							let end_offset: usize = start_offset + run.text.chars().count();

							// A start-anchored run must be the first thing the message emits:
							// nothing may precede it in this part, nor in any earlier part.
							if run.anchored_start
								&& ((0 != start_offset) || !model.can_start_at(index))
							{
								continue;
							}
							// And symmetrically for the end.
							if run.anchored_end
								&& ((end_offset != text.chars().count())
									|| !model.can_end_at(index))
							{
								continue;
							}

							for_run.push(Placement::within_part(
								index,
								start_offset,
								end_offset,
								run.text.clone(),
								false,
							));
						}

						// Straddle starting in static text: this text supplies a trailing
						// piece of itself as the run's leading piece,
						// and the following parts supply the rest.
						for_run.extend(Self::straddles_from_text(
							spec, model, run, index, text, fits,
						)?);
					},
					ShapePart::Variable(variable) => {
						// Anchoring demands more than containment. A start-anchored run must be
						// the first thing the message emits, so every earlier part must be able
						// to vanish *and* the rule's match must *begin* with the run, not
						// merely contain it: without that a query of `N*` would be placed in a
						// rule matching `WARN`. The end is the mirror image; anchored at both
						// ends the rule must match the run exactly, with nothing around it.
						let fits_here: bool = {
							let pinned: Pinned = Pinned {
								start: run.anchored_start,
								end: run.anchored_end,
							};
							(!run.anchored_start || model.can_start_at(index))
								&& (!run.anchored_end || model.can_end_at(index))
								&& fits.matches_piece(spec, &variable.name, &run.text, pinned)
						};
						if fits_here {
							// Wholly inside a rule: no fixed position within the part.
							for_run.push(Placement::within_part(
								index,
								0,
								0,
								run.text.clone(),
								true,
							));
						}

						// Straddle: this rule supplies a leading piece of the run,
						// and the following parts supply the rest.
						for_run.extend(Self::straddles_from(spec, model, run, index, fits)?);
					},
				}

				if for_run.len() > MAX_PLACEMENTS_PER_RUN {
					info!(
						"falling back to the engine: run {run_index} ({:?}) has more than \
						 {MAX_PLACEMENTS_PER_RUN} placements, so the table is incomplete",
						run.text
					);
					// The table would be incomplete; refuse to answer rather than answer wrongly.
					return None;
				}
			}

			// Where this run can leave off, for the next one.
			previous_exits = BTreeMap::new();
			for placement in for_run.iter() {
				let (part, offset) = placement.next_available();
				previous_exits.entry(part).or_default().push(offset);
			}
			for offsets in previous_exits.values_mut() {
				offsets.sort_unstable();
				offsets.dedup();
			}

			placements.push(for_run);
		}

		Some(Self { placements })
	}

	/// Placements where the static text at `index` supplies a *leading* piece of `run`
	/// and the parts after it supply the remainder.
	///
	/// The leading piece must be a *suffix* of the static text:
	/// the run continues past the end of the text into whatever follows,
	/// so the text's own tail is what it can contribute.
	fn straddles_from_text(
		spec: &ParsingSpec,
		model: &ShapeModel,
		run: &Run,
		index: usize,
		text: &str,
		fits: &RunFitCache,
	) -> Option<Vec<Placement>> {
		let characters: Vec<char> = run.text.chars().collect::<Vec<_>>();
		let mut results: Vec<Placement> = Vec::new();

		// `split` is how many leading characters of the run the static text supplies;
		// a proper non-empty prefix, since the whole-run case is handled by the substring search.
		for split in 1..characters.len() {
			let head: String = characters[..split].iter().collect::<String>();
			if !text.ends_with(&head) {
				continue;
			}
			// A start-anchored run must be the first thing the message emits,
			// so nothing may precede it:
			// every earlier part must be able to vanish,
			// and the head must be the whole of this text.
			if run.anchored_start
				&& (!model.can_start_at(index) || (head.chars().count() != text.chars().count()))
			{
				continue;
			}

			// The head is a suffix of this text, so the run begins where that suffix begins.
			let start_offset: usize = text.chars().count() - head.chars().count();
			let straddle: Straddle<'_> = Straddle {
				spec,
				model,
				run,
				characters: &characters,
				fits,
				start_part: index,
				start_offset,
			};
			results.extend(straddle.extend(
				index + 1,
				split,
				vec![Piece {
					part: index,
					offset: start_offset,
					text: head,
					is_rule: false,
				}],
			)?);
		}

		Some(results)
	}

	/// Placements where the rule at `index` supplies a *leading* piece of `run`
	/// and the parts after it supply the remainder.
	///
	/// The remainder is matched greedily against the following parts:
	/// static text must match verbatim from its start,
	/// and a following rule must be able to *begin* with its piece.
	/// This is the "inside/outside" split, and it is bounded by the surrounding literal text.
	fn straddles_from(
		spec: &ParsingSpec,
		model: &ShapeModel,
		run: &Run,
		index: usize,
		fits: &RunFitCache,
	) -> Option<Vec<Placement>> {
		// A start-anchored run cannot begin part-way through a rule's output
		// unless nothing can precede that rule.
		if run.anchored_start && !model.can_start_at(index) {
			return Some(Vec::new());
		}

		let characters: Vec<char> = run.text.chars().collect::<Vec<_>>();
		let mut results: Vec<Placement> = Vec::new();
		let straddle: Straddle<'_> = Straddle {
			spec,
			model,
			run,
			characters: &characters,
			fits,
			start_part: index,
			// Begins inside a rule: no position within the part.
			start_offset: 0,
		};

		// `split` is how many leading characters the rule supplies;
		// it must be a non-empty proper prefix,
		// since `split == 0` and `split == len` are the non-straddling cases handled elsewhere.
		for split in 1..characters.len() {
			let head: String = characters[..split].iter().collect::<String>();
			// An anchored run pins the rule's start as well as its end,
			// so the rule must match the piece exactly;
			// otherwise `NIn*` would be split as `level=N` even though no level is just `N`.
			let pinned: Pinned = Pinned {
				start: run.anchored_start,
				end: true,
			};
			if !fits.matches_piece(
				spec,
				model.parts[index].variable_name().expect("a variable"),
				&head,
				pinned,
			) {
				continue;
			}

			results.extend(straddle.extend(
				index + 1,
				split,
				vec![Piece {
					part: index,
					offset: 0,
					text: head,
					is_rule: true,
				}],
			)?);
		}

		Some(results)
	}
}

/// The character offsets in `text` where a placement of `run` is worth recording.
///
/// Enumerating *every* occurrence -- including overlapping ones, which
/// `str::match_indices` skips -- then discarding what neither composition nor
/// rendering can distinguish:
///
/// - **Composition** enters the part at offset `0` or wherever the previous run
///   left off (`entries`), and only ever needs the earliest occurrence at or after
///   each such position: an earlier end leaves strictly more room for the runs after.
/// - **Rendering** reads only *where* a piece sits in the text (its start and
///   whether it reaches the end), and the compositions that differ only there are
///   merged (`merge_by_padding`), OR-ing the padding flags. Keeping the
///   occurrences flush with the text's start or end is what supplies each side
///   of the merge, so they are always kept.
///
/// A long stretch of repeated characters (a banner of `=`, say) holds a run at very
/// many overlapping offsets; keeping only these keeps a shape out of the
/// [`MAX_PLACEMENTS_PER_RUN`] cap without losing a placement that matters.
fn usable_occurrences(text: &str, run: &str, entries: &[usize]) -> Vec<usize> {
	let run_len: usize = run.chars().count();
	let text_len: usize = text.chars().count();
	let occurrences: Vec<usize> = Vec::from_iter(
		text.char_indices()
			.enumerate()
			.filter_map(|(offset, (byte, _))| text[byte..].starts_with(run).then_some(offset)),
	);

	let is_interior = |offset: usize| (0 < offset) && ((offset + run_len) < text_len);
	let interior: Vec<usize> =
		Vec::from_iter(occurrences.iter().copied().filter(|&o| is_interior(o)));
	let mut offsets: Vec<usize> =
		Vec::from_iter(occurrences.iter().copied().filter(|&o| !is_interior(o)));

	for &entry in std::iter::once(&0).chain(entries.iter()) {
		let first_interior_at_or_after: usize = interior.partition_point(|&o| o < entry);
		if let Some(&offset) = interior.get(first_interior_at_or_after) {
			offsets.push(offset);
		}
	}

	offsets.sort_unstable();
	offsets.dedup();
	offsets
}

/// A straddle in progress: one run, begun at a fixed position, being traced through the parts
/// that follow. Holds what is invariant across the recursion in [`Self::extend`].
struct Straddle<'a> {
	spec: &'a ParsingSpec,
	model: &'a ShapeModel,
	run: &'a Run,
	/// `run.text` as characters, so that split points index in `O(1)`.
	characters: &'a [char],
	fits: &'a RunFitCache,
	/// The part the run begins in.
	start_part: usize,
	/// Where in `start_part`'s static text the run begins; zero inside a rule.
	start_offset: usize,
}

impl Straddle<'_> {
	/// How many characters a rule at `part` may contribute as a *middle* piece of the run.
	///
	/// See the call site for why the following shape part pins these candidates.
	///
	/// Returns `None` when the candidate set had to be **truncated**,
	/// i.e. when a split that could still be correct was not tried.
	/// The caller must then treat the whole table as incomplete rather than conclude
	/// the shape cannot match: silently dropping a candidate is a false rejection,
	/// not an approximation.
	fn candidate_middle_lengths(&self, part: usize, consumed: usize) -> Option<Vec<usize>> {
		/// Cap on candidates when nothing in the shape pins the split,
		/// i.e. between back-to-back rules. Without a bound,
		/// a chain of adjacent rules multiplies candidates per link.
		///
		/// Exceeding it means some split was not tried, which is *not* the same as
		/// "no split works", so the caller poisons the table rather than reject the shape.
		const MAX_UNPINNED_SPLITS: usize = 8;

		let available: usize = self.characters.len() - consumed;
		// A middle piece offered here is non-empty and must leave something for the following
		// parts. (An *empty* middle piece, for a nullable rule, is handled by the caller.)
		if available < 2 {
			return Some(Vec::new());
		}

		match self.model.parts.get(part + 1) {
			Some(ShapePart::Static(text)) => {
				// The run must continue with `text`, so the rule's piece ends where `text` begins.
				let Some(boundary) = text.chars().next() else {
					return Some(Vec::new());
				};
				// Every candidate is tried, so this side is always complete.
				Some(Vec::from_iter((1..available).filter(|&take| {
					self.characters[consumed + take] == boundary
				})))
			},
			// Nothing pins the boundary, so every split must be tried. A middle piece is non-empty
			// and must leave something for the following parts,
			// so there are `available - 1` candidates; if that exceeds the cap,
			// not all of them are tried and no conclusion may be drawn.
			Some(ShapePart::Variable(_)) => {
				if (available - 1) > MAX_UNPINNED_SPLITS {
					info!(
						"falling back to the engine: a run spans back-to-back rules with {} characters \
						 left, more than the {MAX_UNPINNED_SPLITS} splits that can be tried",
						available
					);
					return None;
				}
				Some(Vec::from_iter(1..available))
			},
			// The rule is the last part, so it cannot be a *middle* piece.
			None => Some(Vec::new()),
		}
	}

	/// The completed placement, once every character of the run has been supplied.
	///
	/// `None` if the run is end-anchored but does not end the message:
	/// every later part must be able to vanish,
	/// and the run must reach the end of the part it finishes in.
	/// (A piece that ends inside a *rule* is handled where that piece is produced,
	/// which is the only place that knows whether the rule may emit more after it.)
	fn complete(&self, pieces: Vec<Piece>) -> Option<Placement> {
		let last: Option<&Piece> = pieces.last();
		let end_part: usize = last.map_or(self.start_part, |piece| piece.part);
		// A straddle's final piece starts at the beginning of its part (the run flows into it),
		// so the end offset is just that piece's length --
		// and is meaningless if the piece is a rule.
		let end_offset: usize = match last {
			Some(piece) if !piece.is_rule => piece.text.chars().count(),
			_ => 0,
		};

		if self.run.anchored_end {
			let ends_cleanly: bool = match (last, &self.model.parts[end_part]) {
				(Some(piece), ShapePart::Static(text)) if !piece.is_rule => {
					end_offset == text.chars().count()
				},
				_ => true,
			};
			if !ends_cleanly || !self.model.can_end_at(end_part) {
				return None;
			}
		}

		Some(Placement {
			start_part: self.start_part,
			end_part,
			start_offset: self.start_offset,
			end_offset,
			pieces,
		})
	}

	/// Completes the straddle: given `consumed` characters of the run already supplied by parts
	/// up to `part - 1`, matches the remainder against `part` onwards.
	///
	/// Shared by both straddle directions (starting from static text or from a rule).
	/// Each following part must supply the run's next characters *contiguously*,
	/// since a run has no wildcards inside it: static text from its very start,
	/// and a rule either by finishing the run (it can begin with what remains),
	/// by producing exactly a middle piece and handing off to the next part,
	/// or, if the rule is nullable, by producing nothing and handing off immediately.
	///
	/// Returns every completion,
	/// because a rule in the middle of a run can take any number of characters.
	/// This is what lets a run be traced through an alternating sequence such as
	/// `blk_%blockNum%_%genStamp%`, where the run spans four parts.
	///
	/// Returns `None` if any candidate split had to be dropped,
	/// which means the placement set would be incomplete and nothing may be concluded from it.
	/// An empty `Some` is a real answer: no completion exists.
	fn extend(&self, part: usize, consumed: usize, pieces: Vec<Piece>) -> Option<Vec<Placement>> {
		// The run is fully produced: this is a complete placement.
		if consumed == self.characters.len() {
			return Some(Vec::from_iter(self.complete(pieces)));
		}

		// Ran out of shape before the run was fully produced.
		let Some(next) = self.model.parts.get(part) else {
			return Some(Vec::new());
		};

		let remaining: &[char] = &self.characters[consumed..];

		match next {
			ShapePart::Static(text) => {
				// The static text must supply the next characters from its very start.
				let available: usize = text.chars().count();
				let take: usize = available.min(remaining.len());
				let head: String = remaining[..take].iter().collect::<String>();
				if !text.starts_with(&head) {
					return Some(Vec::new());
				}
				// If the run continues past this part, it must have consumed all of the text;
				// otherwise there would be leftover literal text between the run's pieces.
				if (take < remaining.len()) && (take < available) {
					return Some(Vec::new());
				}

				let mut pieces: Vec<Piece> = pieces;
				pieces.push(Piece {
					part,
					// The run flows into this part, so its piece starts at the part's very
					// beginning.
					offset: 0,
					text: head,
					is_rule: false,
				});
				self.extend(part + 1, consumed + take, pieces)
			},
			ShapePart::Variable(variable) => {
				let mut results: Vec<Placement> = Vec::new();
				let remaining: String = remaining.iter().collect::<String>();

				// The rule finishes the run: it must *begin* with everything that remains.
				// An end-anchored run additionally forbids the rule from emitting more
				// afterwards, so the rule must match the remainder exactly
				// and nothing may follow it (`can_end_at`).
				let pinned: Pinned = Pinned {
					start: true,
					end: self.run.anchored_end,
				};
				let finishes_here: bool = (!self.run.anchored_end || self.model.can_end_at(part))
					&& self
						.fits
						.matches_piece(self.spec, &variable.name, &remaining, pinned);
				if finishes_here {
					let mut pieces: Vec<Piece> = pieces.clone();
					pieces.push(Piece {
						part,
						offset: 0,
						text: remaining,
						is_rule: true,
					});
					results.push(Placement {
						start_part: self.start_part,
						end_part: part,
						start_offset: self.start_offset,
						// Ends inside a rule, so there is no position within the part.
						end_offset: 0,
						pieces,
					});
				}

				// Or the rule produces exactly a middle piece,
				// and the run continues into the next part.
				// The piece must be matched *exactly*: a run has no wildcards,
				// so the rule cannot emit anything beyond it.
				//
				// The candidate split points are not arbitrary.
				// Whatever follows this rule in the shape pins them:
				//
				// - static text: the run must continue with that text,
				//   so the rule's piece ends exactly where the text's first character
				//   next occurs in the run -- a handful of candidates,
				//   found by substring search, not one per length;
				// - another rule (back-to-back, no literal boundary): nothing pins the split,
				//   so every length must be tried. `candidate_middle_lengths` refuses to answer
				//   when it cannot try them all,
				//   which poisons the table rather than dropping a possible match.
				for take in self.candidate_middle_lengths(part, consumed)? {
					let middle: String = self.characters[consumed..(consumed + take)]
						.iter()
						.collect::<String>();
					if !self.fits.matches_piece(
						self.spec,
						&variable.name,
						&middle,
						Pinned {
							start: true,
							end: true,
						},
					) {
						continue;
					}
					let mut pieces: Vec<Piece> = pieces.clone();
					pieces.push(Piece {
						part,
						offset: 0,
						text: middle,
						is_rule: true,
					});
					results.extend(self.extend(part + 1, consumed + take, pieces)?);
				}

				// Or the rule produces nothing at all, and the run passes straight through it.
				// This is the empty middle piece `candidate_middle_lengths` never offers;
				// without it a run such as `ab` could not cross `a%optional.pad%b`.
				// The empty piece is still recorded, so the rule is reported as an empty capture,
				// which is exactly what the run pins it to.
				if variable.can_match_empty {
					let mut pieces: Vec<Piece> = pieces;
					pieces.push(Piece {
						part,
						offset: 0,
						text: String::new(),
						is_rule: true,
					});
					results.extend(self.extend(part + 1, consumed, pieces)?);
				}

				Some(results)
			},
		}
	}
}

/// A position in the shape: a part, and a character offset within it.
///
/// Ordered lexicographically, which is exactly "no earlier in the shape than".
/// The offset is what allows several runs to occupy one static part;
/// it is always zero for a position inside a rule, where no fixed position exists.
pub type Position = (usize, usize);

/// The composition feasibility DP over `(run index, earliest available position)`.
///
/// `reachable[i][p]` is true when runs `i..` can all be placed without starting before
/// position `p`. Solved by a reverse sweep, since every placement advances both coordinates.
///
/// The state is a *position* rather than a part index, so the state space is discretised:
/// only positions where some placement starts, or where some placement leaves off,
/// can ever be visited. Keeping this as bits, separate from enumerating the compositions,
/// is what lets a shape be rejected in polynomial time even when the number of compositions
/// is large, and bounds memory on shapes with tens of thousands of parts.
#[derive(Clone, Debug)]
pub struct Reachability {
	/// The distinct positions a composition can be in, sorted.
	positions: Vec<Position>,
	/// `reachable[(run * positions.len()) + position]`.
	reachable: Vec<bool>,
}

impl Reachability {
	#[must_use]
	pub fn compute(table: &PlacementTable, num_parts: usize) -> Self {
		let mut positions: Vec<Position> = vec![(0, 0), (num_parts, 0)];
		for placement in table.placements.iter().flatten() {
			positions.push(placement.start());
			positions.push(placement.next_available());
		}
		positions.sort_unstable();
		positions.dedup();

		let num_runs: usize = table.placements.len();
		let width: usize = positions.len();
		let mut this: Self = Self {
			positions,
			reachable: vec![false; (num_runs + 1) * width],
		};

		// With no runs left, every position is fine.
		for position in 0..width {
			this.reachable[(num_runs * width) + position] = true;
		}

		for run in (0..num_runs).rev() {
			for position in 0..width {
				this.reachable[(run * width) + position] =
					table.placements[run].iter().any(|placement| {
						(placement.start() >= this.positions[position])
							&& this.is_reachable(run + 1, this.index_of(placement.next_available()))
					});
			}
		}

		this
	}

	/// Whether runs `run..` can all be placed without starting before `positions[position]`.
	#[must_use]
	pub fn is_reachable(&self, run: usize, position: usize) -> bool {
		self.reachable[(run * self.positions.len()) + position]
	}

	/// The sorted positions the DP ranges over; `position` indices refer into this.
	#[must_use]
	pub fn positions(&self) -> &[Position] {
		&self.positions
	}

	/// The index of `position` in [`Self::positions`].
	///
	/// Panics if absent, which would mean the DP disagrees with itself about the state space.
	#[must_use]
	pub fn index_of(&self, position: Position) -> usize {
		self.positions
			.binary_search(&position)
			.expect("position was collected")
	}
}

/// Whether the runs can be placed left to right without overlapping.
///
/// Answering this separately from *enumerating* the compositions means a shape can be rejected
/// cheaply even when the number of compositions is large; see [`Reachability`].
#[cfg(test)]
#[must_use]
pub fn can_compose(table: &PlacementTable, num_parts: usize) -> bool {
	!table.is_impossible() && Reachability::compute(table, num_parts).is_reachable(0, 0)
}
