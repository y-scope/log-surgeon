#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_STATE
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_STATE

#include <type_traits>
#include <cassert>
#include <cstdint>
#include <memory>
#include <tuple>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexDFAStateType.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
template <RegexDFAStateType state_type>
class RegexDFAState;

using RegexDFAByteState = RegexDFAState<RegexDFAStateType::Byte>;
using RegexDFAUTF8State = RegexDFAState<RegexDFAStateType::UTF8>;

template <RegexDFAStateType stateType>
class RegexDFAState {
public:
    using Tree = UnicodeIntervalTree<RegexDFAState<stateType>*>;

    RegexDFAState() {
        std::fill(std::begin(m_bytes_transition), std::end(m_bytes_transition), nullptr);
    }

    auto add_matching_variable_id(uint32_t const variable_id) -> void {
        m_matching_variable_ids.push_back(variable_id);
    }

    [[nodiscard]] auto get_matching_variable_ids() const -> std::vector<uint32_t> const& {
        return m_matching_variable_ids;
    }

    [[nodiscard]] auto is_accepting() const -> bool { return !m_matching_variable_ids.empty(); }

    auto add_byte_transition(uint8_t const& byte, RegexDFAState<stateType>* dest_state) -> void {
        m_bytes_transition[byte] = dest_state;
    }

    /**
     * @param character The character (byte or utf8) to transition on.
     * @return A pointer to the DFA state reached after transitioning on `character`.
     */
    [[nodiscard]] auto next(uint32_t character) const -> RegexDFAState<stateType>*;

private:
    std::vector<uint32_t> m_matching_variable_ids;
    RegexDFAState<stateType>* m_bytes_transition[cSizeOfByte];
    // NOTE: We don't need m_tree_transitions for the `stateType == RegexDFAStateType::Byte` case,
    // so we use an empty class (`std::tuple<>`) in that case.
    std::conditional_t<stateType == RegexDFAStateType::UTF8, Tree, std::tuple<>> m_tree_transitions;
};

template <RegexDFAStateType stateType>
auto RegexDFAState<stateType>::next(uint32_t character) const -> RegexDFAState<stateType>* {
    if constexpr (RegexDFAStateType::Byte == stateType) {
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

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_STATE
