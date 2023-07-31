#include "Schema.hpp"

#include <string>

namespace log_surgeon {
Schema::Schema(std::string const& schema_file_path)
        : m_schema_ast{SchemaParser::try_schema_file(schema_file_path)} {}
}  // namespace log_surgeon
