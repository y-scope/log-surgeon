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

    [[nodiscard]] auto is_accepting() const -> bool { return !m_tags.empty(); }

    auto add_byte_transition(uint8_t const& byte, RegexDFAState<stateType>* dest_state) -> void {
        m_bytes_transition[byte] = dest_state;
    }

    /**
     * Returns the next state the DFA transitions to on input character (byte or
     * utf8)
     * @param character
     * @return RegexDFAState<stateType>*
     */
    [[nodiscard]] auto next(uint32_t character) const -> RegexDFAState<stateType>*;

private:
    std::vector<int> m_tags;
    RegexDFAState<stateType>* m_bytes_transition[cSizeOfByte];
    // NOTE: We don't need m_tree_transitions for the `stateType ==
    // RegexDFAStateType::Byte` case, so we use an empty class (`std::tuple<>`)
    // in that case.
    std::conditional_t<stateType == RegexDFAStateType::UTF8, Tree, std::tuple<>> m_tree_transitions;
};

template <typename DFAState>
class RegexDFAStatePair {
public:
    RegexDFAStatePair(DFAState const* state1, DFAState const* state2)
            : m_state1(state1),
              m_state2(state2){};

    /**
     * Used for ordering in a set by considering the states' addresses
     * @param rhs
     * @return true if m_state1 in lhs has a lower address than in rhs, if tied,
     * true if m_state2 in lhs has a lower address than in rhs, false otherwise
     */
    auto operator<(RegexDFAStatePair const& rhs) const -> bool {
        if (m_state1 == rhs.m_state1) {
            return m_state2 < rhs.m_state2;
        }
        return m_state1 < rhs.m_state1;
    }

    /**
     * Generates all pairs reachable from the current pair via any string and
     * store any reachable pair not previously visited in unvisited_pairs
     */
    auto get_reachable_pairs(
            std::set<RegexDFAStatePair<DFAState>>& visited_pairs,
            std::set<RegexDFAStatePair<DFAState>>& unvisited_pairs
    ) const -> void;

    /**
     * @return Whether both states are accepting
     */
    [[nodiscard]] auto is_accepting() const -> bool {
        return m_state1->is_accepting() && m_state2->is_accepting();
    }

    /**
     * @return The tags of the first state of the pair
     */
    [[nodiscard]] auto get_first_tags() const -> std::vector<int> const& {
        return m_state1->get_tags();
    }

private:
    DFAState const* m_state1;
    DFAState const* m_state2;
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

    auto get_root() const -> DFAStateType const* { return m_states.at(0).get(); }

    /**
     * Compares this dfa with dfa_in to determine the set of schema types in
     * this dfa that are reachable by any type in dfa_in. A type is considered
     * reachable if there is at least one string for which: (1) this dfa returns
     * a set of types containing the type, and (2) dfa_in returns any non-empty
     * set of types.
     * @param dfa_in
     * @return set of schema types reachable by dfa_in
     */
    [[nodiscard]] auto get_intersect(std::unique_ptr<RegexDFA> const& dfa_in) const
            -> std::set<uint32_t>;

private:
    std::vector<std::unique_ptr<DFAStateType>> m_states;
};
}  // namespace log_surgeon::finite_automata

#include "RegexDFA.tpp"

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_HPP
