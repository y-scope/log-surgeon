#include "Lalr1Parser.hpp"

namespace log_surgeon {
MatchedSymbol NonTerminal::m_all_children[cSizeOfAllChildren];

ParserAST::~ParserAST() = default;

uint32_t NonTerminal::m_next_children_start = 0;

NonTerminal::NonTerminal(Production* p)
        : m_children_start(m_next_children_start),
          m_production(p),
          m_ast(nullptr) {
    m_next_children_start += p->m_body.size();
}
}  // namespace log_surgeon
