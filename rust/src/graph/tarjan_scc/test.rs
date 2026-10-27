use super::*;
use crate::utils::test::rng;

/// Runs Tarjan's SCC algorithm over `adj` using the given entry vertices.
fn sccs(adj: &[Vec<usize>], entries: &[usize]) -> TarjanSccs {
	let vertices: Vec<usize> = (0..adj.len()).collect();
	TarjanSccs::tarjan_scc(&vertices, entries.iter().copied(), |v| {
		adj[*v].iter().copied()
	})
}

/// Runs Tarjan's SCC algorithm over `adj` using every vertex as an entry.
fn all_sccs(adj: &[Vec<usize>]) -> TarjanSccs {
	let entries: Vec<usize> = (0..adj.len()).collect();
	sccs(adj, &entries)
}

/// Computes SCCs by mutual reachability, as an independent oracle.
/// The resulting partition is normalized (both the list and each component sorted).
fn sccs_brute_force(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
	let n: usize = adj.len();

	// `reachable[i][j]` is true iff `j` is reachable from `i` (including `i` itself).
	let mut reachable: Vec<Vec<bool>> = vec![vec![false; n]; n];
	for (i, reachable_i) in reachable.iter_mut().enumerate() {
		let mut stack: Vec<usize> = vec![i];
		while let Some(u) = stack.pop() {
			if reachable_i[u] {
				continue;
			}
			reachable_i[u] = true;
			for &v in adj[u].iter() {
				if !reachable_i[v] {
					stack.push(v);
				}
			}
		}
	}

	let mut seen: Vec<bool> = vec![false; n];
	let mut partition: Vec<Vec<usize>> = Vec::new();
	for i in 0..n {
		if seen[i] {
			continue;
		}
		let mut component: Vec<usize> =
			Vec::from_iter((0..n).filter(|&j| !seen[j] && reachable[i][j] && reachable[j][i]));
		for &j in component.iter() {
			seen[j] = true;
		}
		component.sort();
		partition.push(component);
	}
	partition.sort();
	partition
}

/// Normalizes an SCC list into a set partition for order-independent comparison.
fn canonical(mut sccs: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
	for scc in sccs.iter_mut() {
		scc.sort();
	}
	sccs.sort();
	sccs
}

#[test]
fn single_vertex() {
	let result: TarjanSccs = sccs(&[vec![]], &[0]);
	assert_eq!(result.sccs, vec![vec![0]]);
	assert_eq!(result.vertices[0].scc, 0);
	assert_eq!(
		result.vertices[0].encountered_at,
		result.vertices[0].low_link
	);
}

#[test]
fn chain_is_forward_topological() {
	// 0 -> 1 -> 2
	let result: TarjanSccs = sccs(&[vec![1], vec![2], vec![]], &[0]);
	assert_eq!(result.sccs, vec![vec![0], vec![1], vec![2]]);
	assert_eq!(result.vertices[0].scc, 0);
	assert_eq!(result.vertices[1].scc, 1);
	assert_eq!(result.vertices[2].scc, 2);
}

#[test]
fn cycle_is_single_scc() {
	let result: TarjanSccs = sccs(&[vec![1], vec![2], vec![0]], &[0]);
	assert_eq!(result.sccs, vec![vec![0, 1, 2]]);
}

#[test]
fn self_loop_is_single_scc() {
	let result: TarjanSccs = sccs(&[vec![0]], &[0]);
	assert_eq!(result.sccs, vec![vec![0]]);
}

#[test]
fn cycle_with_tail() {
	// 0 -> 1 <-> 2
	let result: TarjanSccs = sccs(&[vec![1], vec![2], vec![1]], &[0]);
	assert_eq!(result.sccs, vec![vec![0], vec![1, 2]]);
}

#[test]
fn two_cycles() {
	// 0 <-> 1, 0 -> 2, 2 <-> 3
	let result: TarjanSccs = sccs(&[vec![1, 2], vec![0], vec![3], vec![2]], &[0]);
	assert_eq!(result.sccs, vec![vec![0, 1], vec![2, 3]]);
}

#[test]
fn disconnected_components() {
	// 0 -> 1, 2 -> 3
	let adj: Vec<Vec<usize>> = vec![vec![1], vec![], vec![3], vec![]];
	let result: TarjanSccs = sccs(&adj, &[0, 2]);
	// The components are independent, so any interleaving is a valid topological order.
	assert_eq!(
		canonical(result.sccs.clone()),
		vec![vec![0], vec![1], vec![2], vec![3]]
	);
}

#[test]
fn unvisited_vertices_are_invalid() {
	// Vertex 2 is never reachable from the sole entry.
	let result: TarjanSccs = sccs(&[vec![1], vec![], vec![1]], &[0]);
	assert_eq!(result.sccs, vec![vec![0], vec![1]]);
	assert_eq!(result.vertices[2].encountered_at, DfsIndex::INVALID);
	assert_eq!(result.vertices[2].low_link, DfsIndex::INVALID);
	assert_eq!(result.vertices[2].scc, usize::MAX);
}

#[test]
fn vertices_within_scc_sorted_by_encounter() {
	// 0 -> 1 -> 2 -> 0, plus 0 <-> 3.
	let result: TarjanSccs = sccs(&[vec![1, 3], vec![2], vec![0], vec![0]], &[0]);
	assert_eq!(result.sccs.len(), 1);
	let scc: &Vec<usize> = &result.sccs[0];
	assert!(scc.is_sorted_by_key(|&i| result.vertices[i].encountered_at));
	assert_eq!(scc[0], 0);
}

#[test]
fn original_indices_are_consistent() {
	let adj: Vec<Vec<usize>> = vec![vec![1, 2], vec![2], vec![0, 2], vec![]];
	let result: TarjanSccs = all_sccs(&adj);
	for (i, vertex) in result.vertices.iter().enumerate() {
		if vertex.encountered_at == DfsIndex::INVALID {
			continue;
		}
		assert_eq!(result.original_indices[vertex.encountered_at.0], i);
	}
}

#[test]
fn scc_index_matches_position() {
	let adj: Vec<Vec<usize>> = vec![vec![1, 2], vec![0], vec![3], vec![2], vec![]];
	let result: TarjanSccs = all_sccs(&adj);
	for (scc_idx, scc) in result.sccs.iter().enumerate() {
		for &i in scc.iter() {
			assert_eq!(result.vertices[i].scc, scc_idx);
		}
	}
}

#[test]
fn sccs_are_in_forward_topological_order() {
	let adj: Vec<Vec<usize>> = vec![vec![1, 2], vec![3], vec![3], vec![4], vec![]];
	let result: TarjanSccs = all_sccs(&adj);
	for (u, successors) in adj.iter().enumerate() {
		for &v in successors.iter() {
			let (su, sv): (usize, usize) = (result.vertices[u].scc, result.vertices[v].scc);
			if su != sv {
				// An edge between distinct SCCs points "forward" in the output.
				assert!(su < sv, "edge {u} -> {v}: scc {su} !< scc {sv}");
			}
		}
	}
}

#[test]
fn matches_brute_force_on_crafted_graphs() {
	let graphs: Vec<Vec<Vec<usize>>> = vec![
		vec![vec![]],
		vec![vec![0]],
		vec![vec![1], vec![2], vec![0]],
		vec![vec![1, 2], vec![2], vec![0, 2], vec![]],
		vec![vec![1], vec![], vec![1], vec![2]],
		vec![vec![1, 2], vec![3], vec![3], vec![0, 4], vec![]],
	];
	for adj in graphs {
		let result: TarjanSccs = all_sccs(&adj);
		assert_eq!(
			canonical(result.sccs.clone()),
			sccs_brute_force(&adj),
			"adjacency: {adj:?}"
		);
	}
}

#[test]
fn matches_brute_force_on_random_graphs() {
	let mut next = rng(0x1234_5678_9ABC_DEF0);

	for n in 1..8 {
		for _ in 0..32 {
			let mut adj: Vec<Vec<usize>> = Vec::new();
			for _ in 0..n {
				let degree: usize = (next() as usize) % 5;
				let mut successors: Vec<usize> = Vec::new();
				for _ in 0..degree {
					successors.push((next() as usize) % n);
				}
				adj.push(successors);
			}

			let result: TarjanSccs = all_sccs(&adj);
			assert_eq!(
				canonical(result.sccs.clone()),
				sccs_brute_force(&adj),
				"adjacency: {adj:?}"
			);

			// Every vertex appears in exactly one SCC.
			let mut seen: Vec<bool> = vec![false; n];
			for scc in result.sccs.iter() {
				for &i in scc.iter() {
					assert!(!seen[i], "vertex {i} in multiple SCCs: {adj:?}");
					seen[i] = true;
				}
			}
			assert!(seen.iter().all(|&s| s));
		}
	}
}
