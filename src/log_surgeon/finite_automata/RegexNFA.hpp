#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP

#include <algorithm>
#include <cstdint>
#include <memory>
#include <set>
#include <stack>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {
enum class RegexNFAStateType {
    Byte,
    UTF8
};

template <RegexNFAStateType stateType>
class RegexNFAState {
public:
    using Tree = UnicodeIntervalTree<RegexNFAState<stateType>*>;

    auto set_accepting(bool accepting) -> void { m_accepting = accepting; }

    [[nodiscard]] auto is_accepting() const -> bool const& { return m_accepting; }

    auto set_tag(int rule_name_id) -> void { m_tag = rule_name_id; }

    [[nodiscard]] auto get_tag() const -> int const& { return m_tag; }

    auto set_epsilon_transitions(std::vector<RegexNFAState<stateType>*>& epsilon_transitions)
            -> void {
        m_epsilon_transitions = epsilon_transitions;
    }

    auto add_epsilon_transition(RegexNFAState<stateType>* epsilon_transition) -> void {
        m_epsilon_transitions.push_back(epsilon_transition);
    }

    auto clear_epsilon_transitions() -> void { m_epsilon_transitions.clear(); }

    [[nodiscard]] auto get_epsilon_transitions() const
            -> std::vector<RegexNFAState<stateType>*> const& {
        return m_epsilon_transitions;
    }

    auto
    set_byte_transitions(uint8_t byte, std::vector<RegexNFAState<stateType>*>& byte_transitions)
            -> void {
        m_bytes_transitions[byte] = byte_transitions;
    }

    auto add_byte_transition(uint8_t byte, RegexNFAState<stateType>* dest_state) -> void {
        m_bytes_transitions[byte].push_back(dest_state);
    }

    auto clear_byte_transitions(uint8_t byte) -> void { m_bytes_transitions[byte].clear(); }

    [[nodiscard]] auto get_byte_transitions(uint8_t byte) const
            -> std::vector<RegexNFAState<stateType>*> const& {
        return m_bytes_transitions[byte];
    }

    auto reset_tree_transitions() -> void { m_tree_transitions.reset(); }

    auto get_tree_transitions() -> Tree const& { return m_tree_transitions; }

    /**
     Add dest_state to m_bytes_transitions if all values in interval are a byte, otherwise add
     dest_state to m_tree_transitions
     * @param interval
     * @param dest_state
     */
    auto add_interval(Interval interval, RegexNFAState<stateType>* dest_state) -> void;

private:
    bool m_accepting;
    int m_tag;
    std::vector<RegexNFAState<stateType>*> m_epsilon_transitions;
    std::vector<RegexNFAState<stateType>*> m_bytes_transitions[cSizeOfByte];
    // NOTE: We don't need m_tree_transitions for the `stateType ==
    // RegexDFAStateType::Byte` case, so we use an empty class (`std::tuple<>`)
    // in that case.
    std::conditional_t<stateType == RegexNFAStateType::UTF8, Tree, std::tuple<>> m_tree_transitions;
};

using RegexNFAByteState = RegexNFAState<RegexNFAStateType::Byte>;
using RegexNFAUTF8State = RegexNFAState<RegexNFAStateType::UTF8>;

template <typename NFAStateType>
class RegexNFA {
public:
    using StateVec = std::vector<NFAStateType*>;

    RegexNFA() : m_root{new_state()} {}

    /**
     * Create a unique_ptr for an NFA state and add it to m_states
     * @return NFAStateType*
     */
    auto new_state() -> NFAStateType*;

    /**
     * Reverse the NFA such that it matches on its reverse language
     */
    auto reverse() -> void;

    auto add_root_interval(Interval interval, NFAStateType* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(NFAStateType* root) -> void { m_root = root; }

    auto get_root() -> NFAStateType* { return m_root; }

private:
    std::vector<std::unique_ptr<NFAStateType>> m_states;
    NFAStateType* m_root;
};
}  // namespace log_surgeon::finite_automata

#include "RegexNFA.tpp"

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
