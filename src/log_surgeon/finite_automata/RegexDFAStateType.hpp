#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_STATE_TYPE
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_STATE_TYPE

#include <cstdint>

namespace log_surgeon::finite_automata {
enum class RegexDFAStateType  : uint8_t {
    Byte,
    UTF8
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_STATE_TYPE
