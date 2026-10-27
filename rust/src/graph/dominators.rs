//! Implementation of dominator algorithm by Lengauer and Tarjan:
//! <https://dl.acm.org/doi/10.1145/357062.357071>.

use crate::graph::DfsIndex;

#[derive(Debug, Clone)]
pub struct Dominators {
	/// Vertices unreachable from the entry get `usize::MAX`,
	/// which is always an invalid index into an array/slice.
	pub idom: Vec<usize>,
	vertices: Vec<DominatorVertex>,
	original_indices: Vec<usize>,
}

#[derive(Debug, Clone)]
struct DominatorVertex {
	#[allow(unused)]
	index: usize,
	predecessors: Vec<usize>,
	dfs_index: DfsIndex,
	semi_dominator: DfsIndex,
	parent: usize,
}

#[derive(Debug)]
struct Frame<I> {
	index: usize,
	successors: I,
}

const UNVISITED: usize = usize::MAX;

impl Dominators {
	pub fn dominators<'a, T, F, I>(vertices: &'a [T], entry: usize, successors: F) -> Self
	where
		T: 'a,
		F: Fn(&'a T) -> I + 'a,
		I: Iterator<Item = usize>,
	{
		let mut this: Self = Self {
			idom: vec![UNVISITED; vertices.len()],
			vertices: (0..vertices.len())
				.map(|index| DominatorVertex {
					index,
					predecessors: Vec::new(),
					dfs_index: DfsIndex::INVALID,
					semi_dominator: DfsIndex::INVALID,
					parent: UNVISITED,
				})
				.collect::<Vec<_>>(),
			original_indices: Vec::new(),
		};

		if vertices.is_empty() {
			return this;
		}

		this.dfs(vertices, entry, &successors);

		this.compute()
	}
}

impl Dominators {
	// DFS from an entry.
	fn dfs<'a, T, F, I>(&mut self, vertices: &'a [T], entry: usize, successors: &F)
	where
		T: 'a,
		F: Fn(&'a T) -> I + 'a,
		I: Iterator<Item = usize>,
	{
		// "Frames" for iterative conversion of recursive algorithm.
		let mut frames: Vec<Frame<I>> = Vec::new();

		self.push_frame(vertices, entry, usize::MAX, successors, &mut frames);

		while let Some(mut frame) = frames.pop() {
			let i: usize = frame.index;
			if let Some(j) = frame.successors.next() {
				self.vertices[j].predecessors.push(i);
				frames.push(frame);
				if self.vertices[j].dfs_index == DfsIndex::INVALID {
					self.push_frame(vertices, j, i, successors, &mut frames);
				}
			}
		}
	}

	fn push_frame<'a, T, F, I>(
		&mut self,
		vertices: &'a [T],
		index: usize,
		parent: usize,
		successors: &F,
		frames: &mut Vec<Frame<I>>,
	) where
		F: Fn(&'a T) -> I + 'a,
	{
		let dfs_index: DfsIndex = DfsIndex(self.original_indices.len());
		self.vertices[index].dfs_index = dfs_index;
		self.vertices[index].parent = parent;
		self.original_indices.push(index);

		frames.push(Frame {
			index,
			successors: successors(&vertices[index]),
		})
	}
}

impl Dominators {
	fn compute(mut self) -> Self {
		let mut labels: Vec<usize> = vec![UNVISITED; self.vertices.len()];

		for &i in self.original_indices.iter() {
			self.vertices[i].semi_dominator = self.vertices[i].dfs_index;
			labels[i] = i;
		}

		let mut ancestors: Vec<usize> = vec![UNVISITED; self.vertices.len()];

		let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); self.vertices.len()];

		let mut compress_stack: Vec<usize> = Vec::new();

		for &w in self.original_indices[1..].iter().rev() {
			let mut semi: DfsIndex = self.vertices[w].dfs_index;

			for &v in self.vertices[w].predecessors.iter() {
				let u: usize = self.eval(v, &mut ancestors, &mut labels, &mut compress_stack);

				semi = semi.min(self.vertices[u].semi_dominator);
			}

			self.vertices[w].semi_dominator = semi;

			let s: usize = self.original_indices[semi.0];
			buckets[s].push(w);

			let parent: usize = self.vertices[w].parent;
			ancestors[w] = parent;

			for v in buckets[parent].drain(..) {
				let u: usize = self.eval(v, &mut ancestors, &mut labels, &mut compress_stack);

				self.idom[v] = if self.vertices[u].semi_dominator < self.vertices[v].semi_dominator
				{
					u
				} else {
					parent
				};
			}
		}

		for &w in self.original_indices[1..].iter() {
			let s: usize = self.original_indices[self.vertices[w].semi_dominator.0];
			if self.idom[w] != s {
				self.idom[w] = self.idom[self.idom[w]];
			}
		}

		self
	}

	fn compress(
		&self,
		mut v: usize,
		ancestors: &mut [usize],
		labels: &mut [usize],
		stack: &mut Vec<usize>,
	) {
		while ancestors[v] != UNVISITED {
			let a: usize = ancestors[v];

			if ancestors[a] == UNVISITED {
				break;
			}

			stack.push(v);
			v = a;
		}

		while let Some(v) = stack.pop() {
			let a: usize = ancestors[v];
			let aa: usize = ancestors[a];

			if self.vertices[labels[a]].semi_dominator < self.vertices[labels[v]].semi_dominator {
				labels[v] = labels[a];
			}

			ancestors[v] = aa;
		}
	}

	fn eval(
		&self,
		v: usize,
		ancestors: &mut [usize],
		labels: &mut [usize],
		stack: &mut Vec<usize>,
	) -> usize {
		if ancestors[v] == UNVISITED {
			v
		} else {
			self.compress(v, ancestors, labels, stack);
			labels[v]
		}
	}
}
