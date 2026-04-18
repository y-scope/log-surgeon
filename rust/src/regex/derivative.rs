use super::*;
use crate::search::SearchString;
use crate::search::SymbolicChar;

#[derive(Debug)]
pub enum Derivative {
	Regex(Regex),
	Capture(RegexCapture, Regex),
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
	pub fn simulate(&self, input: &SearchString) -> Vec<Vec<(RegexCapture, usize, usize)>> {
		let regex: Self = Self::Sequence(vec![self.clone(), Self::Literal('\0')]);

		let mut all_paths: Vec<Vec<(RegexCapture, usize)>> = regex.simulate_internal(input.as_slice(), 0);
		all_paths.sort();
		all_paths.dedup();
		for path in all_paths.iter_mut() {
			path.reverse();
		}

		let mut interpretations: Vec<Vec<(RegexCapture, usize, usize)>> = Vec::with_capacity(all_paths.len());
		let mut stacks: Vec<Vec<usize>> = vec![Vec::new(); 1 + self.count_captures()];
		for path in all_paths.iter() {
			stacks.iter_mut().for_each(Vec::clear);
			let mut leaves: Vec<(RegexCapture, usize, usize)> = Vec::new();
			for (capture, pos) in path.iter() {
				if !capture.is_leaf() {
					continue;
				}
				let stack: &mut Vec<usize> = &mut stacks[capture.id.get() as usize];
				if capture.close {
					let start: usize = stack.pop().unwrap();
					leaves.push((capture.clone(), start, *pos));
				} else {
					stack.push(*pos);
				}
			}
			interpretations.push(leaves);
		}

		interpretations
	}

	fn simulate_internal(&self, input: &[SymbolicChar], i: usize) -> Vec<Vec<(RegexCapture, usize)>> {
		let Some(ch): Option<SymbolicChar> = input.first().copied() else {
			return if self.allows_epsilon_string() {
				vec![Vec::new()]
			} else {
				Vec::new()
			};
		};

		let mut all_paths: Vec<Vec<(RegexCapture, usize)>> = Vec::new();

		println!("simulating {ch:?} with {self:?}");

		match ch {
			SymbolicChar::Literal(_) => {
				recurse(self.apply_derivative(ch), ch, input, i, &mut all_paths);
			},
			SymbolicChar::WildcardOne | SymbolicChar::WildcardStar => {
				all_paths.extend(self.simulate_internal(&input[1..], i + 1).into_iter());
				recurse(self.apply_derivative(ch), ch, input, i, &mut all_paths);
			},
		}

		all_paths
	}
}

impl Regex {
	const EPSILON: Self = Self::Sequence(Vec::new());
	// const NIL: Self = Self::Alternation(Vec::new());

	pub fn apply_derivative(&self, input: SymbolicChar) -> Vec<Derivative> {
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
				if let Self::Capture { info, .. } = first {
					if info.close {
						return vec![Derivative::Capture(info.clone(), rest)];
					}
				}
				let mut paths: Vec<Derivative> = Vec::new();
				println!("\t- applying derivative to {first:?}, {input:?}");
				paths.extend(
					first
						.apply_derivative(input)
						.into_iter()
						.map(|derivative| match derivative {
							Derivative::Regex(next) => {
								println!("\t- next is {next:?}");
								let mut items: Vec<Self> = items.clone();
								items[0] = next;
								Derivative::Regex(Regex::Sequence(items))
							},
							Derivative::Capture(info, next) => {
								let mut items: Vec<Self> = items.clone();
								items[0] = next;
								Derivative::Capture(info, Regex::Sequence(items))
							},
						}),
				);
				if first.allows_epsilon_string() {
					println!("\t- {first:?} allows epsilon {rest:?}");
					paths.extend(rest.apply_derivative(input).into_iter());
				}
				println!("- nexts for {input:?} is {paths:?}");
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
				let mut paths: Vec<Derivative> = Vec::new();
				if matches!(input, SymbolicChar::WildcardStar) {
					paths.extend(item.apply_derivative(input).into_iter());
				} else {
					paths.extend(
						Self::Sequence(vec![(**item).clone(), self.clone()])
							.apply_derivative(input)
							.into_iter(),
					);
				}
				paths.extend(epsilon.into_iter());
				paths
			},
			Self::KleenePlus(item) => item.into_kleene_plus().apply_derivative(input),
		}
	}

	/// i.e. Sequence with no items.
	/// `allows_epsilon_string` implies `!is_empty_set`.
	fn allows_epsilon_string(&self) -> bool {
		match self {
			Self::AnyChar | Self::Literal(_) => false,
			Self::Group { .. } => false,
			Self::Capture { info, item } => !info.close && item.allows_epsilon_string(),
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

fn recurse(
	derivatives: Vec<Derivative>,
	ch: SymbolicChar,
	input: &[SymbolicChar],
	i: usize,
	all_paths: &mut Vec<Vec<(RegexCapture, usize)>>,
) {
	let mut next_input: &[SymbolicChar] = &input[1..];
	let mut next_i: usize = i + 1;
	if matches!(ch, SymbolicChar::WildcardStar) {
		next_input = input;
		next_i = i;
	}
	for d in derivatives.into_iter() {
		match d {
			Derivative::Regex(next) => {
				if !next.is_empty_set() {
					all_paths.extend(next.simulate_internal(&next_input[..], next_i).into_iter());
				}
			},
			Derivative::Capture(info, next) => {
				if !next.is_empty_set() {
					all_paths.extend(next.simulate_internal(input, i).into_iter().map(|mut path| {
						path.push((info.clone(), i));
						path
					}));
				}
			},
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn basic() {
		let regex: TopLevelRegex = Regex::from_pattern(r"(?<user>\w+)@(?<parts>\w+\.)+(?<tld>\w+)").unwrap();

		// let regex: Regex = Regex::from_pattern(r"a@example.com").unwrap();
		{
			let search: SearchString = SearchString::parse("a*com").unwrap();

			let interpretations: Vec<Vec<(RegexCapture, usize, usize)>> = regex.inner.simulate(&search);

			println!("interpretations: ");
			for i in interpretations.iter() {
				println!(
					"- {:?}",
					i.iter()
						.map(|(capture, i, j)| (&capture.name, i, j))
						.collect::<Vec<_>>()
				);
			}
			panic!();
		}
	}
}
