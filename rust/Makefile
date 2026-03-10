.PHONY: default FORCE examples cargo-test test

default:

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:

examples: target/debug/liblog_mechanic.a
	$(MAKE) --directory $@

cargo-tests:
	cargo test

python-tests:
	source .env/bin/activate && maturin develop && python3 -m unittest discover tests/python

example-tests: examples
	./examples/cpp_usage/usage.o

test: cargo-tests python-tests example-tests

cpp_ffi/log_mechanic/generated_bindings.hpp: FORCE
	cbindgen --config cbindgen.toml --crate log-mechanic > $@

target/debug/liblog_mechanic.a: FORCE
	cargo build --no-default-features
