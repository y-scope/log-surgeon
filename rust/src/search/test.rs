use super::*;
use crate::parsing_spec::ParsingSpecBuilder;

#[test]
fn search_email() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder
		.add_rule("email", r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)")
		.unwrap();

	let spec: ParsingSpec = builder.build();

	{
		let interpretations: Vec<Interpretation> =
			do_search_by_name(&spec, "*a*@*mail*example*", "email");
		println!("===");

		for i in interpretations.iter() {
			println!("- {i:?}");
		}
	}
}

#[test]
fn search_block_id() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder
		.add_rule("block_id", r"blk_(?<blockNum>[0-9]+)_(?<genStamp>[0-9]+)")
		.unwrap();

	let spec: ParsingSpec = builder.build();

	{
		let interpretations: Vec<Interpretation> =
			do_search_by_name(&spec, "*blk*_566*", "block_id");
		println!("===");

		for i in interpretations.iter() {
			println!("- {i:?}");
		}

		assert_eq!(interpretations.len(), 2);
		assert_eq!(interpretations[0].leaf_queries[0].string_value, "blk_");
		assert_eq!(interpretations[0].leaf_queries[1].string_value, "566*");
		assert_eq!(
			&*interpretations[0].leaf_queries[1].fully_qualified_name,
			"block_id.blockNum"
		);
		assert_eq!(interpretations[0].leaf_queries[2].string_value, "*");
		assert_eq!(interpretations[0].leaf_queries[3].string_value, "*");
		assert_eq!(
			&*interpretations[0].leaf_queries[3].fully_qualified_name,
			"block_id.genStamp"
		);

		assert_eq!(interpretations[1].leaf_queries[0].string_value, "blk*");
		assert_eq!(interpretations[1].leaf_queries[1].string_value, "*");
		assert_eq!(
			&*interpretations[1].leaf_queries[1].fully_qualified_name,
			"block_id.blockNum"
		);
		assert_eq!(interpretations[1].leaf_queries[2].string_value, "_");
		assert_eq!(interpretations[1].leaf_queries[3].string_value, "566*");
		assert_eq!(
			&*interpretations[1].leaf_queries[3].fully_qualified_name,
			"block_id.genStamp"
		);
	}
}

#[test]
fn search_nested_name_without_leaf_capture() {
	let spec: ParsingSpec = spec! {
		r#"
		foo: "_(?<bar>[a-z]+|(?<baz>[0-9]+))_"
		"#
	};

	{
		let interpretations: Vec<Interpretation> = do_search_by_name(&spec, "_a*b_", "foo");
		println!("===");

		for i in interpretations.iter() {
			println!("- {i:?}");
		}

		assert_eq!(interpretations.len(), 1);
		assert_eq!(interpretations[0].leaf_queries[0].string_value, "_a*b_");
		assert_eq!(
			&*interpretations[0].leaf_queries[0].fully_qualified_name,
			"foo"
		);
	}

	{
		let interpretations: Vec<Interpretation> = do_search_by_name(&spec, "a*b", "foo.bar");
		println!("===");

		for i in interpretations.iter() {
			println!("- {i:?}");
		}

		assert_eq!(interpretations.len(), 2);
		assert_eq!(interpretations[0].leaf_queries[0].string_value, "*a*b*");
		assert_eq!(
			&*interpretations[0].leaf_queries[0].fully_qualified_name,
			""
		);
		assert_eq!(interpretations[1].leaf_queries[0].string_value, "*a*b*");
		assert_eq!(
			&*interpretations[1].leaf_queries[0].fully_qualified_name,
			"foo"
		);
	}

	{
		let interpretations: Vec<Interpretation> = do_search_by_name(&spec, "0*1", "foo.bar.baz");
		println!("===");

		for i in interpretations.iter() {
			println!("- {i:?}");
		}

		assert_eq!(interpretations.len(), 1);
		assert_eq!(interpretations[0].leaf_queries[0].string_value, "0*1");
		assert_eq!(
			&*interpretations[0].leaf_queries[0].fully_qualified_name,
			"foo.bar.baz"
		);
	}
}

#[test]
fn test_covers() {
	let a: LeafQuery =
		LeafQuery::new_static_text(vec![SymbolicChar::Literal('a'), SymbolicChar::GlobStar]);
	let b: LeafQuery = LeafQuery::new_static_text(vec![SymbolicChar::Literal('a')]);
	let c: LeafQuery =
		LeafQuery::new_static_text(vec![SymbolicChar::GlobStar, SymbolicChar::Literal('a')]);
	let d: LeafQuery = LeafQuery::new_static_text(vec![SymbolicChar::Literal('a')]);
	let e: LeafQuery = LeafQuery::new_static_text(vec![SymbolicChar::GlobStar]);
	let f: LeafQuery = LeafQuery::new_static_text(vec![
		SymbolicChar::GlobStar,
		SymbolicChar::Literal('a'),
		SymbolicChar::GlobStar,
	]);

	assert!(a.covers(&b));
	assert!(!b.covers(&a));

	assert!(c.covers(&d));
	assert!(!d.covers(&e));

	assert!(!a.covers(&c));
	assert!(!c.covers(&a));

	assert!(a.covers(&a));
	assert!(b.covers(&b));
	assert!(c.covers(&c));
	assert!(d.covers(&d));
	assert!(e.covers(&e));

	assert!(e.covers(&a));
	assert!(e.covers(&b));
	assert!(e.covers(&c));
	assert!(e.covers(&d));
	assert!(e.covers(&f));
}

/// Whether the glob `pattern` matches the whole of `text`.
fn glob_matches(pattern: &[SymbolicChar], text: &[char]) -> bool {
	match pattern.first() {
		None => text.is_empty(),
		Some(SymbolicChar::GlobStar) => {
			(0..=text.len()).any(|skip| glob_matches(&pattern[1..], &text[skip..]))
		},
		Some(&SymbolicChar::Literal(expected)) => {
			!text.is_empty() && (text[0] == expected) && glob_matches(&pattern[1..], &text[1..])
		},
	}
}

/// Every value up to `length` symbols over `alphabet` and `*`,
/// including ones with adjacent wildcards.
fn all_values(alphabet: &[char], length: usize) -> Vec<Vec<SymbolicChar>> {
	let mut found: Vec<Vec<SymbolicChar>> = vec![Vec::new()];
	let mut frontier: Vec<Vec<SymbolicChar>> = vec![Vec::new()];

	for _ in 0..length {
		let mut next: Vec<Vec<SymbolicChar>> = Vec::new();
		for value in frontier.iter() {
			next.push([value.as_slice(), &[SymbolicChar::GlobStar]].concat());
			for &character in alphabet.iter() {
				next.push([value.as_slice(), &[SymbolicChar::Literal(character)]].concat());
			}
		}
		found.extend(next.iter().cloned());
		frontier = next;
	}

	found
}

/// `covers` is exactly glob containment: it holds iff every word matching the specific
/// value matches the general one.
///
/// Values are over `{a, b}` and words over `{a, b, c}`, so `c` serves as the fresh symbol:
/// a failed containment always has a counterexample of at most the specific value's length
/// (each of its `*` replaced by `c`), which the word bound exceeds.
#[test]
fn covers_is_exactly_glob_containment() {
	let values: Vec<Vec<SymbolicChar>> = all_values(&['a', 'b'], 4);

	let mut words: Vec<Vec<char>> = vec![Vec::new()];
	let mut frontier: Vec<Vec<char>> = vec![Vec::new()];
	for _ in 0..5 {
		let mut next: Vec<Vec<char>> = Vec::new();
		for word in frontier.iter() {
			for character in ['a', 'b', 'c'] {
				next.push([word.as_slice(), &[character]].concat());
			}
		}
		words.extend(next.iter().cloned());
		frontier = next;
	}

	let languages: Vec<Vec<bool>> = Vec::from_iter(
		values
			.iter()
			.map(|value| Vec::from_iter(words.iter().map(|word| glob_matches(value, word)))),
	);

	let render = |value: &[SymbolicChar]| String::from_iter(value.iter().map(ToString::to_string));
	let mut covering: usize = 0;
	for (general, general_language) in values.iter().zip(languages.iter()) {
		for (specific, specific_language) in values.iter().zip(languages.iter()) {
			let contained: bool = std::iter::zip(specific_language, general_language)
				.all(|(&in_specific, &in_general)| !in_specific || in_general);
			let covers: bool = LeafQuery::new_static_text(general.clone())
				.covers(&LeafQuery::new_static_text(specific.clone()));
			assert_eq!(
				contained,
				covers,
				"{:?} against {:?}",
				render(general),
				render(specific),
			);
			covering += usize::from(covers);
		}
	}
	assert!(0 < covering, "expected some pair to be covered");
}

/// Containments a segment-by-segment comparison would miss.
#[test]
fn covers_sees_through_misaligned_wildcards() {
	let value = |text: &str| {
		LeafQuery::new_static_text(Vec::from_iter(text.chars().map(|character| {
			if '*' == character {
				SymbolicChar::GlobStar
			} else {
				SymbolicChar::Literal(character)
			}
		})))
	};
	assert!(value("aa*").covers(&value("aaa*")));
	assert!(value("*a*").covers(&value("b*ab")));
	assert!(value("a*").covers(&value("a**")));
	assert!(value("a**").covers(&value("a*")));
	assert!(!value("aa*").covers(&value("a*a")));
	assert!(!value("").covers(&value("*")));
	assert!(value("*").covers(&value("")));
}

/// `covers` is reflexive and transitive, which is what makes
/// `dedup_covered_interpretations` reach a fixpoint.
#[test]
fn covers_is_a_partial_order() {
	let values: Vec<LeafQuery> = Vec::from_iter(
		all_values(&['a', 'b'], 3)
			.into_iter()
			.map(LeafQuery::new_static_text),
	);

	for value in values.iter() {
		assert!(
			value.covers(value),
			"not reflexive: {:?}",
			value.string_value
		);
	}

	for general in values.iter() {
		for middle in values.iter().filter(|middle| general.covers(middle)) {
			for specific in values.iter().filter(|specific| middle.covers(specific)) {
				assert!(
					general.covers(specific),
					"not transitive: {:?} > {:?} > {:?}",
					general.string_value,
					middle.string_value,
					specific.string_value,
				);
			}
		}
	}
}

#[test]
fn full_log_search() {
	let mut builder: ParsingSpecBuilder = ParsingSpecBuilder::new();
	builder
		.add_rule("email", r"(?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+)")
		.unwrap();

	let spec: ParsingSpec = builder.build();

	let interpretations: Vec<Interpretation> =
		do_full_search(&spec, "a@com*", "hello a@com.example");

	println!("=== Interpretations");
	for interpretation in interpretations.iter() {
		println!("- {interpretation:?}");
	}
}

#[test]
fn kv_ip_pattern() {
	let spec: ParsingSpec = spec! {
		r#"
		kv: "(?<key>[\w_./\\-]+)([:=]|: )(?<ip_value>/?\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(:\d{1,5})?)"
		"#
	};

	let interpretations: Vec<Interpretation> = do_full_search(
		&spec,
		"*172.31.17.135*",
		"hello %kv.key%: %kv.ip_value% world",
	);

	println!("=== Interpretations");
	for interpretation in interpretations.iter() {
		println!("- {interpretation:?}");
	}

	// assert_eq!(interpretations.len(), 2);
}

fn do_full_search(spec: &ParsingSpec, query: &str, shape: &str) -> Vec<Interpretation> {
	let query: SearchString = SearchString::parse(query).unwrap();

	let mut interpretations: Vec<Vec<Interpretation>> = query.search_by_log_shapes(&spec, &[shape]);

	interpretations.pop().unwrap()
}

fn do_search_by_name(spec: &ParsingSpec, query: &str, name: &str) -> Vec<Interpretation> {
	let query: SearchString = SearchString::parse(query).unwrap();

	let interpretations: Vec<Interpretation> = query.search_by_name(&spec, name);

	interpretations
}
