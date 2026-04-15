# Development Guide

> This is **very much** work in progress.
> Please don't treat anything here as indicative of style or quality.

## Building the Rust library

```bash
# Build (C FFI staticlib, no Python)
cargo build

# Run Rust tests
cargo test
```

## Enabling Tracing
```
# C/C++
log_surgeon_enable_tracing()

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

## Python extension

### Setup

```bash
python3 -m venv .env
source .env/bin/activate
pip install maturin
```

### Build and test

```bash
# Build and install into the venv
maturin develop --release

# Run examples
python3 examples/python_usage/usage.py

# Run tests
python3 -m unittest discover tests/python
```

### Build a wheel locally

```bash
maturin build --release --out dist
pip install dist/*.whl
```

### Type stubs

Since the Python module is compiled from Rust, IDEs can't inspect the source
code to provide autocomplete, parameter hints, or docstrings. The type stub file
at `python/log_surgeon/__init__.pyi` provides this information. When adding or
changing methods in `src/python_interface.rs`, update the stubs to match.
