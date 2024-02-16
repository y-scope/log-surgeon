#include <iostream>
#include <string>

#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/Schema.hpp>

using log_surgeon::finite_automata::RegexDFA;
using log_surgeon::finite_automata::RegexDFAByteState;
using log_surgeon::finite_automata::RegexNFA;
using log_surgeon::finite_automata::RegexNFAByteState;
using log_surgeon::lexers::ByteLexer;
using log_surgeon::ParserAST;
using log_surgeon::SchemaVarAST;
using std::string;
using std::unique_ptr;

auto get_intersect_for_query(log_surgeon::Schema& schema, std::string const& search_string)
        -> void {
    std::map<uint32_t, std::string> m_id_symbol;
    RegexNFA<RegexNFAByteState> nfa1;
    for (unique_ptr<ParserAST> const& parser_ast : schema.get_schema_ast_ptr()->m_schema_vars) {
        auto* var_ast = dynamic_cast<SchemaVarAST*>(parser_ast.get());
        ByteLexer::Rule rule(m_id_symbol.size(), std::move(var_ast->m_regex_ptr));
        m_id_symbol[m_id_symbol.size()] = var_ast->m_name;
        rule.add_ast(&nfa1);
    }
    std::unique_ptr<RegexDFA<RegexDFAByteState>> dfa1 = ByteLexer::nfa_to_dfa(nfa1);
    std::string processed_search_string;
    // Replace all * with .*
    for (char const& c : search_string) {
        if (c == '*') {
            processed_search_string.push_back('.');
        }
        processed_search_string.push_back(c);
    }
    log_surgeon::Schema schema2;
    schema2.add_variable("search", processed_search_string, -1);
    RegexNFA<RegexNFAByteState> nfa2;
    for (unique_ptr<ParserAST> const& parser_ast : schema2.get_schema_ast_ptr()->m_schema_vars) {
        auto* schema_var_ast = dynamic_cast<SchemaVarAST*>(parser_ast.get());
        ByteLexer::Rule rule(0, std::move(schema_var_ast->m_regex_ptr));
        rule.add_ast(&nfa2);
    }
    std::unique_ptr<RegexDFA<RegexDFAByteState>> dfa2 = ByteLexer::nfa_to_dfa(nfa2);
    std::set<uint32_t> schema_types = dfa1->get_intersect(dfa2);
    std::cout << search_string << ":";
    for (auto const& schema_type : schema_types) {
        std::cout << m_id_symbol[schema_type] << ",";
    }
    std::cout << std::endl;
}

auto main() -> int {
    for (int i = 0; i < 2; i++) {
        log_surgeon::Schema schema;
        if (0 == i) {
            std::cout << "--Schema1--" << std::endl;
            schema.add_variable("int", "\\-{0,1}[0-9]+", -1);
            schema.add_variable("float", "\\-{0,1}[0-9]+\\.[0-9]+", -1);
            schema.add_variable("hex", "[a-fA-F]+", -1);
            schema.add_variable("hasNumber", ".*\\d.*", -1);
            schema.add_variable("equals", ".*=.*[a-zA-Z0-9].*", -1);
            schema.add_variable("logLevel", "(INFO)|(DEBUG)|(WARN)|(ERROR)|(TRACE)|(FATAL)", -1);
        } else {
            std::cout << "--Schema2--" << std::endl;
            schema.add_variable("v1", "1", -1);
            schema.add_variable("v2", "2", -1);
            schema.add_variable("v3", "3", -1);
            schema.add_variable("v4", "abc12", -1);
            schema.add_variable("v5", "23def", -1);
            schema.add_variable("v6", "123", -1);
        }
        get_intersect_for_query(schema, "*1*");
        get_intersect_for_query(schema, "*a*");
        get_intersect_for_query(schema, "*a1*");
        get_intersect_for_query(schema, "*=*");
        get_intersect_for_query(schema, "abc123");
        get_intersect_for_query(schema, "=");
        get_intersect_for_query(schema, "1");
        get_intersect_for_query(schema, "a*1");
        get_intersect_for_query(schema, "a1");
    }
    return 0;
}
