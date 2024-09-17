#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP

namespace log_surgeon::finite_automata {
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

template <typename DFAState>
auto RegexDFAStatePair<DFAState>::get_reachable_pairs(
        std::set<RegexDFAStatePair<DFAState>>& visited_pairs,
        std::set<RegexDFAStatePair<DFAState>>& unvisited_pairs
) const -> void {
    // TODO: Handle UTF-8 (multi-byte transitions) as well
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        auto next_state1 = m_state1->next(i);
        auto next_state2 = m_state2->next(i);
        if (next_state1 != nullptr && next_state2 != nullptr) {
            RegexDFAStatePair<DFAState> reachable_pair{next_state1, next_state2};
            if (visited_pairs.count(reachable_pair) == 0) {
                unvisited_pairs.insert(reachable_pair);
            }
        }
    }
}

template <typename DFAStateType>
template <typename NFAStateType>
auto RegexDFA<DFAStateType>::new_state(std::set<NFAStateType*> const& nfa_state_set
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
auto RegexDFA<DFAStateType>::get_intersect(std::unique_ptr<RegexDFA> const& dfa_in
) const -> std::set<uint32_t> {
    std::set<uint32_t> schema_types;
    std::set<RegexDFAStatePair<DFAStateType>> unvisited_pairs;
    std::set<RegexDFAStatePair<DFAStateType>> visited_pairs;
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

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP
