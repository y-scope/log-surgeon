#include <string.h>
#include "log_mechanic.h"

CSlice_c_char string_view(char const*);

int main() {
	Schema* schema = clp_log_surgeon_schema_new();

	clp_log_surgeon_schema_add_rule(schema, string_view("hello"), string_view("abc|def"));

	Nfa* nfa = clp_log_surgeon_nfa_for_schema(schema);
	clp_log_surgeon_nfa_debug(nfa);

	clp_log_surgeon_nfa_delete(nfa);
	clp_log_surgeon_schema_delete(schema);
	return 0;
}

CSlice_c_char string_view(char const* pointer) {
	return clp_log_surgeon_c_string_view(pointer);
}
