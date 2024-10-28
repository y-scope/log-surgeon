#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_STATE_TYPE
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_STATE_TYPE

#include <cstdint>

namespace log_surgeon::finite_automata {
enum class RegexNFAStateType : uint8_t {
    Byte,
    UTF8
};

template <RegexNFAStateType state_type>
class RegexNFAState;

using RegexNFAByteState = RegexNFAState<RegexNFAStateType::Byte>;
using RegexNFAUTF8State = RegexNFAState<RegexNFAStateType::UTF8>;
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_STATE_TYPE
