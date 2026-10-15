pub mod decompose;
mod interpretation;
mod query;

#[cfg(test)]
mod test;

pub use interpretation::Interpretation;
pub use interpretation::LeafQuery;
pub(crate) use interpretation::Tail;
pub use query::SearchString;
pub use query::SearchStringError;
pub use query::SymbolicChar;

use std::sync::Arc;

use crate::nfa::Path;
use crate::nfa::PathComponent;
use crate::nfa::Tnfa;
use crate::parsing_spec::LogShapeFragment;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::RootRule;
use crate::parsing_spec::RuleInfo;
use crate::regex::Regex;
use crate::search::decompose::ComposeBudget;
use crate::search::decompose::Composed;
use crate::search::decompose::PlacementTable;
use crate::search::decompose::Query;
use crate::search::decompose::RunFitCache;
use crate::search::decompose::ShapeModel;
use crate::search::decompose::ShapeModelCache;

/// A shape's automaton, possibly cut short, with what that cut left out.
///
/// See [`SearchString::truncated_automata`]. The tail is what lets the rendering stay
/// identical to the un-truncated shape's: dropped static text still has to appear as a
/// trailing `'*'`, because it is text the message contains even though the query does not
/// constrain it.
struct TruncatedShape {
	automata: Tnfa,
	/// What the dropped parts (if any) are made of; [`Tail::Empty`] for a complete shape.
	dropped: Tail,
}

impl SearchString {
	pub fn search_by_name(&self, spec: &ParsingSpec, name: &str) -> Vec<Interpretation> {
		if name.is_empty() {
			return Vec::new();
		}

		let rows: Vec<(&RuleInfo, &Regex)> = spec.rules_for_name(name);

		self.interpretations_for_name(spec, &rows)
	}

	/// Shape models are cached on `spec` (see [`ParsingSpec::shape_models`]), so searching the
	/// same shapes more than once reuses each built model instead of rebuilding it per query.
	pub fn search_by_log_shapes(
		&self,
		spec: &ParsingSpec,
		log_shapes: &[&str],
	) -> Vec<Vec<Interpretation>> {
		let cache: &ShapeModelCache = spec.shape_models();

		// The query's runs and their fits are shared across every shape:
		// this is where the composition path gets its leverage,
		// since a corpus mentions the same few rules over and over,
		// and a rule's behaviour on a run does not depend on the shape referencing it.
		let query: Query<'_> = Query::new(self);
		let fits: RunFitCache = RunFitCache::new();

		// Resolve every shape before searching any, so an unsupported shape fails the call outright
		// rather than after some shapes have already been processed. See [`ShapeModel::new`].
		let models: Vec<Arc<ShapeModel>> =
			Vec::from_iter(log_shapes.iter().map(|&shape| cache.get(spec, shape)));

		std::iter::zip(&models, log_shapes)
			.map(|(model, &shape)| {
				self.interpretations_for_log_shape(spec, model, shape, &query, &fits)
			})
			.collect::<Vec<_>>()
	}

	/// Interpretations for `log_shape`, always via the automata,
	/// bypassing [`crate::search::decompose`].
	///
	/// Exposed so that tests can pin [`decompose`] against the engine it is meant to agree with.
	pub fn interpretations_for_log_shape_via_engine(
		&self,
		spec: &ParsingSpec,
		log_shape: &str,
	) -> Vec<Interpretation> {
		let model: ShapeModel = ShapeModel::new(spec, log_shape);
		let automata: Tnfa = Self::automata_for_prefix(spec, &model, model.parts.len() - 1);
		self.interpretations_for_automata(spec, &automata, &[])
	}

	/// Interpretations for an already-built shape automaton.
	///
	/// Exposed so that tests can pin a *truncated* automaton (see
	/// [`ParsingSpec::automata_for_fragments`]) against the full one it stands in for.
	///
	/// `dropped` are the parts the automaton is missing from the shape's end, if any.
	/// It is needed because dropped static text is still reported -- as a trailing `'*'` --
	/// by the full shape, so a truncated one has to put it back to render identically.
	/// Pass an empty slice for a complete shape.
	pub fn interpretations_for_automata(
		&self,
		spec: &ParsingSpec,
		automata: &Tnfa,
		dropped: &[decompose::ShapePart],
	) -> Vec<Interpretation> {
		self.interpretations_for_shape(spec, automata, Tail::of_parts(dropped))
	}

	/// Builds the automaton for shape parts `0..=end`, round-tripping through the model's
	/// fragments so the parts built are exactly the parts placement reasoned about.
	///
	/// A `ShapeModel` already resolved every variable (see [`ShapeModel::new`]),
	/// so building from its fragments cannot fail.
	fn automata_for_prefix(spec: &ParsingSpec, model: &ShapeModel, end: usize) -> Tnfa {
		let fragments: Vec<LogShapeFragment> = model.fragments_in(0, end);
		spec.automata_for_fragments(&fragments)
			.expect("the model already resolved every rule in the shape")
	}
}

impl SearchString {
	/// This query as a regex, symbol for symbol: a wildcard is `.*`.
	///
	/// The intersection always runs to the end of the shape,
	/// so "and then anything" is something the query itself consumes.
	fn to_regex(&self) -> Regex {
		Regex::Sequence(Vec::from_iter(
			self.symbols.iter().map(SymbolicChar::to_regex),
		))
	}

	/// Interpretations of this query for a single log shape.
	///
	/// Three paths, cheapest first:
	///
	/// 1. [`crate::search::decompose::can_match`] on the coarse charset model.
	///    When it proves the shape cannot match,
	///    the shape's TNFA is never built and never intersected with the query.
	/// 2. [`crate::search::decompose::compose`],
	///    which decomposes the query against the shape directly,
	///    reusing per-rule simulations across shapes.
	///    This answers the real question -- which rule instance holds what text --
	///    without building any automata for the shape.
	/// 3. The engine, when composition declines to conclude (an unreasonable rule, or an exhausted
	///    budget). This is the slow path that the other two exist to avoid.
	///
	/// Composition is exact because it simulates the rules themselves,
	/// whereas the prefilter's `can_match` sees only their coarse charsets
	/// (and variables there may match the empty string, which a rule like `[a-z]+` cannot),
	/// so only the prefilter's "no" is a proof.
	fn interpretations_for_log_shape(
		&self,
		spec: &ParsingSpec,
		model: &ShapeModel,
		shape: &str,
		query: &Query<'_>,
		fits: &RunFitCache,
	) -> Vec<Interpretation> {
		now!(t0);

		if !decompose::can_match(model, query.symbols) {
			trace!("decompose rejected shape {shape:.256}");
			return Vec::new();
		}

		// `None` means a resource cap was exceeded, so the table is incomplete and nothing may be
		// concluded from it; fall back to the engine.
		let maybe_table: Option<PlacementTable> = PlacementTable::compute(spec, model, query, fits);

		if let Some(table) = maybe_table.as_ref()
			&& let Some(interpretations) =
				self.composed_interpretations(spec, model, table, query, fits)
		{
			now!(t1);
			trace!("composed shape in {} ms {shape:.256}", millis!(t0, t1));
			return interpretations;
		}

		// Composition declined to conclude, so build the shape's automaton and use the engine.
		// The table still bounds where the query's literal text can sit,
		// which is what decides how much of the shape has to be built;
		// see [`Self::truncated_automata`].
		let truncated: TruncatedShape = self.truncated_automata(spec, model, query, &maybe_table);
		let interpretations: Vec<Interpretation> =
			self.interpretations_for_shape(spec, &truncated.automata, truncated.dropped);
		now!(t1);
		debug!("- took {} ms", millis!(t0, t1));
		interpretations
	}

	/// The shape's automaton, truncated to the parts the query can actually reach.
	///
	/// Real shapes carry very long tails of static text -- tens of thousands of characters --
	/// while their variables cluster near the front,
	/// and one state is emitted per literal character.
	/// A query that is not anchored at the end ends in a wildcard,
	/// and that wildcard consumes everything past its last literal run,
	/// so those trailing states are built and intersected only to be matched by `.*`.
	///
	/// [`PlacementTable::last_reachable_part`] bounds every placement of every run, so no literal
	/// character of the query can land beyond it. Truncating there is exactly "do not simulate the
	/// query's trailing wildcard": the parts dropped are the ones it would have consumed.
	/// Nothing is truncated when the query is anchored at the end (it must consume the shape
	/// through to its end), when the last part is reachable anyway, or when the table itself
	/// was incomplete and so does not bound the reach.
	///
	/// Only the *tail* is dropped. The head must be kept verbatim --
	/// the query's leading wildcard still has to traverse it,
	/// and replacing it with `.*` would be a superset,
	/// letting runs straddle where the real static text forbids it
	/// and inventing interpretations the full shape does not have.
	///
	/// The automaton for the kept prefix is built from the model's own fragments,
	/// never from the shape string,
	/// so the parts built are exactly the parts placement reasoned about
	/// (see [`SearchString::automata_for_prefix`]).
	fn truncated_automata(
		&self,
		spec: &ParsingSpec,
		model: &ShapeModel,
		query: &Query<'_>,
		maybe_table: &Option<PlacementTable>,
	) -> TruncatedShape {
		let last: usize = model.parts.len() - 1;

		let end: usize = if query.anchored_end {
			last
		} else {
			maybe_table
				.as_ref()
				.and_then(PlacementTable::last_reachable_part)
				.map_or(last, |end| end.min(last))
		};

		if end < last {
			trace!(
				"truncated shape from {} parts to 0..={end}",
				model.parts.len()
			);
		}

		TruncatedShape {
			automata: SearchString::automata_for_prefix(spec, model, end),
			// Dropped static text is still reported by the full shape, as a trailing `'*'` --
			// it is text the message contains even though the query says nothing about it --
			// so the truncated rendering has to put that back.
			// A tail of only rule references needs nothing:
			// those are unconstrained captures, which are omitted either way.
			dropped: Tail::of_parts(&model.parts[(end + 1)..]),
		}
	}

	/// Interpretations for `model` via [`crate::search::decompose::compose`].
	///
	/// `None` means composition declined to conclude and the caller must fall back to the engine.
	/// An empty vector is a real answer -- the shape cannot match -- and is not the same thing.
	fn composed_interpretations(
		&self,
		spec: &ParsingSpec,
		model: &ShapeModel,
		table: &PlacementTable,
		query: &Query<'_>,
		fits: &RunFitCache,
	) -> Option<Vec<Interpretation>> {
		match decompose::compose(spec, model, table, query, fits, ComposeBudget::default()) {
			Composed::Impossible => Some(Vec::new()),
			Composed::Unknown => None,
			Composed::Compositions(compositions) => {
				let mut interpretations: Vec<Interpretation> =
					Vec::from_iter(compositions.iter().map(|composition| {
						composition.to_interpretation(spec, model, query, fits)
					}));
				// The engine's callers expect a canonical, duplicate-free set;
				// distinct compositions can render identically once positions collapse
				// to the same sub-queries.
				interpretations.sort();
				interpretations.dedup();
				Some(interpretations)
			},
		}
	}

	fn interpretations_for_name(
		&self,
		spec: &ParsingSpec,
		rows: &[(&RuleInfo, &Regex)],
	) -> Vec<Interpretation> {
		let mut interpretations: Vec<Interpretation> = Vec::new();

		for &(rule_info, regex) in rows.iter() {
			let rule_nfa: Tnfa = Tnfa::for_single_rule(rule_info.root_idx, regex, &[]);

			let potential_interpretations: Vec<Interpretation> =
				self.interpretations_for_nfa(spec, &rule_nfa, rule_info);

			interpretations.extend(potential_interpretations.into_iter());
		}

		// Each row is deduplicated by covering on its own; rows of one name can still
		// produce interpretations that cover each other, so do it once more across rows.
		Interpretation::dedup_covered_interpretations(&mut interpretations);

		interpretations
	}

	/// Interpretations of this query against a shape's automaton.
	///
	/// The intersection always runs **to the end**:
	/// a state accepts only when the shape's automaton and the query's accept together.
	/// End-anchoring is therefore not a mode here --
	/// the query is used exactly as written,
	/// and a trailing wildcard (`*foo*`, `foo*`) is a real `.*` the intersection consumes.
	///
	/// That wildcard means the automaton must traverse whatever remains of the shape,
	/// which is why `shape_nfa` should already have been cut down to the part the query can reach;
	/// see [`Self::truncated_automata`].
	/// That division of labour is the point:
	/// deciding *how much shape matters* belongs to placement,
	/// which knows where the query's text can sit, not to the intersection,
	/// which would otherwise have to guess by stopping early.
	///
	/// `dropped` is what is missing from the end of `shape_nfa`, if it was truncated;
	/// see [`TruncatedShape`].
	fn interpretations_for_shape(
		&self,
		spec: &ParsingSpec,
		shape_nfa: &Tnfa,
		dropped: Tail,
	) -> Vec<Interpretation> {
		let anchored_end: bool = self.anchored_end();

		let mut interpretations: Vec<Interpretation> = Vec::new();

		let search_nfa: Tnfa = Tnfa::for_regex(&self.to_regex());

		now!(t0);
		let intersection: Tnfa = shape_nfa.intersect::<true>(&search_nfa);
		now!(t1);
		trace!(
			"intersecting {} states with {} states took {}",
			shape_nfa.states.len(),
			search_nfa.states.len(),
			t1.duration_since(t0).as_millis()
		);
		if intersection.definitely_cannot_accept() {
			return Vec::new();
		}

		let paths: Vec<Path> = intersection.compute_paths(spec);

		for path in paths.iter() {
			let mut leaf_queries: Vec<LeafQuery> = LeafQuery::from_path(path);

			if !anchored_end {
				// An unanchored query says nothing about the tail of the message,
				// so every rule reference after its last literal character is
				// unconstrained and is dropped (composition omits exactly these).
				let trimmed: Tail = Self::truncate_to_last_constrained(&mut leaf_queries);
				// What was trimmed, together with what truncation dropped from the
				// shape's automaton, is the tail the rendering must still account for.
				(trimmed.or(dropped)).finish_unanchored(&mut leaf_queries);
			}

			interpretations.push(Interpretation { leaf_queries });
		}

		interpretations.iter().for_each(Interpretation::invariants);

		Interpretation::dedup_covered_interpretations(&mut interpretations);

		interpretations
	}

	/// Drops the sub-queries past the last one the query constrains, and classifies them.
	///
	/// A capture past the last constrained sub-query is vacuous (the query says nothing
	/// about it); a static one is kept as-is to the left of `last_constrained` only when
	/// it sits between two constrained parts. Trailing static text is reported by
	/// [`Tail::finish_unanchored`], so it is classified here rather than kept.
	///
	/// Positional identity is unharmed because it is read left to right --
	/// the first `n` sub-queries still correspond to the first `n` shape parts --
	/// so only trailing entries may go,
	/// and a *leading* or *interior* vacuous capture is always retained.
	fn truncate_to_last_constrained(leaf_queries: &mut Vec<LeafQuery>) -> Tail {
		let last_constrained: Option<usize> = leaf_queries.iter().rposition(|leaf_query| {
			leaf_query
				.symbolic_value
				.iter()
				.any(|symbol| !symbol.is_wildcard())
		});

		let Some(last) = last_constrained else {
			let tail: Tail = Tail::of_leaves(leaf_queries);
			// A path that constrains nothing describes a single unrestricted wildcard.
			leaf_queries.clear();
			leaf_queries.push(LeafQuery::new_static_text(vec![SymbolicChar::GlobStar]));
			return tail;
		};

		let tail: Tail = Tail::of_leaves(&leaf_queries[(last + 1)..]);
		leaf_queries.truncate(last + 1);
		tail
	}

	fn interpretations_for_nfa(
		&self,
		spec: &ParsingSpec,
		nfa: &Tnfa,
		rule_info: &RuleInfo,
	) -> Vec<Interpretation> {
		let mut interpretations: Vec<Interpretation> = Vec::new();

		let search_nfa: Tnfa = Tnfa::for_regex(&self.to_regex());

		let intersection: Tnfa = nfa.intersect::<true>(&search_nfa);

		let paths: Vec<Path> = intersection.compute_paths(spec);

		for path in paths.iter() {
			if let [PathComponent::Literal(contents)] = path.components.as_slice() {
				let rule: &RootRule = &spec[path.rule_idx];
				assert_eq!(rule_info.root_idx, rule.idx);

				let mut implicit_capture: LeafQuery =
					LeafQuery::new_rule(rule.name.clone(), contents.clone());

				if !rule_info.is_root() {
					assert!(!rule_info.is_leaf());
					let mut static_text: LeafQuery = LeafQuery::new_static_text(contents.clone());

					implicit_capture.surround_with_wildcards();
					static_text.surround_with_wildcards();

					interpretations.push(Interpretation {
						leaf_queries: vec![implicit_capture],
					});
					interpretations.push(Interpretation {
						leaf_queries: vec![static_text],
					});
				} else {
					interpretations.push(Interpretation {
						leaf_queries: vec![implicit_capture],
					});
				}

				continue;
			}

			interpretations.push(Interpretation {
				leaf_queries: LeafQuery::from_path(path),
			});
		}

		interpretations.iter().for_each(Interpretation::invariants);

		Interpretation::dedup_covered_interpretations(&mut interpretations);

		interpretations
	}
}
