.PHONY: default FORCE examples cargo-test test

default:

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:

examples: target/debug/liblog_mechanic.a
	$(MAKE) --directory $@

cargo-test:
	cargo test

test: cargo-test examples
	./examples/cpp_usage/usage.o

include/log_mechanic.generated.hpp: FORCE
	cbindgen --config cbindgen.toml --crate log-mechanic > $@

target/debug/liblog_mechanic.a: FORCE
	cargo build --no-default-features
