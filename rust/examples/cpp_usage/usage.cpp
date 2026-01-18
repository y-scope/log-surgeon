#include <cassert>
#include <cstdio>

#include "log_mechanic.hpp"

using namespace clp::log_mechanic;

int main() {
	Schema* schema { clp_log_mechanic_schema_new() };

	clp_log_mechanic_schema_add_rule(schema, "hello", "abc|def");

	Lexer* lexer { clp_log_mechanic_lexer_new(schema) };

	LogComponent component {};
	size_t pos { 0 };

	clp_log_mechanic_lexer_next_token(lexer, "def", &pos, &component);
	assert(component.rule == 1);
	assert(component.start + 3 == component.end);

	printf("good!\n");

	clp_log_mechanic_lexer_delete(lexer);
	clp_log_mechanic_schema_delete(schema);

	return 0;
}
