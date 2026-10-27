//! A coarse model of a log shape.
//!
//! A log shape is a sequence of static text and references to rules (variables).
//! [`ShapeModel`] keeps that structure,
//! pairing each variable with a [`Charset`] bounding the characters the rule can emit,
//! plus the sub-rule(s) needed to report a capture.
//!
//! The model is built from [`ParsingSpec::split_log_shape`],
//! the same tokenizer [`ParsingSpec::automata_for_shape`] uses,
//! so the model and the automaton can never disagree about where the variables are.
//!
//! # Preconditions
//!
//! A shape is expected to be *decomposable*:
//! every variable must name a rule the spec defines,
//! and every such rule must be a **leaf** (no nested captures).
//! Both are asserted when the model is built,
//! so callers never have to ask whether a shape can be decomposed -- the type says it can.
//! Violations are programming errors in the shape, not conditions to recover from;
//! see [`ShapeModel::new`].

#[cfg(test)]
mod test;

use crate::parsing_spec::CaptureRef;
use crate::parsing_spec::LogShapeFragment;
use crate::parsing_spec::ParsingSpec;
use crate::parsing_spec::ResolvedCapture;
use crate::parsing_spec::RuleInfo;
use crate::regex::Regex;
use crate::search::decompose::Charset;

/// One token of a log shape.
#[derive(Clone, Debug)]
pub enum ShapePart {
	/// Fixed text, which a match must reproduce verbatim.
	Static(String),
	/// A reference to a rule, standing for any string that rule can match.
	Variable(Variable),
}

impl ShapePart {
	/// Whether this part is a rule reference.
	#[must_use]
	pub fn is_variable(&self) -> bool {
		matches!(self, Self::Variable(_))
	}

	/// Whether this part can emit nothing at all.
	///
	/// Static text is never empty (the tokenizer does not emit empty fragments), so only a nullable
	/// variable can.
	#[must_use]
	pub fn can_be_empty(&self) -> bool {
		match self {
			Self::Variable(variable) => variable.can_match_empty,
			Self::Static(text) => text.is_empty(),
		}
	}

	/// The rule name this part references, if it is a reference at all.
	#[must_use]
	pub fn variable_name(&self) -> Option<&str> {
		match self {
			Self::Variable(variable) => Some(&variable.name),
			Self::Static(_) => None,
		}
	}
}

/// A rule reference in a log shape.
#[derive(Clone, Debug)]
pub struct Variable {
	/// The name as written in the shape, e.g. `kv.key`.
	pub name: String,
	/// A superset of the characters any match of this variable can contain.
	pub charset: Charset,
	/// The alternatives this name resolves to, each with the capture to attribute a capture to.
	///
	/// A name can resolve to several rows of [`ParsingSpec::rules_for_name`]:
	/// several root rules may share a name (`%num%` with `num = [0-9]+` and
	/// `num = 0x[0-9a-f]+`), and a capture name may occur in several places of one rule
	/// (`%foo.bar%` with `(?<bar>[a-z]+)|(?<bar>[0-9]+)`).
	///
	/// Feasibility is decided over *all* of them -- [`Self::charset`] is their union,
	/// [`Self::can_match_empty`] their disjunction,
	/// and rule matching goes through `search_by_name`, which tries every row --
	/// but rendering attributes the value to the **first** only.
	/// Every row shares one `fully_qualified_name` (it is the shape's name, by construction),
	/// so the reported [`crate::search::LeafQuery`] is the same whichever is picked.
	/// Only the [`CaptureRef`] differs, and interpretations do not carry it;
	/// a consumer needing to know *which* alternative accepted a value cannot get it from
	/// the decomposer, whereas the engine's path does record the rule it traversed.
	/// Whether ambiguous names should instead be rejected, or reported once per accepting
	/// alternative, is undecided; see `REVIEW-deferred.md` (item 2) if still present.
	pub alternatives: Vec<ResolvedCapture>,
	/// Whether some alternative can match the empty string.
	///
	/// A nullable variable can stand entirely aside,
	/// so a part *before* it can still be the first thing a message emits,
	/// and a part *after* it can still be the last.
	/// Anchoring therefore cannot simply demand the first or last shape part;
	/// see [`ShapeModel::can_start_at`].
	pub can_match_empty: bool,
}

/// A log shape, modelled as a sequence of [`ShapePart`]s.
///
/// Every variable has been resolved and checked to be a leaf rule; see [`Self::new`].
#[derive(Clone, Debug)]
pub struct ShapeModel {
	pub parts: Vec<ShapePart>,
}

impl ShapeModel {
	/// Builds the model for `shape`.
	///
	/// # Panics
	///
	/// Panics if a variable names a rule the spec does not define,
	/// or names a rule with nested captures (i.e. one that is not a leaf).
	/// Both make the shape unsupported for direct decomposition,
	/// and continuing past them would silently produce a different answer than the engine --
	/// a non-leaf rule must be reported as its nested captures,
	/// which this model does not carry --
	/// so they are treated as errors in the shape,
	/// rather than as conditions to recover from.
	#[must_use]
	pub fn new(spec: &ParsingSpec, shape: &str) -> Self {
		let mut parts: Vec<ShapePart> = Vec::new();

		for fragment in spec.split_log_shape(shape).into_iter() {
			match fragment {
				LogShapeFragment::Text(text) => {
					parts.push(ShapePart::Static(text));
				},
				LogShapeFragment::Rule(name) => {
					parts.push(ShapePart::Variable(Variable::new(spec, name, shape)));
				},
			}
		}

		Self { parts }
	}

	/// The fragments for parts `start..=end`, for rebuilding a *slice* of the shape's automaton.
	///
	/// Round-trips through [`LogShapeFragment`] rather than re-tokenizing the shape string,
	/// so the parts the automaton is built from are exactly the parts the placement reasoned about.
	#[must_use]
	pub fn fragments_in(&self, start: usize, end: usize) -> Vec<LogShapeFragment> {
		Vec::from_iter(self.parts[start..=end].iter().map(|part| match part {
			ShapePart::Static(text) => LogShapeFragment::Text(text.clone()),
			ShapePart::Variable(variable) => LogShapeFragment::Rule(variable.name.clone()),
		}))
	}

	/// Whether a message of this shape can *begin* with the text part `part` emits.
	///
	/// True when every earlier part can emit nothing at all.
	/// Static text is never empty -- the tokenizer does not produce empty fragments --
	/// so only nullable variables can stand aside.
	/// This is what a start-anchored run needs:
	/// it must be the first thing in the message,
	/// which does not require it to be in the first *part* if the parts before it can vanish.
	#[must_use]
	pub fn can_start_at(&self, part: usize) -> bool {
		self.parts[..part].iter().all(ShapePart::can_be_empty)
	}

	/// Whether a message of this shape can *end* with the text part `part` emits.
	///
	/// The mirror of [`Self::can_start_at`]: every later part must be able to emit nothing.
	#[must_use]
	pub fn can_end_at(&self, part: usize) -> bool {
		self.parts[(part + 1)..].iter().all(ShapePart::can_be_empty)
	}

	/// The variables of this shape, in order.
	#[cfg(test)]
	pub fn variables(&self) -> impl Iterator<Item = &Variable> {
		self.parts.iter().filter_map(|part| match part {
			ShapePart::Variable(variable) => Some(variable),
			ShapePart::Static(_) => None,
		})
	}

	/// The number of variables.
	#[cfg(test)]
	#[must_use]
	pub fn num_variables(&self) -> usize {
		self.variables().count()
	}
}

impl Variable {
	/// Resolves `name` against `spec`.
	///
	/// # Panics
	///
	/// Panics if `name` resolves to no rule, or to a rule with nested captures. See
	/// [`ShapeModel::new`].
	#[must_use]
	fn new(spec: &ParsingSpec, name: String, shape: &str) -> Self {
		let rows: Vec<(&RuleInfo, &Regex)> = spec.rules_for_name(&name);
		assert!(
			!rows.is_empty(),
			"log shape {shape:?} references undefined rule {name:?}"
		);

		let mut charset: Charset = Charset::empty();
		let mut alternatives: Vec<ResolvedCapture> = Vec::with_capacity(rows.len());
		let mut can_match_empty: bool = false;

		for &(info, regex) in rows.iter() {
			charset.add_regex(regex);
			// Conservative in the direction that keeps anchoring sound:
			// if *any* alternative is nullable,
			// the variable is treated as able to stand aside.
			can_match_empty |= regex.is_nullable().is_some();

			assert!(
				info.is_leaf(),
				"log shape {shape:?} references non-leaf rule {name:?}: a rule with nested \
				 captures cannot be decomposed and must be referenced by one of its leaf captures \
				 instead (e.g. `%{name}.<capture>%`). Note this applies to log shapes; \
				 `search_by_name` accepts non-leaf names"
			);
			alternatives.push(ResolvedCapture {
				capture: CaptureRef {
					rule_idx: info.root_idx,
					capture_id: info.id,
				},
				fully_qualified_name: info.fully_qualified_name.clone(),
			});
		}

		Self {
			name,
			charset,
			alternatives,
			can_match_empty,
		}
	}
}
