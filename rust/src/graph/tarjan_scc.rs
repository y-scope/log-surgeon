//! Tarjan's SCC algorithm.
//! See <https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm>.

#[cfg(test)]
mod test;

use crate::graph::DfsIndex;

/// Results of Tarjan's SCC algorithm.
/// See [`TarjanSccs::tarjan_scc`].
#[derive(Debug, Clone)]
pub struct TarjanSccs {
	/// List of SCCs, where an SCC is a list of vertex indices.
	///
	/// **Unlike** the standard Tarjan's SCC algorithm,
	/// which identifies SCCs in a reverse topological sort (of the DAG of the SCCs),
	/// the output [`TarjanSccs::sccs`] is an (forward) topological sort.
	/// In other words, if `vertices[0]` is an "entry" or "root",
	/// its SCC will be the first in `TarjanSccs::sccs`.
	///
	/// Further, vertices within an SCC are sorted by [`TarjanVertex::encountered_at`].
	pub sccs: Vec<Vec<usize>>,
	/// Additional info associated with each vertex.
	pub vertices: Vec<TarjanVertex>,
	/// Mapping [`TarjanVertex::encountered_at`] to original vertex indices;
	/// (conceptually) `original_indices[vertices[i].encountered_at] == i`
	/// and `self[self[i].encountered_at] == i` (through [`std::ops::Index`]).
	original_indices: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct TarjanVertex {
	/// Order in which vertices are visited by the algorithm.
	/// Unvisited vertices are initialized with `usize::MAX`,
	/// which is not a valid index.
	pub encountered_at: DfsIndex,
	/// Minimal `encountered_at` among vertices of the corresponding SCC.
	pub low_link: DfsIndex,
	/// SCC index (in [`TarjanSccs::sccs`]).
	pub scc: usize,
	/// Working data for the algorithm.
	on_stack: bool,
}

#[derive(Debug)]
struct Frame<I> {
	index: usize,
	successors: I,
}

impl TarjanSccs {
	/// Compute [`TarjanSccs`] data from a list of vertices;
	/// the data refers to the vertices by their original index in the input slice.
	///
	/// See [`TarjanSccs`] for more details on the output.
	pub fn tarjan_scc<'a, E, T, F, I>(vertices: &'a [T], entries: E, successors: F) -> Self
	where
		T: 'a,
		E: IntoIterator<Item = usize>,
		F: Fn(&'a T) -> I + 'a,
		I: Iterator<Item = usize>,
	{
		let mut this: Self = Self {
			sccs: Vec::new(),
			vertices: vec![
				TarjanVertex {
					encountered_at: DfsIndex::INVALID,
					low_link: DfsIndex::INVALID,
					scc: usize::MAX,
					on_stack: false,
				};
				vertices.len()
			],
			original_indices: Vec::new(),
		};

		{
			// For disjoint graphs, `strong_connect` needs to be called on
			// (at least one vertex of) each connected subgraph.
			for i in entries {
				this.strong_connect_iterative(vertices, i, &successors);
			}
		}

		{
			// Tarjan's SCC algorithm identifies SCCs
			// in a reverse topological sort of the DAG of the SCCs;
			// i.e., the SCC containing the entry state of an NFA/DFA would be ordered last.
			this.sccs.reverse();

			for (scc_idx, scc) in this.sccs.iter().enumerate() {
				for &i in scc.iter() {
					this.vertices[i].scc = scc_idx;
				}
			}
		}

		this
	}

	/// "Iterative" implementation of the recursive `strong_connect` subprocedure
	/// in Tarjan's SCC algorithm:
	/// visits all vertices that are reachable from `start`.
	///
	/// The recursion call-stack is implemented as a heap-allocated `Vec`/stack of frames;
	/// unlike iterative versions of tail-recursive functions,
	/// more bookkeeping is necessary to perform the "after-recursion" work.
	fn strong_connect_iterative<'a, T, F, I>(
		&mut self,
		vertices: &'a [T],
		start: usize,
		successors: &F,
	) where
		T: 'a,
		F: Fn(&'a T) -> I + 'a,
		I: Iterator<Item = usize>,
	{
		if self.vertices[start].encountered_at != DfsIndex::INVALID {
			return;
		}

		// "Frames" for iterative conversion of recursive algorithm.
		let mut frames: Vec<Frame<I>> = Vec::new();
		// Tarjan's SCC algorithm's stack of vertices in the current SCC.
		let mut stack: Vec<usize> = Vec::new();

		self.push_frame(vertices, start, successors, &mut frames, &mut stack);

		while let Some(mut frame) = frames.pop() {
			let i: usize = frame.index;
			if let Some(j) = frame.successors.next() {
				frames.push(frame);
				if self.vertices[j].encountered_at == DfsIndex::INVALID {
					self.push_frame(vertices, j, successors, &mut frames, &mut stack);
				} else {
					if self.vertices[j].on_stack {
						// As described in the original paper, it would be
						// `min(self.vertices[i].low_link, self.vertices[j].encountered_at)`.
						// However, it remains true that `low_link == encountered_at` iff
						// the vertex is the root of its SCC,
						// and the algorithm otherwise remains valid.
						// See
						// <https://en.wikipedia.org/wiki/Tarjan's_strongly_connected_components_algorithm>.
						self.vertices[i].low_link =
							self.vertices[i].low_link.min(self.vertices[j].low_link);
					}
				}
			} else {
				if let Some(parent) = frames.last() {
					self.vertices[parent.index].low_link = std::cmp::min(
						self.vertices[parent.index].low_link,
						self.vertices[i].low_link,
					);
				}

				if self.vertices[i].low_link == self.vertices[i].encountered_at {
					let mut scc: Vec<usize> = Vec::new();
					loop {
						let j: usize = stack.pop().unwrap();
						self.vertices[j].on_stack = false;
						scc.push(j);
						if j == i {
							break;
						}
					}
					scc.reverse();
					assert!(scc.is_sorted_by_key(|&i| self.vertices[i].encountered_at));
					assert_eq!(scc[0], i);
					self.sccs.push(scc);
				}
			}
		}
	}

	fn push_frame<'a, T, F, I>(
		&mut self,
		vertices: &'a [T],
		index: usize,
		successors: &F,
		frames: &mut Vec<Frame<I>>,
		stack: &mut Vec<usize>,
	) where
		F: Fn(&'a T) -> I + 'a,
	{
		let encountered_at: DfsIndex = DfsIndex(self.original_indices.len());
		self.vertices[index] = TarjanVertex {
			encountered_at,
			low_link: encountered_at,
			on_stack: true,
			scc: usize::MAX,
		};
		self.original_indices.push(index);
		stack.push(index);

		frames.push(Frame {
			index,
			successors: successors(&vertices[index]),
		})
	}

	#[allow(unused)]
	fn strong_connect_recursive<'a, T, F, I>(
		&mut self,
		vertices: &'a [T],
		i: usize,
		successors: &F,
		stack: &mut Vec<usize>,
	) where
		T: 'a,
		F: Fn(&'a T) -> I + 'a,
		I: Iterator<Item = usize>,
	{
		if self.vertices[i].encountered_at != DfsIndex::INVALID {
			return;
		}

		let encountered_at: DfsIndex = DfsIndex(self.original_indices.len());
		self.vertices[i] = TarjanVertex {
			encountered_at,
			low_link: encountered_at,
			on_stack: true,
			scc: usize::MAX,
		};
		self.original_indices.push(i);
		stack.push(i);

		for j in successors(&vertices[i]) {
			if self.vertices[j].encountered_at != DfsIndex::INVALID {
				if self.vertices[j].on_stack {
					self.vertices[i].low_link =
						self.vertices[i].low_link.min(self.vertices[j].low_link);
				}
			} else {
				self.strong_connect_recursive(vertices, j, successors, stack);
				self.vertices[i].low_link =
					std::cmp::min(self.vertices[i].low_link, self.vertices[j].low_link);
			}
		}

		if self.vertices[i].low_link == self.vertices[i].encountered_at {
			let mut scc: Vec<usize> = Vec::new();
			loop {
				let j: usize = stack.pop().unwrap();
				self.vertices[j].on_stack = false;
				scc.push(j);
				if j == i {
					break;
				}
			}
			scc.reverse();
			assert!(scc.is_sorted_by_key(|&i| self.vertices[i].encountered_at));
			assert_eq!(scc[0], i);
			self.sccs.push(scc);
		}
	}
}
