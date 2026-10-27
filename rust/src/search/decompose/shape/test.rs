use super::*;
use crate::parsing_spec::ParsingSpecBuilder;

fn spec_with_rules(rules: &[(&str, &str)]) -> ParsingSpec {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	for &(name, pattern) in rules.iter() {
		builder.add_rule(name, pattern).unwrap();
	}
	builder.build()
}

fn leaf_spec() -> ParsingSpec {
	spec_with_rules(&[
		("level", "TRACE|DEBUG|INFO|WARN|ERROR"),
		("digits", "[0-9]+"),
	])
}

/// A compact rendering of the parts, for readable assertions.
fn describe(model: &ShapeModel) -> Vec<String> {
	model
		.parts
		.iter()
		.map(|part| match part {
			ShapePart::Static(text) => format!("text({text})"),
			ShapePart::Variable(variable) => format!("rule({})", variable.name),
		})
		.collect::<Vec<_>>()
}

#[test]
fn tokenizes_static_text_and_variables() {
	let spec: ParsingSpec = leaf_spec();
	let model: ShapeModel = ShapeModel::new(&spec, "hello %level% world");
	assert_eq!(
		vec!["text(hello )", "rule(level)", "text( world)"],
		describe(&model)
	);
	assert_eq!(1, model.num_variables());
}

#[test]
fn tokenization_matches_the_engines() {
	// The model must agree with `automata_for_shape` about where variables are; both go through
	// `split_log_shape`, so this pins the shared behaviour (including the `%%` escape).
	let spec: ParsingSpec = leaf_spec();
	for shape in [
		"%level%",
		"a%level%b",
		"100%% done",
		"%level%%digits%",
		"plain",
	] {
		let model: ShapeModel = ShapeModel::new(&spec, shape);
		let fragments: Vec<LogShapeFragment> = spec.split_log_shape(shape);
		assert_eq!(fragments.len(), model.parts.len(), "shape={shape:?}");
		for (fragment, part) in std::iter::zip(fragments.iter(), model.parts.iter()) {
			match (fragment, part) {
				(LogShapeFragment::Text(text), ShapePart::Static(modelled)) => {
					assert_eq!(text, modelled)
				},
				(LogShapeFragment::Rule(name), ShapePart::Variable(variable)) => {
					assert_eq!(name, &variable.name);
				},
				_ => panic!("mismatched fragment/part for shape={shape:?}"),
			}
		}
	}
}

#[test]
fn escaped_percent_is_static_text() {
	let spec: ParsingSpec = leaf_spec();
	let model: ShapeModel = ShapeModel::new(&spec, "100%% done");
	assert_eq!(0, model.num_variables());
}

#[test]
fn variable_charset_comes_from_the_rule() {
	let spec: ParsingSpec = leaf_spec();
	let model: ShapeModel = ShapeModel::new(&spec, "%digits%");
	let variable: &Variable = model.variables().next().unwrap();
	assert!(variable.charset.contains('0'));
	assert!(variable.charset.contains('9'));
	assert!(!variable.charset.contains('a'));
}

#[test]
#[should_panic(expected = "references undefined rule")]
fn unknown_rule_panics() {
	let spec: ParsingSpec = leaf_spec();
	// The shape is unsupported, not merely un-filterable, so building its model is an error.
	let _: ShapeModel = ShapeModel::new(&spec, "%nonexistent%");
}

#[test]
fn leaf_root_rule_resolves() {
	let spec: ParsingSpec = leaf_spec();
	let model: ShapeModel = ShapeModel::new(&spec, "x %level% y");

	// A capture-less root rule is reported under its own name,
	// matching `automata_for_shape`'s implicit whole-rule capture.
	let variable: &Variable = model.variables().next().unwrap();
	assert_eq!(1, variable.alternatives.len());
	assert_eq!("level", &*variable.alternatives[0].fully_qualified_name);
}

#[test]
#[should_panic(expected = "references non-leaf rule")]
fn rule_with_nested_captures_panics() {
	// `blockID` has captures, so it is not a leaf.
	// A decomposition would have to report `num`/`gen`,
	// which this model does not carry, so the shape must name a leaf capture instead.
	let spec: ParsingSpec = spec_with_rules(&[("blockID", r"blk_(?<num>[0-9]+)_(?<gen>[0-9]+)")]);
	let _: ShapeModel = ShapeModel::new(&spec, "%blockID%");
}

#[test]
fn leaf_sub_rule_reference_resolves() {
	// Referencing the leaf capture directly is supported.
	let spec: ParsingSpec = spec_with_rules(&[("blockID", r"blk_(?<num>[0-9]+)_(?<gen>[0-9]+)")]);
	let model: ShapeModel = ShapeModel::new(&spec, "%blockID.num%");
	let variable: &Variable = model.variables().next().unwrap();
	assert_eq!(
		"blockID.num",
		&*variable.alternatives[0].fully_qualified_name
	);
	assert!(variable.charset.contains('7'));
	assert!(!variable.charset.contains('b'));
}

#[test]
fn variableless_shape_is_modelled() {
	// A literal-only shape has no variables, but it is still a valid model:
	// its static text is exactly what a query must reproduce,
	// so placement and composition handle it like any other.
	let spec: ParsingSpec = leaf_spec();
	let model: ShapeModel = ShapeModel::new(&spec, "plain literal text");
	assert_eq!(0, model.num_variables());
}
