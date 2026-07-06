.PHONY: default FORCE examples cargo-test test format format-rust format-cpp lint

default:

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:

examples: target/debug/liblog_surgeon.a
	$(MAKE) --directory $@

test-cargo:
	cargo test

test-python:
	source .env/bin/activate && maturin develop && python3 -m unittest discover tests/python

test-examples: examples
	./examples/cpp_usage/usage.o

test: test-cargo test-python test-examples

cxx/log_surgeon/generated_bindings.hpp: FORCE
	cbindgen --config cbindgen.toml --crate log-surgeon > $@

target/debug/liblog_surgeon.a: FORCE
	cargo build --no-default-features

format-rust:
	cargo +nightly fmt

format-cpp:
	task lint:fix-cpp-format-log-surgeon

format: format-rust format-cpp

lint:
	task lint:check-cpp-static-log-surgeon
