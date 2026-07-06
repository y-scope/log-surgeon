#[derive(Debug, Clone)]
pub struct TarjanSccs {
	pub sccs: Vec<Vec<usize>>,
	pub vertices: Vec<TarjanVertex>,
	pub original_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct TarjanVertex {
	pub encountered_at: usize,
	pub low_link: usize,
	pub scc: usize,
	on_stack: bool,
}

impl TarjanSccs {
	pub fn tarjan_scc<'a, T, F, I>(vertices: &'a [T], successors: F) -> Self
	where
		T: 'a,
		F: Fn(&'a T) -> I + 'a,
		I: Iterator<Item = usize>,
	{
		let mut this: Self = Self {
			sccs: Vec::new(),
			vertices: vec![
				TarjanVertex {
					encountered_at: usize::MAX,
					low_link: usize::MAX,
					scc: usize::MAX,
					on_stack: false,
				};
				vertices.len()
			],
			original_indices: Vec::new(),
		};

		let mut stack: Vec<usize> = Vec::new();

		for i in 0..vertices.len() {
			this.strong_connect(vertices, i, &successors, &mut stack);
		}

		this.sccs.reverse();

		for vertex in this.vertices.iter_mut() {
			vertex.scc = this.sccs.len() - vertex.scc - 1;
		}

		this
	}

	fn strong_connect<'a, T, F, I>(&mut self, vertices: &'a [T], i: usize, successors: &F, stack: &mut Vec<usize>)
	where
		T: 'a,
		F: Fn(&'a T) -> I + 'a,
		I: Iterator<Item = usize>,
	{
		if self.vertices[i].encountered_at != usize::MAX {
			return;
		}

		let encountered_at: usize = self.original_indices.len();
		self.vertices[i] = TarjanVertex {
			encountered_at,
			low_link: encountered_at,
			on_stack: true,
			scc: usize::MAX,
		};
		self.original_indices.push(i);
		stack.push(i);

		for j in successors(&vertices[i]) {
			if self.vertices[j].encountered_at != usize::MAX {
				if self.vertices[j].on_stack {
					self.vertices[i].low_link =
						std::cmp::min(self.vertices[i].low_link, self.vertices[j].encountered_at);
				}
			} else {
				self.strong_connect(vertices, j, successors, stack);
				self.vertices[i].low_link = std::cmp::min(self.vertices[i].low_link, self.vertices[j].low_link);
			}
		}

		if self.vertices[i].low_link == self.vertices[i].encountered_at {
			let mut scc: Vec<usize> = Vec::new();
			loop {
				let j: usize = stack.pop().unwrap();
				self.vertices[j].on_stack = false;
				self.vertices[j].scc = self.sccs.len();
				scc.push(j);
				if j == i {
					break;
				}
			}
			scc.reverse();
			scc.sort_by_key(|&i| self.vertices[i].encountered_at);
			assert_eq!(scc[0], i);
			self.sccs.push(scc);
		}
	}
}
