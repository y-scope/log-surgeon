#include "Schema.hpp"

#include <string>

namespace log_surgeon {
Schema::Schema() {
    m_schema_ast = std::make_unique<SchemaAST>();
}

Schema::Schema(std::string const& schema_file_path)
        : m_schema_ast{SchemaParser::try_schema_file(schema_file_path)} {}

auto Schema::add_schema_line(std::string const& schema_line, int priority) -> void {
    std::unique_ptr<SchemaAST> schema_ast = SchemaParser::try_schema_string(schema_line);
    if ("delimiters" == schema_line.substr(0, std::string("delimiters").size())) {
        m_schema_ast->add_delimiters(std::move(schema_ast->m_delimiters[0]));
    } else {
        m_schema_ast->add_schema_var(std::move(schema_ast->m_schema_vars[0]), priority);
    }
}
}  // namespace log_surgeon
