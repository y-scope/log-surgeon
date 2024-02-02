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
auto RegexDFAStatePair<DFAState>::generate_reachable_pairs() -> std::set<RegexDFAStatePair> {
    std::set<RegexDFAStatePair> reachable_pairs;
    // TODO: currently assumes it is a byte input, needs to handle utf8 as well
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        if (m_state1->next(i) != nullptr && m_state2->next(i) != nullptr) {
            reachable_pairs.emplace(m_state1->next(i), m_state2->next(i));
        }
    }
    return reachable_pairs;
}

template <typename DFAState>
auto RegexDFAStatePair<DFAState>::is_accepting() const -> bool {
    return m_state1->is_accepting() && m_state2->is_accepting();
}

template <typename DFAState>
auto RegexDFAStatePair<DFAState>::get_first_tags() const -> std::vector<int> const& {
    return m_state1->get_tags();
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
auto RegexDFA<DFAStateType>::get_intersect(std::unique_ptr<RegexDFA> const& dfa_in)
        -> std::set<uint32_t> {
    std::set<uint32_t> schema_types;
    std::set<RegexDFAStatePair<DFAStateType>> unused_pairs;
    std::set<RegexDFAStatePair<DFAStateType>> used_pairs;
    unused_pairs.emplace(this->get_root(), dfa_in->get_root());
    // TODO: currently assumes it is a byte input, needs to handle utf8 as well
    while (false == unused_pairs.empty()) {
        auto current_pair = *unused_pairs.begin();
        if (current_pair.is_accepting()) {
            std::vector<int> const& tags = current_pair.get_first_tags();
            schema_types.insert(tags.begin(), tags.end());
        }
        unused_pairs.erase(unused_pairs.begin());
        used_pairs.insert(current_pair);
        std::set<RegexDFAStatePair<DFAStateType>> reachable_pairs
                = current_pair.generate_reachable_pairs();
        for (RegexDFAStatePair<DFAStateType> const& reachable_pair : reachable_pairs) {
            if (used_pairs.find(reachable_pair) == used_pairs.end()) {
                unused_pairs.insert(reachable_pair);
            }
        }
    }
    return schema_types;
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP
