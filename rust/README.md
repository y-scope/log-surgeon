Log Surgeon (Rust)
==================
Rust implementation of [log-surgeon](https://github.com/y-scope/log-surgeon),
a high-performance log parsing library.
This crate provides the core parsing engine and exposes it through C/C++ and Python bindings.

**Still a work in progress!**

## Components
- Regex, (tagged) NFA/DFA based on [re2c][re2c].
- Schema-driven log parser.
- C/C++ bindings (`/cxx`).
- CPython extension module through [PyO3][pyo3] and [Maturin][maturin] (`src/python_interface.rs`, `/python`).
	- See the [Python README](python/README.md) for more details.

## Building
```bash
# Check/Build the Rust library and C FFI static library
cargo check
cargo build

# Rust tests
cargo test
cargo test -- --nocapture
cargo test -- [--nocapture] test_name

# Python setup
python3 -m venv .env
source .env/bin/activate
pip install maturin

# Python build
maturin develop --release

# Python tests
python3 -m unittest discover tests/python
```

## Development

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
[`tracing_subscriber::filter::EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html),
e.g.

```bash
export LOG_SURGEON_LOG=log_surgeon=trace
```

`#[tracing::instrument]` spans should be set to level `trace` in this crate.

[re2c]: https://re2c.org/
[pyo3]: https://pyo3.rs/
[maturin]: https://www.maturin.rs/
