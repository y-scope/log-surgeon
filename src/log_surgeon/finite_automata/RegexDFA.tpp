#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP

namespace log_surgeon::finite_automata {

template <RegexDFAStateType stateType>
auto RegexDFAState<stateType>::next(uint32_t character) -> RegexDFAState<stateType>* {
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
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_DFA_TPP
