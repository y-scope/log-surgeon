#include <cstdint>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/NonTerminal.hpp>
#include <log_surgeon/Production.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon {
MatchedSymbol NonTerminal::m_all_children[cSizeOfAllChildren];

uint32_t NonTerminal::m_next_children_start = 0;

NonTerminal::NonTerminal(Production* p)
        : m_children_start(m_next_children_start),
          m_production(p),
          m_ast(nullptr) {
    m_next_children_start += p->m_body.size();
}
}  // namespace log_surgeon
