#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_HPP

#include <cstdint>
#include <memory>
#include <set>
#include <vector>

#include <log_surgeon/finite_automata/DfaStatePair.hpp>

namespace log_surgeon::finite_automata {
template <typename DFAStateType>
class Dfa {
public:
    /**
     * Creates a new DFA state based on a set of NFA states and adds it to
     * m_states
     * @param nfa_state_set
     * @return DFAStateType*
     */
    template <typename NFAStateType>
    auto new_state(std::set<NFAStateType*> const& nfa_state_set) -> DFAStateType*;

    auto get_root() const -> DFAStateType const* { return m_states.at(0).get(); }

    /**
     * Compares this dfa with dfa_in to determine the set of schema types in
     * this dfa that are reachable by any type in dfa_in. A type is considered
     * reachable if there is at least one string for which: (1) this dfa returns
     * a set of types containing the type, and (2) dfa_in returns any non-empty
     * set of types.
     * @param dfa_in
     * @return The set of schema types reachable by dfa_in
     */
    [[nodiscard]] auto get_intersect(std::unique_ptr<Dfa> const& dfa_in
    ) const -> std::set<uint32_t>;

private:
    std::vector<std::unique_ptr<DFAStateType>> m_states;
};

template <typename DFAStateType>
template <typename NFAStateType>
auto Dfa<DFAStateType>::new_state(std::set<NFAStateType*> const& nfa_state_set
) -> DFAStateType* {
    m_states.emplace_back(std::make_unique<DFAStateType>());
    auto* dfa_state = m_states.back().get();
    for (auto const* nfa_state : nfa_state_set) {
        if (nfa_state->is_accepting()) {
            dfa_state->add_matching_variable_id(nfa_state->get_matching_variable_id());
        }
    }
    return dfa_state;
}

template <typename DFAStateType>
auto Dfa<DFAStateType>::get_intersect(std::unique_ptr<Dfa> const& dfa_in
) const -> std::set<uint32_t> {
    std::set<uint32_t> schema_types;
    std::set<DfaStatePair<DFAStateType>> unvisited_pairs;
    std::set<DfaStatePair<DFAStateType>> visited_pairs;
    unvisited_pairs.emplace(this->get_root(), dfa_in->get_root());
    // TODO: Handle UTF-8 (multi-byte transitions) as well
    while (false == unvisited_pairs.empty()) {
        auto current_pair_it = unvisited_pairs.begin();
        if (current_pair_it->is_accepting()) {
            auto const& matching_variable_ids = current_pair_it->get_matching_variable_ids();
            schema_types.insert(matching_variable_ids.cbegin(), matching_variable_ids.cend());
        }
        visited_pairs.insert(*current_pair_it);
        current_pair_it->get_reachable_pairs(visited_pairs, unvisited_pairs);
        unvisited_pairs.erase(current_pair_it);
    }
    return schema_types;
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_HPP
