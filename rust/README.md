# log-surgeon (Rust)

Rust implementation of [log-surgeon](https://github.com/y-scope/log-surgeon), a
high-performance log parsing library. This crate provides the core parsing
engine and exposes it through multiple language bindings.

## Components

- **Core library** — regex engine, NFA/DFA construction, schema-driven log
  parser.
- **C FFI** (`cpp_ffi/`) — C-compatible static library for use from C/C++ code.
- **Python bindings** (`python/`) — Python extension module built with
  [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs). See the
  [Python README](python/README.md) for installation and API docs.

## Building

```bash
# Build the Rust library and C FFI static library
cargo build

# Run tests
cargo test
```

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development guide, including
Python extension build instructions.
