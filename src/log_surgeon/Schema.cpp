#include "Schema.hpp"

#include <string>

namespace log_surgeon {
Schema::Schema() {
    m_schema_ast = std::make_unique<SchemaAST>();
}

Schema::Schema(std::string const& schema_file_path)
        : m_schema_ast{SchemaParser::try_schema_file(schema_file_path)} {}

auto Schema::add_variable(std::string_view const var_schema, int const inverse_priority) const
        -> void {
    auto const schema_ast{SchemaParser::try_schema_string(var_schema)};
    if (schema_ast->m_schema_vars.empty()) {
        throw std::invalid_argument("No variable found in schema string");
    }
    if (-1 > inverse_priority
        || static_cast<int>(m_schema_ast->m_schema_vars.size()) < inverse_priority)
    {
        throw std::invalid_argument("Priority is out of bounds.");
    }
    m_schema_ast->add_schema_var(std::move(schema_ast->m_schema_vars[0]), inverse_priority);
}

auto Schema::add_delimiters(std::string_view const delimiters_schema) const -> void {
    auto const schema_ast{SchemaParser::try_schema_string(delimiters_schema)};
    if (schema_ast->m_delimiters.empty()) {
        throw std::invalid_argument("No delimiters found in schema string");
    }
    m_schema_ast->add_delimiters(std::move(schema_ast->m_delimiters[0]));
}
}  // namespace log_surgeon
