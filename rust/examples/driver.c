#include <assert.h>
#include <string.h>
#include <stdio.h>

#include "log_mechanic.h"

CSlice_c_char string_view(char const*);

int main() {
	Schema* schema = clp_log_mechanic_schema_new();

	clp_log_mechanic_schema_add_rule(schema, string_view("hello"), string_view("abc|def"));

	Lexer* lexer = clp_log_mechanic_lexer_new(schema);

	LogComponent component = {};
	size_t pos = 0;

	clp_log_mechanic_lexer_next_token(lexer, string_view("def"), &pos, &component);
	assert(component.rule == 1);
	assert(component.start + 3 == component.end);

	printf("good!\n");

	clp_log_mechanic_lexer_delete(lexer);
	clp_log_mechanic_schema_delete(schema);
	return 0;
}

CSlice_c_char string_view(char const* pointer) {
	return clp_log_mechanic_c_string_view(pointer);
}
