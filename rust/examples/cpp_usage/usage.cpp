#include <cassert>
#include <cstdio>

#include "log_mechanic.hpp"

using namespace clp::log_mechanic;

int main() {
	Box<Schema> schema { clp_log_mechanic_schema_new() };

	clp_log_mechanic_schema_add_rule(schema, "hello", "abc|def");

	Box<Lexer> lexer { clp_log_mechanic_lexer_new(schema) };

	size_t pos { 0 };

	CLogFragment fragment { clp_log_mechanic_lexer_next_fragment(lexer, "def", &pos) };
	assert(fragment.rule == 1);
	assert(fragment.start + 3 == fragment.end);

	printf("good!\n");

	clp_log_mechanic_lexer_delete(lexer);
	clp_log_mechanic_schema_delete(schema);

	return 0;
}
