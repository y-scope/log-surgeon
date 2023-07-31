#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_HPP

#include <algorithm>
#include <cstdint>
#include <memory>
#include <set>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
enum class RegexDFAStateType {
    Byte,
    UTF8
};

template <RegexDFAStateType stateType>
class RegexDFAState {
public:
    using Tree = UnicodeIntervalTree<RegexDFAState<stateType>*>;

    auto add_tag(int const& rule_name_id) -> void { m_tags.push_back(rule_name_id); }

    [[nodiscard]] auto get_tags() const -> std::vector<int> const& { return m_tags; }

    auto is_accepting() -> bool { return !m_tags.empty(); }

    auto add_byte_transition(uint8_t const& byte, RegexDFAState<stateType>* dest_state) -> void {
        m_bytes_transition[byte] = dest_state;
    }

    /**
     * Returns the next state the DFA transitions to on input character (byte or
     * utf8)
     * @param character
     * @return RegexDFAState<stateType>*
     */
    auto next(uint32_t character) -> RegexDFAState<stateType>*;

private:
    std::vector<int> m_tags;
    RegexDFAState<stateType>* m_bytes_transition[cSizeOfByte];
    // NOTE: We don't need m_tree_transitions for the `stateType ==
    // RegexDFAStateType::Byte` case, so we use an empty class (`std::tuple<>`)
    // in that case.
    std::conditional_t<stateType == RegexDFAStateType::UTF8, Tree, std::tuple<>> m_tree_transitions;
};

using RegexDFAByteState = RegexDFAState<RegexDFAStateType::Byte>;
using RegexDFAUTF8State = RegexDFAState<RegexDFAStateType::UTF8>;

template <typename DFAStateType>
class RegexDFA {
public:
    /**
     * Creates a new DFA state based on a set of NFA states and adds it to
     * m_states
     * @param set
     * @return DFAStateType*
     */
    template <typename NFAStateType>
    auto new_state(std::set<NFAStateType*> const& set) -> DFAStateType*;

    auto get_root() -> DFAStateType* { return m_states.at(0).get(); }

private:
    std::vector<std::unique_ptr<DFAStateType>> m_states;
};
}  // namespace log_surgeon::finite_automata

#include "RegexDFA.tpp"

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_HPP
