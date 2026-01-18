.PHONY: default FORCE examples

default:

# https://www.gnu.org/software/make/manual/html_node/Force-Targets.html
FORCE:

examples:
	$(MAKE) --directory $@

include/log_mechanic.generated.hpp: FORCE
	cbindgen --config cbindgen.toml --crate log-mechanic > $@
