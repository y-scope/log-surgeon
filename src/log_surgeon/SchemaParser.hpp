#ifndef SCHEMAPARSER_HPP
#define SCHEMAPARSER_HPP

#include <utility>

// Project headers
#include "LALR1Parser.hpp"

namespace log_surgeon {
// ASTs used in SchemaParser AST
class SchemaAST : public ParserAST {
public:
    // Constructor
    SchemaAST() = default;

    /// TODO: shouldn't this add delimiters instead of setting it?
    auto set_delimiters(std::unique_ptr<ParserAST> delimiters_in) -> void {
        m_delimiters = std::move(delimiters_in);
    }

    auto add_schema_var(std::unique_ptr<ParserAST> schema_var) -> void {
        m_schema_vars.push_back(std::move(schema_var));
    }

    std::vector<std::unique_ptr<ParserAST>> m_schema_vars;
    std::unique_ptr<ParserAST> m_delimiters;
    std::string m_file_path;
};

class IdentifierAST : public ParserAST {
public:
    // Constructor
    explicit IdentifierAST(char character) { m_name.push_back(character); }

    auto add_character(char character) -> void { m_name.push_back(character); }

    std::string m_name;
};

class SchemaVarAST : public ParserAST {
public:
    // Constructor
    SchemaVarAST(std::string name,
                 std::unique_ptr<finite_automata::RegexAST<finite_automata::RegexNFAByteState>>
                         regex_ptr,
                 uint32_t line_num)
        : m_line_num(line_num),
          m_name(std::move(name)),
          m_regex_ptr(std::move(regex_ptr)) {}

    uint32_t m_line_num;
    std::string m_name;
    std::unique_ptr<finite_automata::RegexAST<finite_automata::RegexNFAByteState>> m_regex_ptr;
};

class DelimiterStringAST : public ParserAST {
public:
    // Constructor
    explicit DelimiterStringAST(uint32_t delimiter) { m_delimiters.push_back(delimiter); }

    auto add_delimiter(uint32_t delimiter) -> void { m_delimiters.push_back(delimiter); }

    std::vector<uint32_t> m_delimiters;
};

class SchemaParser
    : public LALR1Parser<finite_automata::RegexNFAByteState, finite_automata::RegexDFAByteState> {
public:
    // Constructor
    SchemaParser();

    /**
     * A semantic rule that needs access to soft_reset()
     * @param m
     * @return std::unique_ptr<SchemaAST>
     */
    auto existing_schema_rule(NonTerminal* m) -> std::unique_ptr<SchemaAST>;

    /**
     * Parse a user defined schema to generate a schema AST used for generating the log lexer
     * @param reader
     * @return std::unique_ptr<SchemaAST>
     */
    auto generate_schema_ast(Reader& reader) -> std::unique_ptr<SchemaAST>;

    /**
     * Wrapper around generate_schema_ast()
     * @param schema_file_path
     * @return std::unique_ptr<SchemaAST>
     */
    static auto try_schema_file(std::string const& schema_file_path)
            -> std::unique_ptr<SchemaAST>;

private:
    /**
     * After lexing half of the buffer, reads into that half of the buffer and changes variables
     * accordingly
     * @param next_children_start
     */
    auto soft_reset(uint32_t& next_children_start) -> void;

    /**
     * Add all lexical rules needed for schema lexing
     */
    auto add_lexical_rules() -> void;

    /**
     * Add all productions needed for schema parsing
     */
    auto add_productions() -> void;
};
} // namespace log_surgeon

#endif // SCHEMAPARSER_HPP
