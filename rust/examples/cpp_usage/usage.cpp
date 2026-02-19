#include <cassert>
#include <cstdio>
#include <string_view>
#include <optional>
#include <iostream>

#include "log_mechanic.hpp"

using namespace log_mechanic;

int main() {
	Box<Schema> schema { logmech_schema_new() };

	logmech_schema_add_rule(schema, "hello", "abc|d(?<foo>[a-z])f");

	ParserHandle parser { schema } ;

	std::string_view const input { "def foobarbaz" };
	size_t pos { 0 };

	std::optional<EventHandle> maybe_event { parser.next_event(input, &pos) };
	assert(maybe_event.has_value());
	assert(pos == input.length());

	EventHandle event { *maybe_event };
	assert(event.log_type() == "%0:hello% foobarbaz");

	std::optional<Variable> maybe_var { event[0] };
	assert(maybe_var.has_value());

	Variable var { *maybe_var };
	assert(var.name == "hello");

	Variable::CaptureIterator begin { var.begin() };

	assert(var[begin[0]][0].name.as_cpp_view() == "foo");
	assert(var[begin[0]][0].lexeme.as_cpp_view() == "e");

	assert(begin + 1 == var.end());

	assert(!event[1].has_value());

	printf("good!\n");

	logmech_schema_drop(schema);

	return 0;
}
