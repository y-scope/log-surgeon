#include "Schema.hpp"

#include <string>

namespace log_surgeon {
Schema::Schema() {
    m_schema_ast = std::make_unique<SchemaAST>();
}

Schema::Schema(std::string const& schema_file_path)
        : m_schema_ast{SchemaParser::try_schema_file(schema_file_path)} {}

auto Schema::add_variable(std::string_view const var_schema, int const priority) const -> void {
    auto const schema_ast = SchemaParser::try_schema_string(var_schema);
    m_schema_ast->add_schema_var(std::move(schema_ast->m_schema_vars[0]), priority);
}

auto Schema::add_delimiters(std::string_view const delimiters_schema) const -> void {
    auto const schema_ast = SchemaParser::try_schema_string(delimiters_schema);
    m_schema_ast->add_delimiters(std::move(schema_ast->m_delimiters[0]));
}
}  // namespace log_surgeon
