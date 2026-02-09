.PHONY: default FORCE examples

default:

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:

examples: target/debug/liblog_mechanic.a
	$(MAKE) --directory $@

include/log_mechanic.generated.hpp: FORCE
	cbindgen --config cbindgen.toml --crate log-mechanic > $@

target/debug/liblog_mechanic.a: FORCE
	cargo build --no-default-features
