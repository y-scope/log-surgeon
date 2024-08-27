#ifndef LOG_SURGEON_PARSER_TPP
#define LOG_SURGEON_PARSER_TPP

namespace log_surgeon {

template <typename NFAStateType, typename DFAStateType>
Parser<NFAStateType, DFAStateType>::Parser() {
    // TODO move clp-reserved symbols out of the parser
    m_lexer.m_symbol_id[cTokenEnd] = static_cast<int>(SymbolID::TokenEndID);
    m_lexer.m_symbol_id[cTokenUncaughtString] = static_cast<int>(SymbolID::TokenUncaughtStringID);
    m_lexer.m_symbol_id[cTokenInt] = static_cast<int>(SymbolID::TokenIntId);
    m_lexer.m_symbol_id[cTokenFloat] = static_cast<int>(SymbolID::TokenFloatId);
    m_lexer.m_symbol_id[cTokenHex] = static_cast<int>(SymbolID::TokenHexId);
    m_lexer.m_symbol_id[cTokenFirstTimestamp] = static_cast<int>(SymbolID::TokenFirstTimestampId);
    m_lexer.m_symbol_id[cTokenNewlineTimestamp]
            = static_cast<int>(SymbolID::TokenNewlineTimestampId);
    m_lexer.m_symbol_id[cTokenNewline] = static_cast<int>(SymbolID::TokenNewlineId);

    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenEndID)] = cTokenEnd;
    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenUncaughtStringID)] = cTokenUncaughtString;
    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenIntId)] = cTokenInt;
    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenFloatId)] = cTokenFloat;
    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenHexId)] = cTokenHex;
    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenFirstTimestampId)] = cTokenFirstTimestamp;
    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenNewlineTimestampId)]
            = cTokenNewlineTimestamp;
    m_lexer.m_id_symbol[static_cast<int>(SymbolID::TokenNewlineId)] = cTokenNewline;
}

template <typename NFAStateType, typename DFAStateType>
void Parser<NFAStateType, DFAStateType>::add_rule(
        std::string const& name,
        std::unique_ptr<finite_automata::RegexAST<NFAStateType>> rule
) {
    if (m_lexer.m_symbol_id.find(name) == m_lexer.m_symbol_id.end()) {
        m_lexer.m_symbol_id[name] = m_lexer.m_symbol_id.size();
        m_lexer.m_id_symbol[m_lexer.m_symbol_id[name]] = name;
    }
    m_lexer.add_rule(m_lexer.m_symbol_id[name], std::move(rule));
}

template <typename NFAStateType, typename DFAStateType>
void Parser<NFAStateType, DFAStateType>::add_token(std::string const& name, char rule_char) {
    add_rule(name, std::make_unique<finite_automata::RegexASTLiteral<NFAStateType>>(rule_char));
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_PARSER_TPP
