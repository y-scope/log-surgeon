#ifndef LOG_SURGEON_LEXER_TPP
#define LOG_SURGEON_LEXER_TPP

#include <cassert>
#include <string>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>

/**
 * utf8 format (https://en.wikipedia.org/wiki/UTF-8)
 * 1 byte: 0x0 - 0x80 : 0xxxxxxx
 * 2 byte: 0x80 - 0x7FF : 110xxxxx 10xxxxxx
 * 3 byte: 0x800 - 0xFFFF : 1110xxxx 10xxxxxx 10xxxxxx
 * 4 byte: 0x10000 - 0x1FFFFF : 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
 */
namespace log_surgeon {
template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::flip_states(uint32_t old_storage_size) {
    if (m_match_pos >= old_storage_size / 2) {
        m_match_pos -= old_storage_size / 2;
    } else {
        m_match_pos += old_storage_size / 2;
    }
    // TODO when m_start_pos == old_storage_size / 2, theres two possible cases
    // currently so both options are potentially wrong
    if (m_start_pos > old_storage_size / 2) {
        m_start_pos -= old_storage_size / 2;
    } else {
        m_start_pos += old_storage_size / 2;
    }
    if (m_last_match_pos >= old_storage_size / 2) {
        m_last_match_pos -= old_storage_size / 2;
    } else {
        m_last_match_pos += old_storage_size / 2;
    }
}

template <typename NFAStateType, typename DFAStateType>
auto Lexer<NFAStateType, DFAStateType>::scan(ParserInputBuffer& input_buffer, Token& token)
        -> ErrorCode {
    DFAStateType* state = m_dfa->get_root();
    if (m_asked_for_more_data) {
        state = m_prev_state;
        m_asked_for_more_data = false;
    } else {
        if (m_match) {
            m_match = false;
            m_last_match_pos = m_match_pos;
            m_last_match_line = m_match_line;
            token = Token{
                    m_start_pos,
                    m_match_pos,
                    input_buffer.storage().get_active_buffer(),
                    input_buffer.storage().size(),
                    m_match_line,
                    m_type_ids};
            return ErrorCode::Success;
        }
        m_start_pos = input_buffer.storage().pos();
        m_match_pos = input_buffer.storage().pos();
        m_match_line = m_line;
        m_type_ids = nullptr;
    }
    while (true) {
        uint32_t prev_byte_buf_pos = input_buffer.storage().pos();
        unsigned char next_char{utf8::cCharErr};
        if (ErrorCode err = input_buffer.get_next_character(next_char); ErrorCode::Success != err) {
            m_asked_for_more_data = true;
            m_prev_state = state;
            return err;
        }
        if ((m_is_delimiter[next_char] || input_buffer.log_fully_consumed() || !m_has_delimiters)
            && state->is_accepting())
        {
            m_match = true;
            m_type_ids = &(state->get_tags());
            m_match_pos = prev_byte_buf_pos;
            m_match_line = m_line;
        }
        DFAStateType* next = state->next(next_char);
        if (next_char == '\n') {
            m_line++;
            if (m_has_delimiters && !m_match) {
                next = m_dfa->get_root()->next(next_char);
                m_match = true;
                m_type_ids = &(next->get_tags());
                m_start_pos = prev_byte_buf_pos;
                m_match_pos = input_buffer.storage().pos();
                m_match_line = m_line;
            }
        }
        if (input_buffer.log_fully_consumed() || next == nullptr) {
            if (m_match) {
                input_buffer.set_log_fully_consumed(false);
                input_buffer.set_pos(m_match_pos);
                m_line = m_match_line;
                if (m_last_match_pos != m_start_pos) {
                    token = Token{
                            m_last_match_pos,
                            m_start_pos,
                            input_buffer.storage().get_active_buffer(),
                            input_buffer.storage().size(),
                            m_last_match_line,
                            &cTokenUncaughtStringTypes};
                    return ErrorCode::Success;
                }
                m_match = false;
                m_last_match_pos = m_match_pos;
                m_last_match_line = m_match_line;
                token = Token{
                        m_start_pos,
                        m_match_pos,
                        input_buffer.storage().get_active_buffer(),
                        input_buffer.storage().size(),
                        m_match_line,
                        m_type_ids};
                return ErrorCode::Success;
            }
            if (input_buffer.log_fully_consumed() && m_start_pos == input_buffer.storage().pos()) {
                if (m_last_match_pos != m_start_pos) {
                    m_match_pos = input_buffer.storage().pos();
                    m_type_ids = &cTokenEndTypes;
                    m_match = true;
                    token = Token{
                            m_last_match_pos,
                            m_start_pos,
                            input_buffer.storage().get_active_buffer(),
                            input_buffer.storage().size(),
                            m_last_match_line,
                            &cTokenUncaughtStringTypes};
                    return ErrorCode::Success;
                }
                token = Token{
                        input_buffer.storage().pos(),
                        input_buffer.storage().pos(),
                        input_buffer.storage().get_active_buffer(),
                        input_buffer.storage().size(),
                        m_line,
                        &cTokenEndTypes};
                return ErrorCode::Success;
            }
            // TODO: remove timestamp from m_is_fist_char so that m_is_delimiter
            // check not needed
            while (input_buffer.log_fully_consumed() == false
                   && (m_is_first_char[next_char] == false || m_is_delimiter[next_char] == false))
            {
                prev_byte_buf_pos = input_buffer.storage().pos();
                if (ErrorCode err = input_buffer.get_next_character(next_char);
                    ErrorCode::Success != err)
                {
                    m_asked_for_more_data = true;
                    m_prev_state = state;
                    return err;
                }
            }
            input_buffer.set_pos(prev_byte_buf_pos);
            m_start_pos = prev_byte_buf_pos;
            state = m_dfa->get_root();
            continue;
        }
        state = next;
    }
}

// TODO: this is duplicating almost all the code of scan()
template <typename NFAStateType, typename DFAStateType>
auto Lexer<NFAStateType, DFAStateType>::scan_with_wildcard(
        ParserInputBuffer& input_buffer,
        char wildcard,
        Token& token
) -> ErrorCode {
    DFAStateType* state = m_dfa->get_root();
    if (m_asked_for_more_data) {
        state = m_prev_state;
        m_asked_for_more_data = false;
    } else {
        if (m_match) {
            m_match = false;
            m_last_match_pos = m_match_pos;
            m_last_match_line = m_match_line;
            token = Token{
                    m_start_pos,
                    m_match_pos,
                    input_buffer.storage().get_active_buffer(),
                    input_buffer.storage().size(),
                    m_match_line,
                    m_type_ids};
            return ErrorCode::Success;
        }
        m_start_pos = input_buffer.storage().pos();
        m_match_pos = input_buffer.storage().pos();
        m_match_line = m_line;
        m_type_ids = nullptr;
    }
    while (true) {
        uint32_t prev_byte_buf_pos = input_buffer.storage().pos();
        unsigned char next_char{utf8::cCharErr};
        if (ErrorCode err = input_buffer.get_next_character(next_char); ErrorCode::Success != err) {
            m_asked_for_more_data = true;
            m_prev_state = state;
            return err;
        }
        if ((m_is_delimiter[next_char] || input_buffer.log_fully_consumed() || !m_has_delimiters)
            && state->is_accepting())
        {
            m_match = true;
            m_type_ids = &(state->get_tags());
            m_match_pos = prev_byte_buf_pos;
            m_match_line = m_line;
        }
        DFAStateType* next = state->next(next_char);
        if (next_char == '\n') {
            m_line++;
            if (m_has_delimiters && !m_match) {
                next = m_dfa->get_root()->next(next_char);
                m_match = true;
                m_type_ids = &(next->get_tags());
                m_start_pos = prev_byte_buf_pos;
                m_match_pos = input_buffer.storage().pos();
                m_match_line = m_line;
            }
        }
        if (input_buffer.log_fully_consumed() || next == nullptr) {
            assert(input_buffer.log_fully_consumed());
            if (!m_match || (m_match && m_match_pos != input_buffer.storage().pos())) {
                token = Token{
                        m_last_match_pos,
                        input_buffer.storage().pos(),
                        input_buffer.storage().get_active_buffer(),
                        input_buffer.storage().size(),
                        m_last_match_line,
                        &cTokenUncaughtStringTypes};
                return ErrorCode::Success;
            }
            if (m_match) {
                // BFS (keep track of m_type_ids)
                if (wildcard == '?') {
                    for (uint32_t byte = 0; byte < cSizeOfByte; byte++) {
                        DFAStateType* next_state = state->next(byte);
                        if (next_state->is_accepting() == false) {
                            token = Token{
                                    m_last_match_pos,
                                    input_buffer.storage().pos(),
                                    input_buffer.storage().get_active_buffer(),
                                    input_buffer.storage().size(),
                                    m_last_match_line,
                                    &cTokenUncaughtStringTypes};
                            return ErrorCode::Success;
                        }
                    }
                } else if (wildcard == '*') {
                    std::stack<DFAStateType*> unvisited_states;
                    std::set<DFAStateType*> visited_states;
                    unvisited_states.push(state);
                    while (!unvisited_states.empty()) {
                        DFAStateType* current_state = unvisited_states.top();
                        if (current_state == nullptr || current_state->is_accepting() == false) {
                            token = Token{
                                    m_last_match_pos,
                                    input_buffer.storage().pos(),
                                    input_buffer.storage().get_active_buffer(),
                                    input_buffer.storage().size(),
                                    m_last_match_line,
                                    &cTokenUncaughtStringTypes};
                            return ErrorCode::Success;
                        }
                        unvisited_states.pop();
                        visited_states.insert(current_state);
                        for (uint32_t byte = 0; byte < cSizeOfByte; byte++) {
                            if (m_is_delimiter[byte]) {
                                continue;
                            }
                            DFAStateType* next_state = current_state->next(byte);
                            if (visited_states.find(next_state) == visited_states.end()) {
                                unvisited_states.push(next_state);
                            }
                        }
                    }
                }
                input_buffer.set_pos(m_match_pos);
                m_line = m_match_line;
                m_match = false;
                m_last_match_pos = m_match_pos;
                m_last_match_line = m_match_line;
                token = Token{
                        m_start_pos,
                        m_match_pos,
                        input_buffer.storage().get_active_buffer(),
                        input_buffer.storage().size(),
                        m_match_line,
                        m_type_ids};
                return ErrorCode::Success;
            }
        }
        state = next;
    }
}

template <typename NFAStateType, typename DFAStateType>
auto Lexer<NFAStateType, DFAStateType>::increase_buffer_capacity(ParserInputBuffer& input_buffer)
        -> void {
    uint32_t old_storage_size{0};
    bool flipped_static_buffer{false};
    input_buffer.increase_capacity(old_storage_size, flipped_static_buffer);
    if (old_storage_size < input_buffer.storage().size()) {
        if (flipped_static_buffer) {
            flip_states(old_storage_size);
        }
        if (0 == m_last_match_pos) {
            m_last_match_pos = old_storage_size;
            m_start_pos = old_storage_size;
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::reset() {
    m_last_match_pos = 0;
    m_match = false;
    m_line = 0;
    m_match_pos = 0;
    m_start_pos = 0;
    m_match_line = 0;
    m_last_match_line = 0;
    m_type_ids = nullptr;
    m_asked_for_more_data = false;
    m_prev_state = nullptr;
}

template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::prepend_start_of_file_char(ParserInputBuffer& input_buffer
) {
    m_prev_state = m_dfa->get_root()->next(utf8::cCharStartOfFile);
    m_asked_for_more_data = true;
    m_start_pos = input_buffer.storage().pos();
    m_match_pos = input_buffer.storage().pos();
    m_match_line = m_line;
    m_type_ids = nullptr;
}

template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::add_delimiters(std::vector<uint32_t> const& delimiters) {
    assert(!delimiters.empty());
    m_has_delimiters = true;
    for (bool& i : m_is_delimiter) {
        i = false;
    }
    for (uint32_t delimiter : delimiters) {
        m_is_delimiter[delimiter] = true;
    }
    m_is_delimiter[utf8::cCharStartOfFile] = true;
}

template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::add_rule(
        uint32_t const& id,
        std::unique_ptr<finite_automata::RegexAST<NFAStateType>> rule
) {
    m_rules.emplace_back(id, std::move(rule));
}

template <typename NFAStateType, typename DFAStateType>
auto Lexer<NFAStateType, DFAStateType>::get_rule(uint32_t const& name)
        -> finite_automata::RegexAST<NFAStateType>* {
    for (Rule& rule : m_rules) {
        if (rule.m_name == name) {
            return rule.m_regex.get();
        }
    }
    return nullptr;
}

template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::generate() {
    finite_automata::RegexNFA<NFAStateType> nfa;
    for (Rule const& r : m_rules) {
        r.add_ast(&nfa);
    }
    m_dfa = nfa_to_dfa(nfa);
    DFAStateType* state = m_dfa->get_root();
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        if (state->next(i) != nullptr) {
            m_is_first_char[i] = true;
        } else {
            m_is_first_char[i] = false;
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::generate_reverse() {
    finite_automata::RegexNFA<NFAStateType> nfa;
    for (Rule const& r : m_rules) {
        r.add_ast(&nfa);
    }
    nfa.reverse();
    m_dfa = nfa_to_dfa(nfa);
    DFAStateType* state = m_dfa->get_root();
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        if (state->next(i) != nullptr) {
            m_is_first_char[i] = true;
        } else {
            m_is_first_char[i] = false;
        }
    }
}

template <typename NFAStateType, typename DFAStateType>
void Lexer<NFAStateType, DFAStateType>::Rule::add_ast(finite_automata::RegexNFA<NFAStateType>* nfa
) const {
    NFAStateType* s = nfa->new_state();
    s->set_accepting(true);
    s->set_tag(m_name);
    m_regex->add(nfa, s);
}

template <typename NFAStateType, typename DFAStateType>
auto Lexer<NFAStateType, DFAStateType>::epsilon_closure(NFAStateType const* state_ptr)
        -> std::set<NFAStateType const*> {
    std::set<NFAStateType const*> closure_set;
    std::stack<NFAStateType const*> stack;
    stack.push(state_ptr);
    while (!stack.empty()) {
        NFAStateType const* t = stack.top();
        stack.pop();
        if (closure_set.insert(t).second) {
            for (NFAStateType* const u : t->get_epsilon_transitions()) {
                stack.push(u);
            }
        }
    }
    return closure_set;
}

template <typename NFAStateType, typename DFAStateType>
auto Lexer<NFAStateType, DFAStateType>::nfa_to_dfa(finite_automata::RegexNFA<NFAStateType>& nfa)
        -> std::unique_ptr<finite_automata::RegexDFA<DFAStateType>> {
    typedef std::set<NFAStateType const*> StateSet;
    std::unique_ptr<finite_automata::RegexDFA<DFAStateType>> dfa
            = std::make_unique<finite_automata::RegexDFA<DFAStateType>>();
    std::map<StateSet, DFAStateType*> dfa_states;
    std::stack<StateSet> unmarked_sets;
    auto create_dfa_state = [&dfa, &dfa_states, &unmarked_sets](StateSet const& set
                            ) -> DFAStateType* {
        DFAStateType* state = dfa->new_state(set);
        dfa_states[set] = state;
        unmarked_sets.push(set);
        return state;
    };
    StateSet start_set = epsilon_closure(nfa.get_root());
    create_dfa_state(start_set);
    while (!unmarked_sets.empty()) {
        StateSet set = unmarked_sets.top();
        unmarked_sets.pop();
        DFAStateType* dfa_state = dfa_states.at(set);
        std::map<uint32_t, StateSet> ascii_transitions_map;
        // map<Interval, StateSet> transitions_map;
        for (NFAStateType const* s0 : set) {
            for (uint32_t i = 0; i < cSizeOfByte; i++) {
                for (NFAStateType* const s1 : s0->get_byte_transitions(i)) {
                    StateSet closure = epsilon_closure(s1);
                    ascii_transitions_map[i].insert(closure.begin(), closure.end());
                }
            }
            // TODO: add this for the utf8 case
            /*
            for (const typename NFAStateType::Tree::Data& data : s0->get_tree_transitions().all()) {
                for (NFAStateType* const s1 : data.m_value) {
                    StateSet closure = epsilon_closure(s1);
                    transitions_map[data.m_interval].insert(closure.begin(), closure.end());
                }
            }
            */
        }
        auto next_dfa_state = [&dfa_states,
                               &create_dfa_state](StateSet const& set) -> DFAStateType* {
            DFAStateType* state{nullptr};
            auto it = dfa_states.find(set);
            if (it == dfa_states.end()) {
                state = create_dfa_state(set);
            } else {
                state = it->second;
            }
            return state;
        };
        for (typename std::map<uint32_t, StateSet>::value_type const& kv : ascii_transitions_map) {
            DFAStateType* dest_state = next_dfa_state(kv.second);
            dfa_state->add_byte_transition(kv.first, dest_state);
        }
        // TODO: add this for the utf8 case
        /*
        for (const typename map<Interval, typename NFAStateType::StateSet>::value_type& kv :
             transitions_map)
        {
            DFAStateType* dest_state = next_dfa_state(kv.second);
            dfa_state->add_tree_transition(kv.first, dest_state);
        }
        */
    }
    return dfa;
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LEXER_TPP
