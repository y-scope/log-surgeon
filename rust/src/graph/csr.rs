#[cfg(test)]
mod test;

/// Compressed Sparse Row.
#[derive(Debug, Clone)]
pub struct Csr<T> {
	offsets: Vec<usize>,
	edges: Vec<Edge<T>>,
}

#[derive(Debug, Clone)]
pub struct Edge<T> {
	pub source: usize,
	pub target: usize,
	pub value: T,
}

impl<T> Csr<T> {
	// Note: edges not stable w.r.t. iterator.
	pub fn build<I>(num_vertices: usize, get_edges: I) -> Self
	where
		I: IntoIterator<Item = (usize, usize, T)>,
	{
		// Flat array of all edges.
		let mut edges: Vec<Edge<T>> = Vec::new();

		// The edges of state `i` should be `edges[offsets[i]..offsets[i + 1]]`.
		// In other words, `offsets[i]` is the start, `offsets[i + 1]` is the end.
		let mut offsets: Vec<usize> = vec![0; num_vertices + 1];

		for (source, target, value) in get_edges {
			// Initially, we store the number of edges for `i` in `offsets[i + 1]`;
			// the start of `i + 1`'s transitions are offset by (at least) `i`'s transitions.
			offsets[source + 1] += 1;
			edges.push(Edge {
				source,
				target,
				value,
			});
		}

		// Now, `offsets[i + 1]` includes "just" the contribution from state `i`.
		// The following compounding sum updates the offsets
		// to include the contribution from states `0..(i - 1)`.
		for i in 1..offsets.len() {
			offsets[i] += offsets[i - 1];
		}

		let mut cursors: Vec<usize> = offsets[0..num_vertices].to_vec();
		// Process vertices left to right.
		for source in 0..num_vertices {
			let end: usize = offsets[source + 1];
			// Process the edges in the current bucket;
			// if an edge sticks out, put it in its place.
			while cursors[source] < end {
				let edge_source: usize = edges[cursors[source]].source;
				if edge_source == source {
					cursors[source] += 1;
				} else {
					edges.swap(cursors[source], cursors[edge_source]);
					// You're welcome.
					cursors[edge_source] += 1;
				}
			}
		}

		Self { offsets, edges }
	}
}

impl<T> std::ops::Index<usize> for Csr<T> {
	type Output = [Edge<T>];

	fn index(&self, i: usize) -> &Self::Output {
		&self.edges[self.offsets[i]..self.offsets[i + 1]]
	}
}
