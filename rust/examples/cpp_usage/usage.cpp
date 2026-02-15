#include <cassert>
#include <cstdio>
#include <string_view>
#include <optional>
#include <iostream>

#include "log_mechanic.hpp"

using namespace clp::log_mechanic;

int main() {
	Box<Schema> schema { clp_log_mechanic_schema_new() };

	clp_log_mechanic_schema_add_rule(schema, "hello", "abc|d(?<foo>[a-z])f");

	ParserHandle parser { schema } ;

	std::string_view const input { "def foobarbaz" };
	size_t pos { 0 };

	std::optional<EventHandle> event { parser.next_event(input, &pos) };
	assert(event.has_value());
	assert(pos == input.length());

	assert(event->log_type() == "%0:hello% foobarbaz");

	assert((*event)[0].has_value());
	assert((*event)[0]->name == "hello");

	assert((*event)[0]->capture(0).has_value());
	assert((*event)[0]->capture(0)->name.as_cpp_view() == "foo");
	assert((*event)[0]->capture(0)->lexeme.as_cpp_view() == "e");

	assert(!(*event)[0]->capture(1).has_value());

	assert(!(*event)[1].has_value());

	printf("good!\n");

	clp_log_mechanic_schema_drop(schema);

	return 0;
}
