#include <cassert>
#include <cstdio>
#include <cstring>

#include "log_mechanic.hpp"

using namespace clp::log_mechanic;

int main() {
	Box<Schema> schema { clp_log_mechanic_schema_new() };

	clp_log_mechanic_schema_add_rule(schema, "hello", "abc|def");

	Box<Parser> parser { clp_log_mechanic_parser_new(schema) };

	size_t pos { 0 };

	char const* input = "def";

	Box<LogEvent> event { clp_log_mechanic_parser_next_event(parser, input, &pos) };
	assert(clp_log_mechanic_event_token_count(event) == 1);
	Token const* token { clp_log_mechanic_event_token(event, 0) };
	assert(clp_log_mechanic_event_token_rule(token) == 1);
	assert(pos == strlen(input));

	printf("good!\n");

	clp_log_mechanic_event_delete(event);
	clp_log_mechanic_parser_delete(parser);
	clp_log_mechanic_schema_delete(schema);

	return 0;
}
