#ifndef LOG_SURGEON_FINITE_AUTOMATA_STATE_TYPE
#define LOG_SURGEON_FINITE_AUTOMATA_STATE_TYPE

#include <cstdint>

namespace log_surgeon::finite_automata {
enum class StateType  : uint8_t {
    Byte,
    Utf8
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_STATE_TYPE
