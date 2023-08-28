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
    Schema() = default;

    explicit Schema(std::string const& schema_file_path);

    /**
     * Parses var_name+":"+regex as if it were its own entire schema file. Then
     * extracts the SchemaVarAST from the resulting SchemaAST and adds it to
     * m_schema_vars in m_schema_ast. Position in m_schema_vars is determined by
     * the priority (priority == -1 to set to lowest).
     * @param var_name
     * @param regex
     * @param priority
     */
    auto add_variable(std::string const& var_name, std::string const& regex, int priority) -> void;

    /* Work in progress API to modify a schema object

    auto remove_variable (std::string var_name) -> void;

    auto add_variables (std::map<std::string, std::string> variables) -> void;

    auto remove_variables (std::map<std::string, std::string> variables) -> void;

    auto remove_all_variables () -> void;

    auto set_variables (std::map<std::string, std::string> variables) -> void;

    auto add_delimiter (char delimiter) -> void;

    auto remove_delimiter (char delimiter) -> void;

    auto add_delimiters (std::vector<char> delimiter) -> void;

    auto remove_delimiters (std::vector<char> delimiter) -> void;

    auto remove_all_delimiters () -> void;

    auto set_delimiters (std::vector<char> delimiters) -> void;

    auto clear ();
    */

    [[nodiscard]] auto get_schema_ast_ptr() const -> SchemaAST const* { return m_schema_ast.get(); }

private:
    std::unique_ptr<SchemaAST> m_schema_ast;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_SCHEMA_HPP
