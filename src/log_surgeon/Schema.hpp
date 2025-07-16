#ifndef LOG_SURGEON_SCHEMA_HPP
#define LOG_SURGEON_SCHEMA_HPP

#include <memory>
#include <string>

#include <log_surgeon/SchemaParser.hpp>

namespace log_surgeon {
/**
 * Contains various functions to load a schema and manipulate it
 * programmatically. The majority of use cases should not require users to
 * modify the schema programmatically, as they can simply use a schema file.
 */
class Schema {
public:
    Schema();

    explicit Schema(std::string const& schema_file_path);

    /**
     * Parses `delimiters_schema` as if it were its own entire schema file. Then extracts the
     * `DelimiterStringAST` from the resulting `SchemaAST` and adds it to `m_delimiters` in
     * `m_schema_ast`.
     * @param delimiters_schema String of delimiters.
     * @throw std::invalid_argument if the delimiters string provided is invalid.
     */
    auto add_delimiters(std::string_view delimiters_schema) const -> void;

    /**
     * Parses `var_schema` as if it were its own entire schema file. Then extracts the
     * `SchemaVarAST` from the resulting `SchemaAST` and adds it to `m_schema_vars` in
     * `m_schema_ast`. Position in `m_schema_vars` is determined by the `priority` (`priority` == -1
     * to set to lowest).
     * @param var_schema String containing variable name + ":" + variable regex.
     * @param inverse_priority Variable priority where 0 is highest.
     * @throw std::invalid_argument if the variable string provided is invalid.
     * @throw std::invalid_argument if the priority provided is out of bounds.
     */
    auto add_variable(std::string_view var_schema, int inverse_priority) const -> void;

    /**
     * Transfers ownership of the previously built schema_ast to the caller and
     * replaces it with an empty schema_ast to be used by this schema object in
     * the future
     * @return the previously built schema_ast
     */
    [[nodiscard]] auto release_schema_ast_ptr() -> std::unique_ptr<SchemaAST> {
        auto old_schema_ast = std::move(m_schema_ast);
        m_schema_ast = std::make_unique<SchemaAST>();
        return old_schema_ast;
    }

private:
    std::unique_ptr<SchemaAST> m_schema_ast;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_SCHEMA_HPP
