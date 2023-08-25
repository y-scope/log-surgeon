#include "Schema.hpp"

#include <string>

namespace log_surgeon {
Schema::Schema(std::string const& schema_file_path)
        : m_schema_ast{SchemaParser::try_schema_file(schema_file_path)} {}

auto Schema::add_variable(std::string const& var_name, std::string const& regex, int priority)
        -> void {
    std::string unparsed_string = var_name + ":" + regex;
    std::unique_ptr<SchemaAST> schema_ast = SchemaParser::try_schema_string(unparsed_string);
    m_schema_ast->add_schema_var(std::move(schema_ast->m_schema_vars[0]), priority);
}
}  // namespace log_surgeon
