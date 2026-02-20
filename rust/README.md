Rust Implementation of Log Surgeon
==================================
This is **very much** work in progress.
Please don't treat anything here as indicative of style or quality.

```bash
# Basic `cargo` commands.
cargo check
cargo build
cargo test

# Check/build/test without Python C extension module
cargo build --no-default-features

# Run `src/bin/playground.rs`.
cargo run --bin playground

# Python environment
python3 -m venv .env
source .env/bin/activate
pip install maturin
maturin develop
./examples/python/usage.py
python3 -m unittest discover tests/python
```
