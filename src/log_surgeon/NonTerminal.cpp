#include <cstdint>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/NonTerminal.hpp>
#include <log_surgeon/Production.hpp>

namespace log_surgeon {
MatchedSymbol NonTerminal::m_all_children[cSizeOfAllChildren];
}  // namespace log_surgeon
