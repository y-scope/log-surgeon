#ifndef LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE
#define LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE

#include <cassert>
#include <cstdint>
#include <memory>
#include <tuple>
#include <vector>

#include <log_surgeon/finite_automata/StateType.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
template <StateType state_type>
class DfaState;

using DfaByteState = DfaState<StateType::Byte>;
using DfaUtf8State = DfaState<StateType::Utf8>;

template <StateType state_type>
class DfaState {
public:
    using Tree = UnicodeIntervalTree<DfaState*>;

    auto add_matching_variable_id(uint32_t const variable_id) -> void {
        m_matching_variable_ids.push_back(variable_id);
    }

    [[nodiscard]] auto get_matching_variable_ids() const -> std::vector<uint32_t> const& {
        return m_matching_variable_ids;
    }

    [[nodiscard]] auto is_accepting() const -> bool { return !m_matching_variable_ids.empty(); }

    auto add_byte_transition(uint8_t const& byte, DfaState* dest_state) -> void {
        m_bytes_transition[byte] = dest_state;
    }

    /**
     * Returns the next state the DFA transitions to on input character (byte or utf8).
     * @param character
     * @return DfaState<stateType>*
     */
    [[nodiscard]] auto next(uint32_t character) const -> DfaState*;

private:
    std::vector<uint32_t> m_matching_variable_ids;
    DfaState* m_bytes_transition[cSizeOfByte];
    // NOTE: We don't need m_tree_transitions for the `stateType == DfaStateType::Byte` case,
    // so we use an empty class (`std::tuple<>`) in that case.
    std::conditional_t<state_type == StateType::Utf8, Tree, std::tuple<>> m_tree_transitions;
};

template <StateType state_type>
auto DfaState<state_type>::next(uint32_t character) const -> DfaState* {
    if constexpr (StateType::Byte == state_type) {
        return m_bytes_transition[character];
    } else {
        if (character < cSizeOfByte) {
            return m_bytes_transition[character];
        }
        std::unique_ptr<std::vector<typename Tree::Data>> result
                = m_tree_transitions.find(Interval(character, character));
        assert(result->size() <= 1);
        if (!result->empty()) {
            return result->front().m_value;
        }
        return nullptr;
    }
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_DFA_STATE
