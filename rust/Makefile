.PHONY: default FORCE examples

default:

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:

examples: examples/driver.o

examples/driver.o: examples/driver.c target/debug/liblog_mechanic.a
	gcc -std=c17 -g \
		-Iinclude \
		-Ltarget/debug \
		-o $@ $^

target/debug/liblog_mechanic.a: FORCE
	cargo build

include/log_mechanic.h: FORCE
	cbindgen --config cbindgen.toml --crate log-mechanic > $@
