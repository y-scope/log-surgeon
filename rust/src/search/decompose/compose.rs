//! Composes [`Placement`]s into whole-shape decompositions.
//!
//! # The DP
//!
//! A [`Composition`] chooses one placement per run such that the chosen placements run
//! left to right and do not overlap. The state is `(run index, earliest usable part)`
//! and every placement advances both, so the state space is a DAG and is solved
//! by a reverse sweep -- the same structure as [`crate::search::decompose::prefilter`],
//! and the reason feasibility can be decided in `O(runs x parts x placements)`
//! even when the number of compositions is large.
//!
//! Enumeration is separated from feasibility (`Reachability`, checked before enumerating),
//! so that a shape can be *rejected* cheaply, and only a shape that survives pays for
//! materializing its decompositions. A [`ComposeBudget`] caps enumeration,
//! so a pathological shape degrades to "no conclusion" instead of exploding.
//!
//! # Positional identity
//!
//! Each [`Capture`] names the **index of the shape part** that produced it,
//! so two references to the same rule in one shape are distinguishable
//! even when they share a name.
//!
//! [`Composition::to_interpretation`] renders that positionally:
//! every rule reference up to the last constrained one contributes a sub-query,
//! with unconstrained references appearing as a bare `*`. The position of a capture
//! among those variables is what identifies which reference it is,
//! so `A%foo%B%foo%C` distinguishes its two `foo`s by whether a vacuous `foo=*`
//! precedes the constrained one.
//!
//! Static text is reported even where the query does not constrain it, as a `'*'`
//! sub-query, so the output has the same shape as the engine's
//! and [`crate::search::SearchString::search_by_name`]'s: a run split across static text
//! and a rule yields one sub-query for each. This is pure rendering -- the parts and
//! their attribution are already known, so no automaton or simulation is involved.
//!
//! A static sub-query's value must glob-match its part's text *exactly*,
//! so a run covering only part of a stretch is padded with `*` on both sides;
//! see [`symbolic_value_of`], which decides this from where the pieces sit
//! rather than from whether the part is a rule.
//! Compositions that differ *only* in where pieces sit within static text
//! are one decomposition, and are merged; see [`merge_by_padding`].
//!
//! References *after* the last constrained capture are omitted when the query is not
//! anchored at the end: its trailing wildcard leaves them unconstrained,
//! so they add nothing. Their static text is still reported,
//! since that is where the trailing wildcard applies;
//! if the rendering ends in static text, that value gets a trailing `*` for them.

#[cfg(test)]
mod test;

use std::collections::BTreeMap;

use crate::parsing_spec::ParsingSpec;
use crate::search::Interpretation;
use crate::search::LeafQuery;
use crate::search::SymbolicChar;
use crate::search::decompose::Placement;
use crate::search::decompose::PlacementTable;
use crate::search::decompose::Query;
use crate::search::decompose::RunFitCache;
use crate::search::decompose::ShapeModel;
use crate::search::decompose::ShapePart;
use crate::search::decompose::placement::Reachability;
use crate::search::decompose::run_fit::Side;

/// One complete way the query's literal text maps onto a shape.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Composition {
	/// The text attributed to rules, in shape order.
	pub captures: Vec<Capture>,
	/// The text attributed to the shape's static text, in shape order.
	pub literals: Vec<Literal>,
}

/// Query text produced by one rule reference in the shape.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Capture {
	/// Index into [`ShapeModel::parts`]: *which* reference, not merely which rule.
	pub part: usize,
	/// The rule's name as written in the shape.
	pub name: String,
	/// The characters of the query this rule must produce.
	///
	/// Several runs can land in one rule, so this may hold more than one piece;
	/// they are recorded in order and are separated by unconstrained text.
	pub pieces: Vec<PieceRef>,
}

/// Query text produced by the shape's static text.
///
/// Where in the text the pieces sit is *not* recorded, only whether text of the part remains
/// before the first piece and after the last. Static text is fixed, so the exact offsets carry no
/// information for the user; they matter only to the composition DP (which has already run) and
/// to padding (which these flags decide). Dropping them is what lets [`merge_by_padding`] treat
/// compositions that differ only in offsets as the one decomposition they are.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Literal {
	/// Index into [`ShapeModel::parts`].
	pub part: usize,
	/// The characters of the query this static text supplies.
	pub pieces: Vec<PieceRef>,
	/// Whether text of the part may precede the first piece, so the value must begin with `*`.
	pub pad_before: bool,
	/// Whether text of the part may follow the last piece, so the value must end with `*`.
	pub pad_after: bool,
}

/// A piece of query text, and the run it came from.
///
/// The run index is what makes wildcard placement recoverable: characters within one run are
/// contiguous in the query and must stay contiguous in the rendering,
/// while a run *boundary* is exactly where the query had a wildcard.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PieceRef {
	/// Index into the query's runs.
	pub run: usize,
	/// The characters supplied.
	pub text: String,
}

/// How a part is padded when rendered; see [`symbolic_value_of`].
#[derive(Clone, Copy, Debug)]
enum Padding {
	/// Static text, with the padding decided by [`Literal::pad_before`]/[`Literal::pad_after`].
	Static { before: bool, after: bool },
	/// A rule, padded wherever the query permits.
	Rule,
}

/// One shape part's contribution to a rendered interpretation.
struct RenderedPart {
	/// Index into [`ShapeModel::parts`].
	part: usize,
	/// The value this part's sub-query will carry.
	value: Vec<SymbolicChar>,
	/// How many pieces were merged into it; more than one needs a feasibility check.
	piece_count: usize,
}

impl Composition {
	/// Renders this composition as an [`Interpretation`].
	///
	/// Shape parts are emitted in order: static text as a static sub-query,
	/// rule references as captures. A reference with no query text attributed to it
	/// becomes a vacuous `*` capture, so a capture's position among the variables
	/// identifies which reference it is -- this is how two references to
	/// one rule name are told apart.
	///
	/// Static text is reported whether or not the query constrains it,
	/// matching the engine and the shape of [`crate::search::SearchString::search_by_name`]'s
	/// output: a stretch the query's wildcard merely passes over becomes `'*'`.
	/// Consecutive static parts are merged into one sub-query,
	/// since [`Interpretation::invariants`] forbids two static sub-queries in a row.
	///
	/// Rule references after the last constrained one are **omitted**:
	/// the query's trailing wildcard leaves them unconstrained,
	/// and reporting each as a vacuous capture would add no information.
	/// What is left of the shape after the last constrained part is not dropped outright but
	/// rendered by the tail rule shared with the engine
	/// (`crate::search::Tail`):
	/// dropped variables are omitted,
	/// while dropped static text has to show up as a trailing `'*'`
	/// (on the last static sub-query, or as one of its own),
	/// because the message still contains that text.
	///
	/// # Invariant
	///
	/// Every literal character of the query appears in the result, in order,
	/// however it was split between static text and rules. A wildcard appears exactly
	/// where the query had one, i.e. between characters of different runs;
	/// characters of the *same* run stay adjacent, with no wildcard introduced
	/// between them even when they straddle a part boundary.
	///
	/// Runs and end-anchoring are read from `query` rather than passed separately,
	/// since the runs alone cannot tell the empty query (anchored at both ends,
	/// with no runs at all) from `*` (anchored nowhere).
	///
	/// A capture's padding is then **tightened** to what the rule can use;
	/// see `tighten_capture`. That is why `spec` and `fits` are needed.
	#[must_use]
	pub fn to_interpretation(
		&self,
		spec: &ParsingSpec,
		model: &ShapeModel,
		query: &Query<'_>,
		fits: &RunFitCache,
	) -> Interpretation {
		let rendered: Vec<RenderedPart> = self.rendered_parts(model, query);

		// Nothing is constrained and nothing is pinned: `*` reports a bare `'*'`.
		// (An end-anchored query renders every part, so it never gets here;
		// for the empty query that is a row of empty captures.)
		if rendered.is_empty() && !query.anchored_end {
			return Interpretation {
				leaf_queries: vec![LeafQuery::new_static_text(vec![SymbolicChar::GlobStar])],
			};
		}

		let mut leaf_queries: Vec<LeafQuery> = Vec::new();

		for part in rendered.into_iter() {
			match &model.parts[part.part] {
				// A name can resolve to several rules, but they occupy the same position,
				// so emitting more than one would duplicate the slot
				// and break the positional correspondence.
				// Every alternative has the same `fully_qualified_name`,
				// so picking the first loses only which `CaptureRef` accepted the value;
				// see `Variable::alternatives`.
				ShapePart::Variable(variable) => {
					if let Some(capture) = variable.alternatives.first() {
						leaf_queries.push(LeafQuery::new_rule(
							capture.fully_qualified_name.clone(),
							tighten_capture(spec, fits, &variable.name, part.value),
						));
					}
				},
				ShapePart::Static(_) => leaf_queries.push(LeafQuery::new_static_text(part.value)),
			}
		}

		// An unanchored query's trailing wildcard absorbs the parts after the last constrained
		// one, which `rendered_parts` therefore left out; the tail rule accounts for them.
		// An end-anchored query has them rendered, each pinned to the empty string.
		if !query.anchored_end
			&& let Some(last_constrained) = self.last_constrained()
		{
			crate::search::Tail::of_parts(&model.parts[(last_constrained + 1)..])
				.finish_unanchored(&mut leaf_queries);
		}

		Interpretation { leaf_queries }
	}

	/// The last part any run puts text in. `None` on an empty composition.
	///
	/// This includes where a straddling run *ends*: [`build_composition`] records a
	/// capture or literal for every part a placement has a piece in,
	/// so the parts here are exactly the parts with pieces.
	fn last_constrained(&self) -> Option<usize> {
		self.captures
			.iter()
			.map(|capture| capture.part)
			.chain(self.literals.iter().map(|literal| literal.part))
			.max()
	}

	/// The shape parts that contribute a sub-query, with their rendered values.
	///
	/// For an unanchored query, only parts up to `last_constrained` are rendered:
	/// variables past that are unconstrained and omitted,
	/// and static text past that is what the caller's tail rule stands for --
	/// see [`crate::search::Tail`].
	/// An end-anchored query renders *every* part:
	/// the ones past the last constrained part must emit nothing,
	/// which `symbolic_value_of` reports as empty captures.
	/// That includes the empty query, whose composition has no pieces at all.
	///
	/// Factored out so that rendering and multi-piece verification agree by construction:
	/// the value checked against a rule is the very one that will be reported.
	fn rendered_parts(&self, model: &ShapeModel, query: &Query<'_>) -> Vec<RenderedPart> {
		let window: Option<usize> = if query.anchored_end {
			model.parts.len().checked_sub(1)
		} else {
			self.last_constrained()
		};
		let Some(window_end) = window else {
			return Vec::new();
		};

		let pieces_by_part: BTreeMap<usize, &[PieceRef]> = (0..model.parts.len())
			.map(|part| (part, self.pieces_for(part)))
			.collect();

		let emitted: Vec<usize> = (0..=window_end).collect();

		// The run touching the character just before each emitted part,
		// and the one just after: from one forward pass and one reverse pass respectively.
		let mut preceding: Vec<Option<usize>> = Vec::with_capacity(emitted.len());
		let mut previous: Option<usize> = None;
		for &part in emitted.iter() {
			preceding.push(previous);
			if let Some(last) = pieces_by_part[&part].last() {
				previous = Some(last.run);
			}
		}
		let mut following: Vec<Option<usize>> = vec![None; emitted.len()];
		let mut next: Option<usize> = None;
		for index in (0..emitted.len()).rev() {
			following[index] = next;
			if let Some(first) = pieces_by_part[&emitted[index]].first() {
				next = Some(first.run);
			}
		}

		let mut rendered: Vec<RenderedPart> = Vec::with_capacity(emitted.len());

		for (index, &part) in emitted.iter().enumerate() {
			// Static text's value must glob-match that text exactly;
			// a rule is padded by what the query permits instead. See `symbolic_value_of`.
			let padding: Padding = match &model.parts[part] {
				ShapePart::Static(_) => {
					let literal: Option<&Literal> =
						self.literals.iter().find(|literal| literal.part == part);
					Padding::Static {
						before: literal.is_some_and(|literal| literal.pad_before),
						after: literal.is_some_and(|literal| literal.pad_after),
					}
				},
				ShapePart::Variable(_) => Padding::Rule,
			};

			let value: Vec<SymbolicChar> = symbolic_value_of(
				pieces_by_part[&part],
				query,
				preceding[index],
				following[index],
				padding,
			);
			// Pinned to the empty string with no text attributed: placement admitted the
			// composition only if every part an anchor pins this way can vanish
			// (`can_start_at` / `can_end_at`); see `symbolic_value_of`.
			debug_assert!(
				!(value.is_empty() && pieces_by_part[&part].is_empty())
					|| model.parts[part].can_be_empty(),
				"part {part} pinned to empty but cannot vanish"
			);
			rendered.push(RenderedPart {
				part,
				value,
				piece_count: pieces_by_part[&part].len(),
			});
		}

		// Merge consecutive static parts into one, since [`Interpretation::invariants`]
		// forbids two static sub-queries in a row. A capture is never merged:
		// its position is what identifies which rule reference it is.
		//
		// The only way two static parts are adjacent is a genuinely-adjacent pair
		// (`a%%b` escapes a `%` as text): each value glob-matches its own text exactly,
		// so the concatenation glob-matches the concatenated text.
		let mut merged: Vec<RenderedPart> = Vec::with_capacity(rendered.len());
		for part in rendered.into_iter() {
			if !model.parts[part.part].is_variable()
				&& let Some(last) = merged.last_mut()
				&& !model.parts[last.part].is_variable()
			{
				last.value =
					condense_wildcards(last.value.iter().chain(part.value.iter()).cloned());
				last.piece_count += part.piece_count;
				continue;
			}
			merged.push(part);
		}

		merged
	}

	/// The captures that need a multi-piece feasibility check, as `(rule name, value)`.
	///
	/// A capture holding pieces from *several* runs was validated only one run at a time:
	/// each run can sit in the rule, but that does not mean the rule can produce
	/// them all together. An alternation such as `INFO|WARN` admits either alone
	/// and neither pair. Single-piece captures are already exact, so they are skipped.
	///
	/// Values come from the same rendering used by [`Self::to_interpretation`],
	/// so the check asks about exactly the string that will be reported --
	/// including whether it is padded, which decides whether the rule may emit
	/// anything around it.
	#[must_use]
	pub fn captures_to_verify(
		&self,
		model: &ShapeModel,
		query: &Query<'_>,
	) -> Vec<(String, String)> {
		Vec::from_iter(
			self.rendered_parts(model, query)
				.into_iter()
				.filter_map(|rendered| {
					match &model.parts[rendered.part] {
						ShapePart::Variable(variable) if rendered.piece_count > 1 => {
							// `Display` escapes the characters a query treats specially,
							// so the value round-trips through `SearchString::parse`
							// as the literal text it stands for.
							Some((
								variable.name.clone(),
								String::from_iter(rendered.value.iter().map(ToString::to_string)),
							))
						},
						_ => None,
					}
				}),
		)
	}

	/// The query text attributed to shape part `part`.
	fn pieces_for(&self, part: usize) -> &[PieceRef] {
		if let Some(capture) = self.captures.iter().find(|capture| capture.part == part) {
			return &capture.pieces;
		}
		if let Some(literal) = self.literals.iter().find(|literal| literal.part == part) {
			return &literal.pieces;
		}
		&[]
	}
}

/// Drops the padding wildcards of a rule capture's `value` that the rule cannot use.
///
/// Rendering pads a capture wherever the *query* permits (see [`symbolic_value_of`]),
/// so `*INFO*` against a `level` rule renders as `*INFO*`,
/// although no level has text before or after `INFO`.
/// The engine reports `INFO`, because its path runs through the rule's automaton.
/// This closes that gap with the rule's own answer,
/// [`RunFitCache::has_slack`], one side at a time:
/// a leading `*` goes when no match of the rule is `rest` with text in front,
/// and then a trailing `*` by the mirror image, on what is left.
///
/// Sound because each step preserves `L(rule) & L(value)`, the set of capture texts
/// the reported value can select: dropping the `*` of `*v` is exact when `L(rule)` misses
/// `L(.+ v)`. Only the *ends* are touched. An interior `*` marks a run boundary,
/// which the query itself demands, and the ends of a value are always padding --
/// `symbolic_value_of` never puts a run boundary first or last.
///
/// A vacuous capture (`*` alone) marks a position and is left as it is:
/// reducing it would claim the rule produced nothing, which is a different statement.
/// An empty value (a capture pinned to nothing) has no padding to drop.
fn tighten_capture(
	spec: &ParsingSpec,
	fits: &RunFitCache,
	name: &str,
	mut value: Vec<SymbolicChar>,
) -> Vec<SymbolicChar> {
	if value.len() < 2 {
		return value;
	}
	if value.first().is_some_and(SymbolicChar::is_wildcard)
		&& !fits.has_slack(spec, name, &value[1..], Side::Before)
	{
		value.remove(0);
	}
	if (2 <= value.len())
		&& value.last().is_some_and(SymbolicChar::is_wildcard)
		&& !fits.has_slack(spec, name, &value[..(value.len() - 1)], Side::After)
	{
		value.pop();
	}
	value
}

/// Concatenates values, collapsing the `**` that adjoining wildcards would otherwise produce.
///
/// Merging two static parts (each of which may itself begin or end with `*`) can create one.
/// `**` says exactly what `*` does, so this is purely about a canonical, readable output:
/// [`crate::search::LeafQuery::covers`] is exact containment and handles `**` correctly,
/// and [`Interpretation::invariants`] asserts the form only to keep producers honest.
fn condense_wildcards(symbols: impl Iterator<Item = SymbolicChar>) -> Vec<SymbolicChar> {
	let mut value: Vec<SymbolicChar> = Vec::new();
	for symbol in symbols {
		if symbol.is_wildcard() && value.last().is_some_and(SymbolicChar::is_wildcard) {
			continue;
		}
		value.push(symbol);
	}
	value
}

/// The symbolic value for one part's pieces.
///
/// A wildcard is emitted exactly where the query had one -- at a run boundary --
/// and nowhere else. In particular a run that crosses from one part into the next
/// stays contiguous: `preceding_run` and `following_run` name the runs adjacent
/// to this part, so a boundary can be told apart from a mere part boundary.
///
/// The two kinds of part are padded on different grounds, so `padding` distinguishes them.
///
/// A **rule** may emit text of its own around the query's characters,
/// so it is padded wherever the query permits: suppressed where the run continues
/// into a neighbouring part, and where the query anchors the run to the start
/// or end of the message.
///
/// **Static text** is reproduced verbatim, so its value must be a glob matching
/// the part's text *exactly*. The padding is therefore decided by where the pieces sit
/// ([`Literal::pad_before`]/[`Literal::pad_after`]) -- a wildcard stands for the characters
/// of the part the run does not cover, and appears if and only if there are some.
/// Anchoring and run continuation need no special case here: a run flowing in from the previous
/// part necessarily begins at offset zero, and one flowing out necessarily reaches the text's end,
/// so both fall out of where the pieces sit.
fn symbolic_value_of(
	pieces: &[PieceRef],
	query: &Query<'_>,
	preceding_run: Option<usize>,
	following_run: Option<usize>,
	padding: Padding,
) -> Vec<SymbolicChar> {
	// A part with no attributed text marks its position. It is a bare `*` -- anything --
	// unless an anchor leaves it no room: a rule before every run of a start-anchored query,
	// or after every run of an end-anchored one, must emit nothing for the message to begin
	// (or end) with the query's text. That is the same reasoning that suppresses a rule's
	// padding at an anchored end below, applied to a rule with no pieces;
	// the engine spells it as an empty capture.
	if pieces.is_empty() {
		let pinned: bool = matches!(padding, Padding::Rule)
			&& ((preceding_run.is_none() && query.anchored_start)
				|| (following_run.is_none() && query.anchored_end));
		return if pinned {
			Vec::new()
		} else {
			vec![SymbolicChar::GlobStar]
		};
	}

	let first: &PieceRef = pieces.first().expect("non-empty");
	let last: &PieceRef = pieces.last().expect("non-empty");
	// The run continues across the part boundary, so no wildcard may separate the characters.
	let continues_before: bool = preceding_run == Some(first.run);
	let continues_after: bool = following_run == Some(last.run);

	let mut value: Vec<SymbolicChar> = Vec::new();

	// Before: for static text, exactly the characters preceding the first piece;
	// for a rule, whatever the query's wildcard allows it to emit.
	let pad_before: bool = match padding {
		Padding::Static { before, .. } => before,
		Padding::Rule => {
			!continues_before && !(preceding_run.is_none() && query.runs[first.run].anchored_start)
		},
	};
	if pad_before {
		value.push(SymbolicChar::GlobStar);
	}

	// Seeded with the first piece's own run: any boundary *before* this part has already been
	// accounted for above, and comparing against `preceding_run` here would emit
	// a second wildcard for it.
	let mut previous_run: Option<usize> = Some(first.run);
	for piece in pieces.iter() {
		if previous_run.is_some_and(|previous| previous != piece.run) {
			value.push(SymbolicChar::GlobStar);
		}
		value.extend(piece.text.chars().map(SymbolicChar::Literal));
		previous_run = Some(piece.run);
	}

	// And after, by the mirror image of the same reasoning.
	let pad_after: bool = match padding {
		Padding::Static { after, .. } => after,
		Padding::Rule => {
			!continues_after && !(following_run.is_none() && query.runs[last.run].anchored_end)
		},
	};
	if pad_after {
		value.push(SymbolicChar::GlobStar);
	}

	value
}

/// Caps composition enumeration.
#[derive(Clone, Copy, Debug)]
pub struct ComposeBudget {
	pub max_compositions: usize,
}

impl Default for ComposeBudget {
	fn default() -> Self {
		Self {
			// Sized so that a shape which genuinely *has* many decompositions
			// is still answered here rather than deferred.
			// The engine is not a cheaper way to enumerate the same
			// answers -- it derives them by walking paths through an intersection,
			// which on such a shape is far more expensive and can hit its own path timeout.
			// Falling back is only worthwhile when the engine would do something different,
			// not when it would do the same thing slowly.
			//
			// The HDFS corpus motivates the size: `*INFO*blk*` against the
			// repeated-classpath shapes yields ~18 000 distinct interpretations,
			// enumerated in ~2s, where the engine times out.
			max_compositions: 65_536,
		}
	}
}

/// The result of composing a shape's placements.
#[derive(Clone, Debug)]
pub enum Composed {
	/// No assignment of runs to positions exists: the shape cannot match.
	Impossible,
	/// The complete set of decompositions.
	Compositions(Vec<Composition>),
	/// The budget was exhausted; the caller must not draw a conclusion.
	Unknown,
}

/// Enumerates every composition of `table`'s placements.
///
/// Compositions that assign several runs to one rule are verified against that rule
/// before being returned: [`PlacementTable`] places a run at a time, and a rule that
/// admits each run separately need not admit them together. Verification lives here,
/// rather than in the caller, so that an infeasible composition cannot escape.
///
/// End-anchoring is read from `query` rather than from the runs,
/// because the runs alone cannot tell the empty query, which is anchored,
/// from `*`, which is not: neither has any runs.
#[must_use]
pub fn compose(
	spec: &ParsingSpec,
	model: &ShapeModel,
	table: &PlacementTable,
	query: &Query<'_>,
	fits: &RunFitCache,
	budget: ComposeBudget,
) -> Composed {
	if table.is_impossible() {
		return Composed::Impossible;
	}

	let num_runs: usize = table.placements.len();
	if 0 == num_runs {
		// With no runs the query constrains nothing (`*`) or everything (the empty query).
		// For `*` there is exactly one decomposition, the empty one.
		if !query.anchored_end {
			return Composed::Compositions(vec![Composition {
				captures: Vec::new(),
				literals: Vec::new(),
			}]);
		}
		// The empty query pins *every* part to producing nothing; a part that
		// cannot vanish makes the match impossible. A rule that can vanish ends
		// up as an empty capture; see `to_interpretation`.
		if model.parts.iter().all(ShapePart::can_be_empty) {
			return Composed::Compositions(vec![Composition {
				captures: Vec::new(),
				literals: Vec::new(),
			}]);
		}
		return Composed::Impossible;
	}

	// Feasibility first, as *bits*. Keeping this separate from enumeration is what bounds
	// memory: materializing the compositions at every state instead would cost
	// `states x compositions`, and a shape can have tens of thousands of parts.
	// See [`Reachability`].
	let reachability: Reachability = Reachability::compute(table, model.parts.len());

	if !reachability.is_reachable(0, 0) {
		return Composed::Impossible;
	}

	// Then enumerate, walking only states already known to lead to a complete assignment.
	// Pruning on `reachability` means every branch entered yields at least one composition,
	// so the work is proportional to the number of compositions
	// rather than to the search space.
	let mut compositions: Vec<Composition> = Vec::new();
	let mut choices: Vec<usize> = Vec::with_capacity(num_runs);
	if !enumerate(
		model,
		table,
		&reachability,
		0,
		0,
		&mut choices,
		&mut compositions,
		budget,
	) {
		return Composed::Unknown;
	}

	// Compositions differing only in where text sits within static parts are one decomposition.
	// Merged before verification, which only inspects captures, so it is unaffected.
	let mut compositions: Vec<Composition> = merge_by_padding(compositions);

	// A rule holding pieces of several runs was validated one run at a time;
	// check it against all of them together. An alternation such as `INFO|WARN`
	// admits either run alone but never both.
	compositions.retain(|composition| {
		composition
			.captures_to_verify(model, query)
			.iter()
			.all(|(name, value)| fits.matches_query(spec, name, value))
	});

	compositions.sort();
	compositions.dedup();

	if compositions.is_empty() {
		return Composed::Impossible;
	}

	Composed::Compositions(compositions)
}

/// Depth-first enumeration of compositions, pruned by `reachability`.
///
/// Returns `false` if the budget was exceeded.
#[allow(clippy::too_many_arguments)]
fn enumerate(
	model: &ShapeModel,
	table: &PlacementTable,
	reachability: &Reachability,
	run: usize,
	position: usize,
	choices: &mut Vec<usize>,
	compositions: &mut Vec<Composition>,
	budget: ComposeBudget,
) -> bool {
	if run == table.placements.len() {
		if compositions.len() >= budget.max_compositions {
			return false;
		}
		compositions.push(build_composition(model, table, choices));
		return true;
	}

	for (choice, placement) in table.placements[run].iter().enumerate() {
		if placement.start() < reachability.positions()[position] {
			continue;
		}
		let next: usize = reachability.index_of(placement.next_available());
		if !reachability.is_reachable(run + 1, next) {
			continue;
		}

		choices.push(choice);
		let ok: bool = enumerate(
			model,
			table,
			reachability,
			run + 1,
			next,
			choices,
			compositions,
			budget,
		);
		choices.pop();

		if !ok {
			return false;
		}
	}

	true
}

/// Turns one set of per-run placement choices into a [`Composition`].
///
/// Pieces landing in the same shape part are merged,
/// which is what allows several runs to share a rule
/// (they are separated by text the rule also produces).
fn build_composition(model: &ShapeModel, table: &PlacementTable, choices: &[usize]) -> Composition {
	// Keyed by part index to merge pieces, then flattened in shape order.
	// Each piece keeps its offset here only long enough to decide the part's padding.
	let mut by_part: BTreeMap<usize, Vec<(usize, PieceRef)>> = BTreeMap::new();

	for (run, &choice) in choices.iter().enumerate() {
		let placement: &Placement = &table.placements[run][choice];
		for piece in placement.pieces.iter() {
			by_part.entry(piece.part).or_default().push((
				piece.offset,
				PieceRef {
					run,
					text: piece.text.clone(),
				},
			));
		}
	}

	let mut captures: Vec<Capture> = Vec::new();
	let mut literals: Vec<Literal> = Vec::new();

	for (part, pieces) in by_part.into_iter() {
		match &model.parts[part] {
			ShapePart::Variable(variable) => captures.push(Capture {
				part,
				name: variable.name.clone(),
				pieces: Vec::from_iter(pieces.into_iter().map(|(_, piece)| piece)),
			}),
			ShapePart::Static(text) => {
				let (first_offset, _) = pieces.first().expect("non-empty");
				let (last_offset, last) = pieces.last().expect("non-empty");
				let pad_before: bool = 0 != *first_offset;
				let pad_after: bool =
					(last_offset + last.text.chars().count()) < text.chars().count();
				literals.push(Literal {
					part,
					pieces: Vec::from_iter(pieces.into_iter().map(|(_, piece)| piece)),
					pad_before,
					pad_after,
				});
			},
		}
	}

	Composition { captures, literals }
}

/// Merges compositions that differ only in the padding of their static parts.
///
/// Such compositions place the same query text in the same parts and differ only in
/// *where* within some static text it sits (`*aa*` against `aaa` at offset 0 or 1),
/// which the user cannot observe: the text is fixed. Each is rendered as a different glob
/// (`'aa*'`, `'*aa'`), so without merging the one decomposition is reported several times.
///
/// The merge ORs the padding flags (`'*aa*'`). That is sound: adding a `*` to either end of a
/// glob that matches the part's text still matches it, so every message the group matches,
/// the merged value matches too.
///
/// It is **not** exact: the merged value can be strictly looser than the group.
/// `L('*aa*')` contains `baab`, which neither `'aa*'` nor `'*aa'` admits;
/// in general the union of the members is not expressible as one glob.
/// What the merge preserves is what the user can act on --
/// which parts the query text lands in, and in what order --
/// and it keeps one decomposition from being reported once per offset.
/// The differential tests do not observe the difference, because they compare a normal form
/// that strips wildcards.
/// Reporting the group unmerged would be exact but multiplies results; that trade-off is
/// recorded in `REVIEW-deferred.md` (item 1) if still present.
fn merge_by_padding(compositions: Vec<Composition>) -> Vec<Composition> {
	let mut merged: BTreeMap<Composition, Composition> = BTreeMap::new();

	for composition in compositions.into_iter() {
		let mut key: Composition = composition.clone();
		for literal in key.literals.iter_mut() {
			literal.pad_before = false;
			literal.pad_after = false;
		}

		match merged.get_mut(&key) {
			Some(existing) => {
				for (mine, theirs) in
					std::iter::zip(existing.literals.iter_mut(), composition.literals)
				{
					mine.pad_before |= theirs.pad_before;
					mine.pad_after |= theirs.pad_after;
				}
			},
			None => {
				merged.insert(key, composition);
			},
		}
	}

	Vec::from_iter(merged.into_values())
}
