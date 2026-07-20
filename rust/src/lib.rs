#[macro_use(debug)]
extern crate tracing;

#[macro_use(Serialize, Deserialize)]
extern crate serde;

#[macro_use]
pub mod utils;

pub mod dfa;
pub mod ffi;
pub mod interval_tree;
pub mod lexer;
pub mod log_event;
pub mod nfa;
pub mod parser;
pub mod parsing_spec;
pub mod regex;
pub mod search;

pub mod c_interface;

#[cfg(feature = "python")]
pub mod python_interface;

/// Registers a global [`tracing`] [`tracing_subscriber::fmt::Subscriber`]
/// with environment variable `LOG_SURGEON_LOG`.
///
/// For example, set `LOG_SURGEON_LOG=log_surgeon=info` to show `info` and higher level messages.
/// See [`tracing_subscriber::filter::EnvFilter`] for more details on the syntax for the environment variable.
///
/// There can only be one global subscriber, and it can only be set once
/// (by [`tracing_subscriber::fmt::SubscriberBuilder::init`]/`try_init`).
///
/// This global subscriber also includes records from <https://docs.rs/log/latest/log/>.
///
/// See also:
/// - <https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/struct.SubscriberBuilder.html#method.init>
/// - <https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives>
///
pub fn enable_tracing() {
	use tracing_subscriber::filter::EnvFilter;
	use tracing_subscriber::fmt::format::FmtSpan;

	// Note: [`tracing_subscriber::fmt::SubscriberBuilder::without_time`]
	// disables _both_ timestamps per log message _and_ timing events/showing their duration.
	// Call `.with_timer()` with an empty formatter to _just_ disable timestamps in each log printed.
	tracing_subscriber::fmt()
		.with_timer(())
		.with_target(false)
		.with_span_events(FmtSpan::CLOSE)
		.with_file(true)
		.with_line_number(true)
		.with_env_filter(EnvFilter::from_env("LOG_SURGEON_LOG"))
		.init()
}

/// The length of an array (`usize`) is at most `isize::MAX` (equivalently `usize::MAX / 2`):
///
/// 1. Rust's only real implementation is rustc,
/// 2. rustc is built on LLVM,
/// 3. LLVM fundamentally assumes that pointer subtraction returns a value in the C `ptrdiff_t` type,
/// 4. so objects/arrays are at most half the address space,
/// 5. and an object/array/vector of size `(isize::MAX as usize) + 1` would violate this.
///
/// Of course, `usize::MAX / 2` states is also massive on 64-bit systems,
/// and for practical purposes we simply wouldn't reach that length.
const _LENGTH_AT_MOST_HALF_USIZE_MAX: () = ();

/// We assume we're on at least a 32-bit platform for lossless `u32` <-> `usize` casts.
/// The only "smaller" platforms rustc supports are 16-bits.
const _USIZE_AT_LEAST_32_BITS: () = {
	assert!(usize::BITS >= u32::BITS, "possibly lossy cast from `u32` to `usize`");
};
