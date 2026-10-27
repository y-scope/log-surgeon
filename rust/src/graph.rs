mod csr;
mod dominators;
mod tarjan_scc;

pub use csr::Csr;
pub use dominators::Dominators;
pub use tarjan_scc::TarjanSccs;
pub use tarjan_scc::TarjanVertex;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct DfsIndex(pub usize);

impl DfsIndex {
	pub const INVALID: Self = Self(usize::MAX);
}
