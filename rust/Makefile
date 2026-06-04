.PHONY: default FORCE examples cargo-test test format lint

default:

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:

examples: target/debug/liblog_surgeon.a
	$(MAKE) --directory $@

cargo-tests:
	cargo test

python-tests:
	source .env/bin/activate && maturin develop && python3 -m unittest discover tests/python

example-tests: examples
	./examples/cpp_usage/usage.o

test: cargo-tests python-tests example-tests

cxx/log_surgeon/generated_bindings.hpp: FORCE
	cbindgen --config cbindgen.toml --crate log-surgeon > $@

target/debug/liblog_surgeon.a: FORCE
	cargo build --no-default-features

format:
	cargo +nightly fmt
	task lint:fix-cpp-format-log-surgeon

lint:
	task lint:check-cpp-static-log-surgeon
