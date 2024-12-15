#ifndef LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP

#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/finite_automata/Tag.hpp>

namespace log_surgeon::finite_automata {

template <typename TypedNfaState>
struct DetermizationConfiguration {
    TypedNfaState* m_nfa_state;
    std::vector<uint32_t> m_register_ids;
    std::vector<Tag*> m_history;
    std::vector<Tag*> m_sequence;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DETERMINIZATION_CONFIGURATION_HPP
