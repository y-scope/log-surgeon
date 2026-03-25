#[macro_use(debug)]
extern crate tracing;

pub mod dfa;
pub mod interval_tree;
pub mod lexer;
pub mod log_event;
pub mod log_type;
pub mod nfa;
pub mod parser;
pub mod query;
pub mod regex;
pub mod schema;

pub mod c_interface;
#[cfg(feature = "python")]
pub mod python_interface;

/// A `usize` that comes from a "length" of things is at most `isize::MAX as usize` (aka `usize::MAX / 2`):
///
/// 1. Rust's only real implementation is rustc,
/// 2. rustc is built on LLVM,
/// 3. LLVM fundamentally assumes that pointer subtraction returns a value in the C `ptrdiff_t` type,
/// 4. so objects/arrays are at most half the address space,
/// 5. and an array/vector of length `(isize::MAX as usize) + 1` would violate this.
///
/// Of course, `usize::MAX / 2` states is also massive on 64-bit systems,
/// and for practical purposes we simply wouldn't reach that length.
const _LENGTH_AT_MOST_HALF_USIZE_MAX: () = ();

/// We assume we're on at least a 32-bit platform for lossless `u32` <-> `usize` casts.
/// The only "smaller" platforms rustc supports are 16-bits.
const _USIZE_AT_LEAST_32_BITS: () = {
	assert!(usize::BITS >= u32::BITS, "possibly lossy u32 to usize cast");
};
