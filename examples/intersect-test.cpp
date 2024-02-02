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

auto get_intersect_for_query(ByteLexer& lexer, std::string const& search_string) -> void {
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
    RegexNFA<RegexNFAByteState> nfa;
    for (unique_ptr<ParserAST> const& parser_ast : schema2.get_schema_ast_ptr()->m_schema_vars) {
        auto* schema_var_ast = dynamic_cast<SchemaVarAST*>(parser_ast.get());
        ByteLexer::Rule rule(0, std::move(schema_var_ast->m_regex_ptr));
        rule.add_ast(&nfa);
    }
    // TODO: this is obviously bad, but the code needs to be reorganized a lot
    // to fix the fact that DFAs and NFAs can't be used without a lexer
    std::unique_ptr<RegexDFA<RegexDFAByteState>> dfa2 = lexer.nfa_to_dfa(nfa);
    [[maybe_unused]] std::unique_ptr<RegexDFA<RegexDFAByteState>> const& dfa1 = lexer.get_dfa();
    std::set<uint32_t> schema_types = dfa1->get_intersect(dfa2);

    std::cout << search_string << ":";
    for (auto const& schema_type : schema_types) {
        std::cout << lexer.m_id_symbol[schema_type] << ",";
    }
    std::cout << std::endl;
}

auto main() -> int {
    {
        std::cout << "--LEXER1--" << std::endl;
        log_surgeon::Schema schema;
        schema.add_variable("int", "\\-{0,1}[0-9]+", -1);
        schema.add_variable("float", "\\-{0,1}[0-9]+\\.[0-9]+", -1);
        schema.add_variable("hex", "[a-fA-F]+", -1);
        schema.add_variable("hasNumber", ".*\\d.*", -1);
        schema.add_variable("equals", ".*=.*[a-zA-Z0-9].*", -1);
        schema.add_variable("logLevel", "(INFO)|(DEBUG)|(WARN)|(ERROR)|(TRACE)|(FATAL)", -1);

        ByteLexer lexer;
        for (unique_ptr<ParserAST> const& parser_ast : schema.get_schema_ast_ptr()->m_schema_vars) {
            auto* rule = dynamic_cast<SchemaVarAST*>(parser_ast.get());
            std::string& name = rule->m_name;
            if (lexer.m_symbol_id.find(name) == lexer.m_symbol_id.end()) {
                lexer.m_symbol_id[name] = lexer.m_symbol_id.size();
                lexer.m_id_symbol[lexer.m_symbol_id[name]] = name;
            }
            lexer.add_rule(lexer.m_symbol_id[name], std::move(rule->m_regex_ptr));
        }
        lexer.generate();

        get_intersect_for_query(lexer, "*1*");
        get_intersect_for_query(lexer, "*a*");
        get_intersect_for_query(lexer, "*a1*");
        get_intersect_for_query(lexer, "*=*");
        get_intersect_for_query(lexer, "abc123");
        get_intersect_for_query(lexer, "=");
        get_intersect_for_query(lexer, "1");
        get_intersect_for_query(lexer, "a*1");
        get_intersect_for_query(lexer, "a1");
    }

    {
        std::cout << "--LEXER2--" << std::endl;
        log_surgeon::Schema schema;
        schema.add_variable("v1", "1", -1);
        schema.add_variable("v2", "2", -1);
        schema.add_variable("v3", "3", -1);
        schema.add_variable("v4", "abc12", -1);
        schema.add_variable("v5", "23def", -1);
        schema.add_variable("v6", "123", -1);

        ByteLexer lexer;
        for (unique_ptr<ParserAST> const& parser_ast : schema.get_schema_ast_ptr()->m_schema_vars) {
            auto* rule = dynamic_cast<SchemaVarAST*>(parser_ast.get());
            std::string& name = rule->m_name;
            if (lexer.m_symbol_id.find(name) == lexer.m_symbol_id.end()) {
                lexer.m_symbol_id[name] = lexer.m_symbol_id.size();
                lexer.m_id_symbol[lexer.m_symbol_id[name]] = name;
            }
            lexer.add_rule(lexer.m_symbol_id[name], std::move(rule->m_regex_ptr));
        }
        lexer.generate();

        get_intersect_for_query(lexer, "*1*");
        get_intersect_for_query(lexer, "*a*");
        get_intersect_for_query(lexer, "*a1*");
        get_intersect_for_query(lexer, "*=*");
        get_intersect_for_query(lexer, "abc12");
        get_intersect_for_query(lexer, "=");
        get_intersect_for_query(lexer, "1");
        get_intersect_for_query(lexer, "a*1");
        get_intersect_for_query(lexer, "a1");
    }
    return 0;
}
