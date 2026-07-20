Log Surgeon (Rust)
==================
Rust implementation of [log-surgeon](https://github.com/y-scope/log-surgeon),
a high-performance log parsing library, replacing the C++ implementation.
This crate provides the core parsing engine and exposes it through C/C++ and Python bindings.

- Regex, (tagged) NFA/DFA inspired by [re2c][re2c].
- [Parsing Specification][parsing-spec]-driven log parser.
- Search query decomposition based on the parsing specification.
- C/C++ bindings (`/cxx`).
- CPython bindings (extension module) through [PyO3][pyo3] and [Maturin][maturin] (`src/python_interface.rs`, `/python`).
	- See the [Python README][python-readme] for more details.

## Table of Contents
The rest of this document is a quickstart development guide.

- [Prerequisites](#prerequisites)
- [Building](#building)
- [Debugging](#debugging)

## Prerequisites
- Stable rust toolchain; 1.97.1+ as of 2026 July 16.
	- If using [rustup][rustup], the `stable` toolchain can be updated as `rustup update stable`.
- CPython 3.12+.
- C++ compiler - GCC 10+ or Clang 7+.

## Building
```bash
# Check/Build the Rust library and C FFI static library
cargo check
cargo build

# Rust tests
cargo test
cargo test -- [--nocapture] [test_name]

# Python setup
python3 -m venv .env
source .env/bin/activate
pip install maturin

# Python build
maturin develop --release

# Python tests
python3 -m unittest discover tests/python [-k test_name]
```

## Debugging

### Enable Tracing
```
# Rust
log_surgeon::enable_tracing();

# C/C++
log_surgeon_enable_tracing();

# Python
import log_surgeon
log_surgeon.log_surgeon_ffi.enable_tracing()
```

Then, set and export the environment variable `LOG_SURGEON_LOG` according to
[`tracing_subscriber::filter::EnvFilter`][rust-tracing-env-filter],
e.g.

```bash
export LOG_SURGEON_LOG=log_surgeon=trace
```

In this crate, `#[tracing::instrument]` spans are set to level `trace` or `debug`;
i.e. running with `LOG_SURGEON_LOG=log_surgeon=info` or higher should not include instrumentation logs.

[parsing-spec]: parsing-specification.md
[python-readme]: python/README.md
[re2c]: https://re2c.org/
[pyo3]: https://pyo3.rs/
[maturin]: https://www.maturin.rs/
[rustup]: https://rustup.rs/
[rust-tracing-env-filter]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
