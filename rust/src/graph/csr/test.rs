use super::*;
use crate::utils::test::rng;

/// The targets of `csr[source]`, sorted for order-independent comparison.
fn targets<T: Copy>(csr: &Csr<T>, source: usize) -> Vec<usize> {
	let mut targets: Vec<usize> = Vec::from_iter(csr[source].iter().map(|edge| edge.target));
	targets.sort();
	targets
}

#[test]
fn build_empty_graph() {
	let csr: Csr<()> = Csr::build(0, std::iter::empty::<(usize, usize, ())>());
	assert!(csr.edges.is_empty());
	assert_eq!(csr.offsets, vec![0]);
}

#[test]
fn build_no_edges() {
	let csr: Csr<()> = Csr::build(3, std::iter::empty::<(usize, usize, ())>());
	assert!(csr.edges.is_empty());
	assert_eq!(csr.offsets, vec![0, 0, 0, 0]);
	for source in 0..3 {
		assert!(csr[source].is_empty());
	}
}

#[test]
fn buckets_group_edges_by_source() {
	let edges: Vec<(usize, usize, u32)> = vec![
		(2, 0, 20),
		(0, 1, 1),
		(2, 3, 23),
		(1, 2, 12),
		(0, 2, 2),
		(2, 1, 21),
	];
	let csr: Csr<u32> = Csr::build(3, edges.iter().copied());

	assert_eq!(csr.offsets, vec![0, 2, 3, 6]);
	assert_eq!(targets(&csr, 0), vec![1, 2]);
	assert_eq!(targets(&csr, 1), vec![2]);
	assert_eq!(targets(&csr, 2), vec![0, 1, 3]);

	for source in 0..3 {
		for edge in csr[source].iter() {
			assert_eq!(edge.source, source);
		}
	}
}

#[test]
fn every_edge_is_preserved() {
	let edges: Vec<(usize, usize, char)> = vec![
		(0, 1, 'a'),
		(1, 0, 'b'),
		(1, 1, 'c'),
		(2, 0, 'd'),
		(0, 2, 'e'),
	];
	let csr: Csr<char> = Csr::build(3, edges.iter().copied());

	let mut got: Vec<(usize, usize, char)> = Vec::from_iter(
		csr.edges
			.iter()
			.map(|edge| (edge.source, edge.target, edge.value)),
	);
	got.sort();

	let mut expected: Vec<(usize, usize, char)> = edges;
	expected.sort();

	assert_eq!(got, expected);
}

#[test]
fn out_of_order_edges_are_reordered() {
	// The first edge belongs to vertex 1, so building vertex 0's bucket
	// must displace it.
	let edges: Vec<(usize, usize, u32)> = vec![(1, 0, 10), (0, 1, 1)];
	let csr: Csr<u32> = Csr::build(2, edges.iter().copied());

	assert_eq!(targets(&csr, 0), vec![1]);
	assert_eq!(targets(&csr, 1), vec![0]);
	assert_eq!(csr[0][0].value, 1);
	assert_eq!(csr[1][0].value, 10);
}

#[test]
fn non_copy_values_are_moved() {
	let edges: Vec<(usize, usize, String)> =
		vec![(0, 1, String::from("x")), (1, 0, String::from("y"))];
	let csr: Csr<String> = Csr::build(2, edges);

	assert_eq!(csr[0][0].value, "x");
	assert_eq!(csr[1][0].value, "y");
}

#[test]
fn randomized_graphs_preserve_edges_per_source() {
	let mut next = rng(0x2545_F491_4F6C_DD1D);

	for num_vertices in 1..8 {
		for _ in 0..16 {
			let edges: Vec<(usize, usize, u64)> = (0..32)
				.map(|value| {
					let source = (next() as usize) % num_vertices;
					let target = (next() as usize) % num_vertices;
					(source, target, value as u64)
				})
				.collect();
			let csr: Csr<u64> = Csr::build(num_vertices, edges.iter().copied());

			let mut expected: Vec<Vec<u64>> = vec![Vec::new(); num_vertices];
			for &(source, _, value) in edges.iter() {
				expected[source].push(value);
			}

			for source in 0..num_vertices {
				let mut got: Vec<u64> = Vec::from_iter(csr[source].iter().map(|edge| edge.value));
				got.sort();
				expected[source].sort();
				assert_eq!(got, expected[source], "edges: {edges:?}");
			}
		}
	}
}
