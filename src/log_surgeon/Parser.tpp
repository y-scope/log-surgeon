#ifndef LOG_SURGEON_PARSER_TPP
#define LOG_SURGEON_PARSER_TPP

#include <memory>

#include <log_surgeon/finite_automata/RegexAST.hpp>

namespace log_surgeon {
template <typename TypedNfaState, typename TypedDfaState>
Parser<TypedNfaState, TypedDfaState>::Parser() {
    // TODO move clp-reserved symbols out of the parser
    m_lexer.m_symbol_id[cTokenEnd] = (uint32_t)SymbolId::TokenEnd;
    m_lexer.m_symbol_id[cTokenUncaughtString] = (uint32_t)SymbolId::TokenUncaughtString;
    m_lexer.m_symbol_id[cTokenInt] = (uint32_t)SymbolId::TokenInt;
    m_lexer.m_symbol_id[cTokenFloat] = (uint32_t)SymbolId::TokenFloat;
    m_lexer.m_symbol_id[cTokenHex] = (uint32_t)SymbolId::TokenHex;
    m_lexer.m_symbol_id[cTokenHeader] = (uint32_t)SymbolId::TokenHeader;
    m_lexer.m_symbol_id[cTokenNewline] = (uint32_t)SymbolId::TokenNewline;

    m_lexer.m_id_symbol[(uint32_t)SymbolId::TokenEnd] = cTokenEnd;
    m_lexer.m_id_symbol[(uint32_t)SymbolId::TokenUncaughtString] = cTokenUncaughtString;
    m_lexer.m_id_symbol[(uint32_t)SymbolId::TokenInt] = cTokenInt;
    m_lexer.m_id_symbol[(uint32_t)SymbolId::TokenFloat] = cTokenFloat;
    m_lexer.m_id_symbol[(uint32_t)SymbolId::TokenHex] = cTokenHex;
    m_lexer.m_id_symbol[(uint32_t)SymbolId::TokenHeader] = cTokenHeader;
    m_lexer.m_id_symbol[(uint32_t)SymbolId::TokenNewline] = cTokenNewline;
}

template <typename TypedNfaState, typename TypedDfaState>
auto Parser<TypedNfaState, TypedDfaState>::add_rule(
        std::string const& name,
        std::unique_ptr<finite_automata::RegexAST<TypedNfaState>> rule
) -> void {
    if (m_lexer.m_symbol_id.find(name) == m_lexer.m_symbol_id.end()) {
        m_lexer.m_symbol_id[name] = m_lexer.m_symbol_id.size();
        m_lexer.m_id_symbol[m_lexer.m_symbol_id[name]] = name;
    }
    m_lexer.add_rule(m_lexer.m_symbol_id[name], std::move(rule));
}

template <typename TypedNfaState, typename TypedDfaState>
auto Parser<TypedNfaState, TypedDfaState>::add_token(std::string const& name, char rule_char)
        -> void {
    add_rule(name, std::make_unique<finite_automata::RegexASTLiteral<TypedNfaState>>(rule_char));
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_PARSER_TPP
