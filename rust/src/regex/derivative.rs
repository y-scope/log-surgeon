use super::*;
use crate::search::SymbolicChar;

#[derive(Debug)]
pub enum Derivative {
	Regex(Regex),
	Capture(RegexCapture, Regex),
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum DerivativeChar {
	Char(SymbolicChar),
	Derivative(RegexCapture, SymbolicChar),
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SimulationToken {
	pub maybe_capture: Option<RegexCapture>,
	pub value: Vec<SymbolicChar>,
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
					DerivativeChar::Derivative(capture, _ch) => {
						if !capture.close {
							if !value.is_empty() {
								tokens.push(SimulationToken {
									maybe_capture: None,
									value: std::mem::replace(&mut value, Vec::new()),
								});
							}
							maybe_start = Some(capture);
							// value.push(ch);
						} else {
							tokens.push(SimulationToken {
								maybe_capture: Some(maybe_start.take().unwrap()),
								value: std::mem::replace(&mut value, Vec::new()),
							});
							// value.push(ch);
						}
					},
				}
			}
			if !value.is_empty() {
				tokens.push(SimulationToken {
					maybe_capture: None,
					value: std::mem::replace(&mut value, Vec::new()),
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
					// println!("- pushing completed path {path:?}");
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
			let offset: usize = if ch == SymbolicChar::WildcardStar { 0 } else { 1 };
			// let mut any: bool = false;
			if ch.is_wildcard() {
				// println!("\t- skipping over {i}, {regex:?}");
				queue.push((regex.clone(), i + 1, path.clone()));
			}
			// println!("- path ({i}, {ch:?}) {regex:?} ");
			for derivative in regex.apply_derivative(ch).into_iter().rev() {
				// any = true;
				match derivative {
					Derivative::Regex(derivative) => {
						if derivative.is_empty_set() {
							continue;
						}
						// println!(
						// 	"- path ({i}, {ch:?})^-1 {regex:?} | {:?}",
						// 	std::fmt::from_fn(|fmt| {
						// 		for path in paths.iter() {
						// 			for capture in path.iter() {
						// 				fmt.write_fmt(format_args!(
						// 					"({}, {}, {}), ",
						// 					capture.0.qualified_name, capture.0.close, capture.1
						// 				))?;
						// 			}
						// 		}
						// 		Ok(())
						// 	})
						// );
						let mut path: Vec<DerivativeChar> = path.clone();
						path.push(DerivativeChar::Char(ch));
						// println!("\t - pushing derivative '{derivative:?}'");
						queue.push((derivative, i + offset, path.clone()));
					},
					Derivative::Capture(capture, derivative) => {
						if derivative.is_empty_set() {
							continue;
						}
						// println!(
						// 	"- path ({i}, {ch:?})^-1 {regex:?} | {:?}",
						// 	std::fmt::from_fn(|fmt| {
						// 		for path in paths.iter() {
						// 			for capture in path.iter() {
						// 				fmt.write_fmt(format_args!(
						// 					"({}, {}, {}), ",
						// 					capture.0.qualified_name, capture.0.close, capture.1
						// 				))?;
						// 			}
						// 		}
						// 		Ok(())
						// 	})
						// );
						// println!("\t- pushing derivative {derivative:?} with capture");
						let mut path: Vec<DerivativeChar> = path.clone();
						path.push(DerivativeChar::Derivative(capture.clone(), ch));
						// let j: usize = if capture.close { i + offset } else { i };
						// println!("\t - pushing capture derivative {derivative:?}, {i}");
						// paths.iter_mut().for_each(|path| {
						// path.push((capture.clone(), i));
						// });
						// let j: usize = if capture.close { i + 1 - offset } else { i };
						queue.push((derivative, i, path));
					},
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
		let epsilon: Vec<Derivative> = vec![Derivative::Regex(Self::EPSILON)];
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
				vec![Derivative::Capture(
					info.clone(),
					Self::Sequence(vec![
						(**item).clone(),
						Self::Capture {
							info: RegexCapture {
								close: true,
								..info.clone()
							},
							item: Box::new(Self::EPSILON),
						},
					]),
				)]
			},
			Self::Sequence(items) => {
				let Some(first): Option<&Self> = items.first() else {
					return nil;
				};
				let rest: Self = Self::Sequence(items[1..].to_vec());
				// println!("- first {first:?}, rest {rest:?}");
				if let Self::Capture { info, .. } = first {
					if info.close {
						return vec![Derivative::Capture(info.clone(), rest)];
					}
				}
				let mut paths: Vec<Derivative> = Vec::new();
				// println!("\t\t- applying derivative to {first:?}");
				// let front_derivatives: Vec<Derivative> = first.apply_derivative(input);
				// println!("\t\t\t- derivative of {first:?} is {:?}", first.apply_derivative(input));
				paths.extend(
					first
						.apply_derivative(input)
						.into_iter()
						.map(|derivative| match derivative {
							Derivative::Regex(next) => {
								let mut items: Vec<Self> = items.clone();
								items[0] = next;
								// println!("- adding items {items:?}");
								Derivative::Regex(Regex::Sequence(items))
							},
							Derivative::Capture(info, next) => {
								let mut items: Vec<Self> = items.clone();
								items[0] = next;
								// println!("- adding items {items:?}");
								Derivative::Capture(info, Regex::Sequence(items))
							},
						}),
				);
				if first.allows_epsilon_string() {
					// println!("- first allows epsilon {first:?}, {rest:?}");
					// println!("\t\t\t- extended with {:?}", rest.apply_derivative(input));
					paths.extend(rest.apply_derivative(input).into_iter());
				}
				// println!("\t\t- returning for {self:?}: {paths:?} ({first:?})");
				// println!("- input {input:?} on {first:?}, {rest:?}, returning {paths:?}");
				paths
			},
			Self::Alternation(items) => items
				.iter()
				.flat_map(|item| item.apply_derivative(input))
				.filter(|derivative| match derivative {
					Derivative::Regex(regex) | Derivative::Capture(_, regex) => !regex.is_empty_set(),
				})
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

	/*
	#[test]
	fn basic2() {
		dbg!(
			Regex::from_pattern("b*")
				.unwrap()
				.inner
				.apply_derivative(SymbolicChar::Literal('b'))
		);
		panic!();
	}
	*/

	// #[test]
	// fn basic() {
	// 	// let regex: TopLevelRegex = Regex::from_pattern(r"(?<user>\w+)@(?<parts>\w+\.)+(?<tld>\w+)").unwrap();
	// 	// let regex: TopLevelRegex = Regex::from_pattern(r"(?<user>\w+)@(?<tld>\w+)").unwrap();
	// 	// let regex: TopLevelRegex = Regex::from_pattern(r"(?<user>\w+)@").unwrap();
	// 	let regex: TopLevelRegex = Regex::from_pattern(r"(?<user>aa*)@a").unwrap();

	// 	// let regex: Regex = Regex::from_pattern(r"a@example.com").unwrap();
	// 	{
	// 		// let search: SearchString = SearchString::parse("a*@*com").unwrap();
	// 		let search: SearchString = SearchString::parse("a*@*").unwrap();

	// 		let interpretations: Vec<Vec<(RegexCapture, usize, usize)>> = regex.inner.simulate(search.as_slice());

	// 		println!("interpretations: ");
	// 		for i in interpretations.iter() {
	// 			println!(
	// 				"- {:?}",
	// 				i.iter()
	// 					.map(|(capture, i, j)| (&capture.name, i, j))
	// 					.collect::<Vec<_>>()
	// 			);
	// 		}
	// 		panic!();
	// 	}
	// }

	// #[test]
	// fn basic3() {
	// 	let regex = Regex::Sequence(vec![
	// 		Regex::KleeneClosure(Box::new(Regex::Literal('a'))),
	// 		Regex::Capture {
	// 			info: RegexCapture::NULL,
	// 			item: Box::new(Regex::EPSILON),
	// 		},
	// 	]);

	// 	dbg!(regex.apply_derivative(SymbolicChar::Literal('@')));
	// 	panic!();
	// }

	#[test]
	fn email() {
		let regex: TopLevelRegex = Regex::from_pattern(r"(?<user>\w+)@(?<parts>\w+\.)+(?<tld>\w+)").unwrap();

		{
			let search: SearchString = SearchString::parse("a@bc.def").unwrap();

			let interpretations: Vec<Vec<SimulationToken>> = regex.inner.simulate(search.as_slice());
			let interpretations: Vec<String> = interpretations.into_iter().map(to_s).collect::<Vec<_>>();

			assert_eq!(interpretations[0], "(?<user>a)@(?<parts>bc.)(?<tld>def)");
			assert_eq!(interpretations.len(), 1);

			// println!("interpretations: ");
			// for i in interpretations.iter() {
			// 	println!("- {i}");
			// }
		}

		{
			let search: SearchString = SearchString::parse("a@*").unwrap();

			let interpretations: Vec<Vec<SimulationToken>> = regex.inner.simulate(search.as_slice());
			let interpretations: Vec<String> = interpretations.into_iter().map(to_s).collect::<Vec<_>>();

			assert_eq!(interpretations[0], "(?<user>a)@(?<parts>**)(?<tld>*)");
			assert_eq!(interpretations.len(), 1);
		}

		{
			let search: SearchString = SearchString::parse("a*").unwrap();

			let interpretations: Vec<Vec<SimulationToken>> = regex.inner.simulate(search.as_slice());
			let interpretations: Vec<String> = interpretations.into_iter().map(to_s).collect::<Vec<_>>();

			assert_eq!(interpretations[0], "(?<user>a)*(?<parts>**)(?<tld>*)");
			assert_eq!(interpretations[1], "(?<user>a*)*(?<parts>**)(?<tld>*)");
			assert_eq!(interpretations.len(), 2);
		}

		{
			let search: SearchString = SearchString::parse("a*@*").unwrap();

			let interpretations: Vec<Vec<SimulationToken>> = regex.inner.simulate(search.as_slice());
			let interpretations: Vec<String> = interpretations.into_iter().map(to_s).collect::<Vec<_>>();

			assert_eq!(interpretations[0], "(?<user>a)@(?<parts>**)(?<tld>*)");
			assert_eq!(interpretations[1], "(?<user>a*)@(?<parts>**)(?<tld>*)");
			assert_eq!(interpretations.len(), 2);
		}
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
