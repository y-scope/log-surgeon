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
auto RegexDFAStatePair<DFAState>::get_reachable_pairs() -> std::set<RegexDFAStatePair> {
    std::set<RegexDFAStatePair> reachable_pairs;
    // TODO: Handle UTF-8 (multi-byte transitions) as well
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        auto next_state1 = m_state1->next(i);
        auto next_state2 = m_state2->next(i);
        if (next_state1 != nullptr && next_state2 != nullptr) {
            reachable_pairs.emplace(next_state1, next_state2);
        }
    }
    return reachable_pairs;
}

template <typename DFAStateType>
template <typename NFAStateType>
auto RegexDFA<DFAStateType>::new_state(std::set<NFAStateType*> const& set) -> DFAStateType* {
    std::unique_ptr<DFAStateType> ptr = std::make_unique<DFAStateType>();
    m_states.push_back(std::move(ptr));
    DFAStateType* state = m_states.back().get();
    for (NFAStateType const* s : set) {
        if (s->is_accepting()) {
            state->add_tag(s->get_tag());
        }
    }
    return state;
}

template <typename DFAStateType>
auto RegexDFA<DFAStateType>::get_intersect(std::unique_ptr<RegexDFA> const& dfa_in) const
        -> std::set<uint32_t> {
    std::set<uint32_t> schema_types;
    std::set<RegexDFAStatePair<DFAStateType>> unvisited_pairs;
    std::set<RegexDFAStatePair<DFAStateType>> visited_pairs;
    unvisited_pairs.emplace(this->get_root(), dfa_in->get_root());
    // TODO: Handle UTF-8 (multi-byte transitions) as well
    while (false == unvisited_pairs.empty()) {
        auto current_pair_itt = unvisited_pairs.begin();
        if (current_pair_itt->is_accepting()) {
            auto tags = current_pair_itt->get_first_tags();
            schema_types.insert(tags.begin(), tags.end());
        }
        visited_pairs.insert(*current_pair_itt);
        std::set<RegexDFAStatePair<DFAStateType>> reachable_pairs
                = current_pair_itt->get_reachable_pairs();
        unvisited_pairs.erase(current_pair_itt);
        for (RegexDFAStatePair<DFAStateType> const& reachable_pair : reachable_pairs) {
            if (visited_pairs.find(reachable_pair) == visited_pairs.end()) {
                unvisited_pairs.insert(reachable_pair);
            }
        }
    }
    return schema_types;
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP
