use super::*;
use crate::search::SymbolicChar;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SimulationToken {
	pub maybe_capture: Option<RegexCapture>,
	pub value: Vec<SymbolicChar>,
}

#[derive(Debug, Eq, PartialEq)]
struct Derivative {
	maybe_capture: Option<RegexCapture>,
	next: Regex,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum DerivativeChar {
	Char(SymbolicChar),
	Derivative(RegexCapture),
}

impl std::fmt::Debug for DerivativeChar {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			&Self::Char(ch) => ch.fmt(fmt),
			Self::Derivative(capture) => {
				if !capture.close {
					fmt.write_fmt(format_args!("(?<{}>)", capture.qualified_name))
				} else {
					fmt.write_fmt(format_args!("(<{}>?)", capture.qualified_name))
				}
			},
		}
	}
}
/// Alternation.
/// We use `|` instead of `+` to avoid confusion.
impl std::ops::BitOr for Regex {
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self::Output {
		match self {
			Self::Alternation(mut items) => {
				if let Self::Alternation(rhs) = rhs {
					items.extend(rhs.into_iter());
				} else {
					items.push(rhs);
				}
				return Self::Alternation(items);
			},
			_ => (),
		}
		Self::Alternation(vec![self, rhs])
	}
}

/// Concatenation.
/// While it's arguably more intuitive to concatenate with addition,
/// formally, multiplication is more appropriate;
/// it distributes over multiplication: `a(b or c) == ab or ac`.
impl std::ops::Mul for Regex {
	type Output = Self;

	fn mul(self, rhs: Self) -> Self::Output {
		match self {
			Self::Sequence(mut items) => {
				if let Self::Sequence(rhs) = rhs {
					items.extend(rhs.into_iter());
				} else {
					items.push(rhs);
				}
				Self::Sequence(items)
			},
			Self::Alternation(mut items) => {
				if items.len() == 1 {
					return items.pop().unwrap() * rhs;
				} else {
					Self::Sequence(vec![Self::Alternation(items), rhs])
				}
			},
			_ => Self::Sequence(vec![self, rhs]),
		}
	}
}

impl Regex {
	pub fn simulate(&self, input: &[SymbolicChar]) -> Vec<Vec<SimulationToken>> {
		let regex: Self = Self::Sequence(vec![self.clone(), Self::AnyChar]);

		let mut all_paths: Vec<Vec<DerivativeChar>> = regex.simulate_internal(input);
		all_paths.sort();
		all_paths.dedup();

		let mut interpretations: Vec<Vec<SimulationToken>> = Vec::new();

		for path in all_paths.into_iter() {
			let mut tokens: Vec<SimulationToken> = Vec::new();
			let mut value: Vec<SymbolicChar> = Vec::new();
			let mut maybe_start: Option<RegexCapture> = None;

			for ch in path.into_iter() {
				match ch {
					DerivativeChar::Char(ch) => {
						value.push(ch);
					},
					DerivativeChar::Derivative(capture) => {
						if !capture.close {
							if !value.is_empty() {
								tokens.push(SimulationToken {
									maybe_capture: None,
									value: std::mem::replace(&mut value, Vec::new()),
								});
							}
							maybe_start = Some(capture);
						} else {
							tokens.push(SimulationToken {
								maybe_capture: Some(maybe_start.take().unwrap()),
								value: std::mem::replace(&mut value, Vec::new()),
							});
						}
					},
				}
			}
			if !value.is_empty() {
				tokens.push(SimulationToken {
					maybe_capture: None,
					value,
				});
			}

			assert!(maybe_start.is_none());

			tokens.pop().unwrap();
			interpretations.push(tokens);
		}

		// let mut interpretations: Vec<Vec<(RegexCapture, usize, usize)>> = Vec::with_capacity(all_paths.len());
		// let mut stacks: Vec<Vec<usize>> = vec![Vec::new(); 1 + self.count_captures()];
		// for path in all_paths.iter() {
		// 	stacks.iter_mut().for_each(Vec::clear);
		// 	let mut leaves: Vec<(RegexCapture, usize, usize)> = Vec::new();
		// 	for (capture, pos) in path.iter() {
		// 		if !capture.is_leaf() {
		// 			continue;
		// 		}
		// 		let stack: &mut Vec<usize> = &mut stacks[capture.id.get() as usize];
		// 		if capture.close {
		// 			let start: usize = stack.pop().unwrap();
		// 			leaves.push((capture.clone(), start, *pos));
		// 		} else {
		// 			stack.push(*pos);
		// 		}
		// 	}
		// 	interpretations.push(leaves);
		// }

		// interpretations.retain(|path| path.iter().all(|(_, start, end)| start < end));
		interpretations.sort();
		interpretations.dedup();

		interpretations
	}

	fn simulate_internal(&self, input: &[SymbolicChar]) -> Vec<Vec<DerivativeChar>> {
		let mut completed_paths: Vec<Vec<DerivativeChar>> = Vec::new();
		let mut queue: Vec<(Regex, usize, Vec<DerivativeChar>)> = vec![(self.clone(), 0, Vec::new())];

		while let Some((regex, i, path)) = queue.pop() {
			if i > input.len() {
				if regex.allows_epsilon_string() {
					completed_paths.push(path);
				}
				continue;
			}
			// // Anchor - any character can be used.
			let ch: SymbolicChar = input.get(i).copied().unwrap_or(SymbolicChar::Literal('$'));
			// let Some(ch): Option<SymbolicChar> = input.get(i).copied() else {
			// 	if regex.allows_epsilon_string() {
			// 		completed_paths.push(path);
			// 	}
			// 	continue;
			// };
			if ch.is_wildcard() {
				queue.push((regex.clone(), i + 1, path.clone()));
			}
			if i == 1 {
				println!("- char {ch:?}, path {path:?}, remaining {regex:?}");
			}
			for derivative in regex.apply_derivative(ch).into_iter().rev() {
				if derivative.next.is_empty_set() {
					continue;
				}
				if let Some(capture) = derivative.maybe_capture {
					let mut path: Vec<DerivativeChar> = path.clone();
					path.push(DerivativeChar::Derivative(capture.clone()));
					queue.push((derivative.next, i, path));
				} else {
					let advance: usize = if ch == SymbolicChar::WildcardStar { 0 } else { 1 };
					let mut path: Vec<DerivativeChar> = path.clone();
					path.push(DerivativeChar::Char(ch));
					queue.push((derivative.next, i + advance, path.clone()));
				}
			}
		}

		completed_paths
	}
}

impl Regex {
	const EPSILON: Self = Self::Sequence(Vec::new());
	// const NIL: Self = Self::Alternation(Vec::new());

	fn apply_derivative(&self, input: SymbolicChar) -> Vec<Derivative> {
		let epsilon: Vec<Derivative> = vec![Derivative {
			maybe_capture: None,
			next: Self::EPSILON,
		}];
		let nil: Vec<Derivative> = Vec::new();

		match self {
			Self::AnyChar => epsilon,
			&Self::Literal(ch) => match input {
				SymbolicChar::Literal(input) => {
					if ch == input {
						epsilon
					} else {
						nil
					}
				},
				SymbolicChar::WildcardOne | SymbolicChar::WildcardStar => epsilon,
			},
			Self::Group { negated, items } => match input {
				SymbolicChar::Literal(input) => {
					for range in items.iter() {
						if (range.0 <= input) && (input <= range.1) {
							return if *negated { nil } else { epsilon };
						}
					}
					if *negated { epsilon } else { nil }
				},
				SymbolicChar::WildcardOne | SymbolicChar::WildcardStar => epsilon,
			},
			Self::Capture { info, item } => {
				assert!(!info.close);
				vec![Derivative {
					maybe_capture: Some(info.clone()),
					next: Self::Sequence(vec![
						(**item).clone(),
						Self::Capture {
							info: RegexCapture {
								close: true,
								..info.clone()
							},
							item: Box::new(Self::EPSILON),
						},
					]),
				}]
			},
			Self::Sequence(items) => {
				let Some(first): Option<&Self> = items.first() else {
					return nil;
				};
				let rest: Self = Self::Sequence(items[1..].to_vec());
				if let Self::Capture { info, .. } = first {
					if info.close {
						return vec![Derivative {
							maybe_capture: Some(info.clone()),
							next: rest,
						}];
					}
				}
				let mut paths: Vec<Derivative> = first.apply_derivative(input);
				paths.iter_mut().for_each(|derivative| {
					let mut items: Vec<Self> = items.clone();
					items[0] = derivative.next.clone();
					derivative.next = Regex::Sequence(items);
				});
				debug!("\t- derivative of first ({first:?}) is {paths:?}");
				if first.allows_epsilon_string() {
					paths.extend(rest.apply_derivative(input).into_iter());
				}
				debug!("\t\t- derivative of first ({first:?}) ({self:?}): {paths:?}");
				paths
			},
			Self::Alternation(items) => items
				.iter()
				.flat_map(|item| item.apply_derivative(input))
				.filter(|derivative| !derivative.next.is_empty_set())
				.collect::<Vec<_>>(),
			Self::BoundedRepetition { min, max, item } => {
				let mut required: Vec<Self> = vec![(**item).clone(); *min as usize];
				let mut optional: Vec<Self> = Vec::new();
				let mut sequence: Vec<Self> = Vec::new();
				for _ in *min..*max {
					sequence.push((**item).clone());
					optional.push(Self::Sequence(sequence.clone()));
				}
				let first: Self = Self::Sequence(required.clone());
				required.push(Self::Alternation(optional));
				let regex: Self = Self::Alternation(vec![first, Self::Sequence(required)]);
				regex.apply_derivative(input)
			},
			Self::KleeneClosure(item) => {
				if input == SymbolicChar::WildcardStar {
					item.apply_derivative(input)
				} else {
					Self::Sequence(vec![(**item).clone(), self.clone()]).apply_derivative(input)
				}
			},
			Self::KleenePlus(item) => {
				if input == SymbolicChar::WildcardStar {
					item.apply_derivative(input)
				} else {
					item.into_kleene_plus().apply_derivative(input)
				}
			},
		}
	}

	/// i.e. Sequence with no items.
	/// `allows_epsilon_string` implies `!is_empty_set`.
	fn allows_epsilon_string(&self) -> bool {
		match self {
			Self::AnyChar | Self::Literal(_) => false,
			Self::Group { .. } => false,
			Self::Capture { .. } => false,
			Self::KleeneClosure { .. } => true,
			Self::KleenePlus { .. } => false,
			Self::BoundedRepetition { min, .. } => *min == 0,
			Self::Sequence(items) => items.iter().all(Self::allows_epsilon_string),
			Self::Alternation(items) => items.iter().any(Self::allows_epsilon_string),
		}
	}

	/// i.e. Alternation with no items.
	fn is_empty_set(&self) -> bool {
		match self {
			Self::AnyChar | Self::Literal(_) => false,
			Self::Group { .. } => false,
			Self::Capture { item, .. } => item.is_empty_set(),
			Self::KleeneClosure(_) => false,
			Self::KleenePlus(item) => item.is_empty_set(),
			Self::BoundedRepetition { min, item, .. } => {
				if *min == 0 {
					false
				} else {
					item.is_empty_set()
				}
			},
			Self::Sequence(items) => items.iter().any(Self::is_empty_set),
			Self::Alternation(items) => items.iter().all(Self::is_empty_set),
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::search::SearchString;

	#[test]
	fn derivative_kleene_star() {
		{
			let regex: Regex = Regex::from_pattern("a*").unwrap().inner;

			let derivatives: Vec<Derivative> = regex.apply_derivative(SymbolicChar::Literal('a'));
			assert_eq!(derivatives[0].next.to_pattern(), "(a)*");
			assert_eq!(&derivatives[1..], &[]);
		}
	}

	#[test]
	fn derivative_kleene_star_and_capture() {
		{
			let regex: Regex = Regex::Sequence(vec![
				Regex::KleeneClosure(Box::new(Regex::Literal('a'))),
				Regex::Capture {
					info: RegexCapture::NULL,
					item: Box::new(Regex::EPSILON),
				},
			]);

			let derivatives: Vec<Derivative> = regex.apply_derivative(SymbolicChar::Literal('z'));

			assert!(!derivatives[0].maybe_capture.as_ref().unwrap().close);
			assert_eq!(&derivatives[1..], &[]);

			let derivatives: Vec<Derivative> = derivatives[0].next.apply_derivative(SymbolicChar::Literal('z'));

			assert!(derivatives[0].maybe_capture.as_ref().unwrap().close);
			assert_eq!(&derivatives[1..], &[]);
		}
	}

	#[test]
	fn simulate_email() {
		let regex: Regex = Regex::from_pattern(r"(?<user>\w+)@(?<parts>\w+\.)+(?<tld>\w+)")
			.unwrap()
			.inner;

		{
			let interpretations: Vec<String> = search(&regex, "a@bc.def");

			assert_eq!(interpretations[0], "(?<user>a)@(?<parts>bc.)(?<tld>def)");
			assert_eq!(&interpretations[1..], &[] as &[String]);
		}

		{
			let interpretations: Vec<String> = search(&regex, "a@*");

			assert_eq!(interpretations[0], "(?<user>a)@(?<parts>**)(?<tld>*)");
			assert_eq!(&interpretations[1..], &[] as &[String]);
		}

		{
			let interpretations: Vec<String> = search(&regex, "a*");

			assert_eq!(interpretations[0], "(?<user>a)*(?<parts>**)(?<tld>*)");
			assert_eq!(interpretations[1], "(?<user>a*)*(?<parts>**)(?<tld>*)");
			assert_eq!(&interpretations[2..], &[] as &[String]);
		}

		{
			let interpretations: Vec<String> = search(&regex, "a*@*");

			assert_eq!(interpretations[0], "(?<user>a)@(?<parts>**)(?<tld>*)");
			assert_eq!(interpretations[1], "(?<user>a*)@(?<parts>**)(?<tld>*)");
			assert_eq!(&interpretations[2..], &[] as &[String]);
		}

		{
			let interpretations: Vec<String> = search(&regex, "*a@example.com");

			println!("- inter is {interpretations:?}");

			// assert_eq!(interpretations[0], "(?<user>a)@(?<parts>**)(?<tld>*)");
			// assert_eq!(interpretations[1], "(?<user>a*)@(?<parts>**)(?<tld>*)");
			// assert_eq!(&interpretations[2..], &[] as &[String]);
		}
	}

	fn search(regex: &Regex, query: &str) -> Vec<String> {
		let search: SearchString = SearchString::parse(query).unwrap();

		let interpretations: Vec<Vec<SimulationToken>> = regex.simulate(search.as_slice());

		interpretations.into_iter().map(to_s).collect::<Vec<_>>()
	}

	fn to_s(interpretation: Vec<SimulationToken>) -> String {
		interpretation
			.into_iter()
			.map(|token| {
				let value: String = token
					.value
					.iter()
					.map(SymbolicChar::escape_for_search_string)
					.collect::<String>();
				if let Some(capture) = token.maybe_capture {
					format!("(?<{}>{})", capture.name, value)
				} else {
					value
				}
			})
			.collect::<String>()
	}
}
