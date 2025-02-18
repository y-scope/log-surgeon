#ifndef LOG_SURGEON_LEXER_TPP
#define LOG_SURGEON_LEXER_TPP

#include <cassert>
#include <memory>
#include <stack>
#include <stdexcept>
#include <string>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/types.hpp>

/**
 * utf8 format (https://en.wikipedia.org/wiki/UTF-8)
 * 1 byte: 0x0 - 0x80 : 0xxxxxxx
 * 2 byte: 0x80 - 0x7FF : 110xxxxx 10xxxxxx
 * 3 byte: 0x800 - 0xFFFF : 1110xxxx 10xxxxxx 10xxxxxx
 * 4 byte: 0x10000 - 0x1FFFFF : 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
 */
namespace log_surgeon {
template <typename TypedNfaState, typename TypedDfaState>
auto Lexer<TypedNfaState, TypedDfaState>::flip_states(uint32_t old_storage_size) -> void {
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

template <typename TypedNfaState, typename TypedDfaState>
auto Lexer<TypedNfaState, TypedDfaState>::scan(ParserInputBuffer& input_buffer, Token& token)
        -> ErrorCode {
    auto const* state = m_dfa->get_root();
    if (m_asked_for_more_data) {
        state = m_prev_state;
        m_asked_for_more_data = false;
    } else {
        if (m_match) {
            m_match = false;
            m_last_match_pos = m_match_pos;
            m_last_match_line = m_match_line;
            token
                    = Token{m_start_pos,
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
        auto prev_byte_buf_pos = input_buffer.storage().pos();
        auto next_char{utf8::cCharErr};
        if (auto const err = input_buffer.get_next_character(next_char); ErrorCode::Success != err)
        {
            m_asked_for_more_data = true;
            m_prev_state = state;
            return err;
        }
        if ((m_is_delimiter[next_char] || input_buffer.log_fully_consumed() || !m_has_delimiters)
            && state->is_accepting())
        {
            m_match = true;
            m_type_ids = &(state->get_matching_variable_ids());
            m_match_pos = prev_byte_buf_pos;
            m_match_line = m_line;
        }
        auto* dest_state = state->get_dest_state(next_char);
        if (next_char == '\n') {
            m_line++;
            if (m_has_delimiters && !m_match) {
                dest_state = m_dfa->get_root()->get_dest_state(next_char);
                m_match = true;
                m_type_ids = &(dest_state->get_matching_variable_ids());
                m_start_pos = prev_byte_buf_pos;
                m_match_pos = input_buffer.storage().pos();
                m_match_line = m_line;
            }
        }
        if (input_buffer.log_fully_consumed() || nullptr == dest_state) {
            if (m_match) {
                input_buffer.set_log_fully_consumed(false);
                input_buffer.set_pos(m_match_pos);
                m_line = m_match_line;
                if (m_last_match_pos != m_start_pos) {
                    token
                            = Token{m_last_match_pos,
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
                token
                        = Token{m_start_pos,
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
                    token
                            = Token{m_last_match_pos,
                                    m_start_pos,
                                    input_buffer.storage().get_active_buffer(),
                                    input_buffer.storage().size(),
                                    m_last_match_line,
                                    &cTokenUncaughtStringTypes};
                    return ErrorCode::Success;
                }
                token
                        = Token{input_buffer.storage().pos(),
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
        state = dest_state;
    }
}

// TODO: this is duplicating almost all the code of scan()
template <typename TypedNfaState, typename TypedDfaState>
auto Lexer<TypedNfaState, TypedDfaState>::scan_with_wildcard(
        ParserInputBuffer& input_buffer,
        char wildcard,
        Token& token
) -> ErrorCode {
    auto const* state = m_dfa->get_root();
    if (m_asked_for_more_data) {
        state = m_prev_state;
        m_asked_for_more_data = false;
    } else {
        if (m_match) {
            m_match = false;
            m_last_match_pos = m_match_pos;
            m_last_match_line = m_match_line;
            token
                    = Token{m_start_pos,
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
        auto prev_byte_buf_pos = input_buffer.storage().pos();
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
            m_type_ids = &(state->get_matching_variable_ids());
            m_match_pos = prev_byte_buf_pos;
            m_match_line = m_line;
        }
        TypedDfaState const* dest_state{state->get_dest_state(next_char)};
        if (next_char == '\n') {
            m_line++;
            if (m_has_delimiters && !m_match) {
                dest_state = m_dfa->get_root()->get_dest_state(next_char);
                m_match = true;
                m_type_ids = &(dest_state->get_matching_variable_ids());
                m_start_pos = prev_byte_buf_pos;
                m_match_pos = input_buffer.storage().pos();
                m_match_line = m_line;
            }
        }
        if (input_buffer.log_fully_consumed() || nullptr == dest_state) {
            assert(input_buffer.log_fully_consumed());
            if (!m_match || (m_match && m_match_pos != input_buffer.storage().pos())) {
                token
                        = Token{m_last_match_pos,
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
                        auto* dest_state{state->get_dest_state(byte)};
                        if (false == dest_state->is_accepting()) {
                            token
                                    = Token{m_last_match_pos,
                                            input_buffer.storage().pos(),
                                            input_buffer.storage().get_active_buffer(),
                                            input_buffer.storage().size(),
                                            m_last_match_line,
                                            &cTokenUncaughtStringTypes};
                            return ErrorCode::Success;
                        }
                    }
                } else if (wildcard == '*') {
                    std::stack<TypedDfaState const*> unvisited_states;
                    std::set<TypedDfaState const*> visited_states;
                    unvisited_states.push(state);
                    while (!unvisited_states.empty()) {
                        TypedDfaState const* current_state = unvisited_states.top();
                        if (current_state == nullptr || current_state->is_accepting() == false) {
                            token
                                    = Token{m_last_match_pos,
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
                            TypedDfaState const* dest_state{current_state->get_dest_state(byte)};
                            if (false == visited_states.contains(dest_state)) {
                                unvisited_states.push(dest_state);
                            }
                        }
                    }
                }
                input_buffer.set_pos(m_match_pos);
                m_line = m_match_line;
                m_match = false;
                m_last_match_pos = m_match_pos;
                m_last_match_line = m_match_line;
                token
                        = Token{m_start_pos,
                                m_match_pos,
                                input_buffer.storage().get_active_buffer(),
                                input_buffer.storage().size(),
                                m_match_line,
                                m_type_ids};
                return ErrorCode::Success;
            }
        }
        state = dest_state;
    }
}

template <typename TypedNfaState, typename TypedDfaState>
auto Lexer<TypedNfaState, TypedDfaState>::increase_buffer_capacity(ParserInputBuffer& input_buffer
) -> void {
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

template <typename TypedNfaState, typename TypedDfaState>
void Lexer<TypedNfaState, TypedDfaState>::reset() {
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

template <typename TypedNfaState, typename TypedDfaState>
void Lexer<TypedNfaState, TypedDfaState>::prepend_start_of_file_char(ParserInputBuffer& input_buffer
) {
    m_prev_state = m_dfa->get_root()->get_dest_state(utf8::cCharStartOfFile);
    m_asked_for_more_data = true;
    m_start_pos = input_buffer.storage().pos();
    m_match_pos = input_buffer.storage().pos();
    m_match_line = m_line;
    m_type_ids = nullptr;
}

template <typename TypedNfaState, typename TypedDfaState>
void Lexer<TypedNfaState, TypedDfaState>::add_delimiters(std::vector<uint32_t> const& delimiters) {
    assert(!delimiters.empty());
    m_has_delimiters = true;
    for (auto& i : m_is_delimiter) {
        i = false;
    }
    for (auto delimiter : delimiters) {
        m_is_delimiter[delimiter] = true;
    }
    m_is_delimiter[utf8::cCharStartOfFile] = true;
}

template <typename TypedNfaState, typename TypedDfaState>
void Lexer<TypedNfaState, TypedDfaState>::add_rule(
        rule_id_t const rule_id,
        std::unique_ptr<finite_automata::RegexAST<TypedNfaState>> rule
) {
    m_rules.emplace_back(rule_id, std::move(rule));
}

template <typename TypedNfaState, typename TypedDfaState>
auto Lexer<TypedNfaState, TypedDfaState>::get_rule(rule_id_t const rule_id
) -> finite_automata::RegexAST<TypedNfaState>* {
    for (auto const& rule : m_rules) {
        if (rule.get_variable_id() == rule_id) {
            return rule.get_regex();
        }
    }
    return nullptr;
}

template <typename TypedNfaState, typename TypedDfaState>
void Lexer<TypedNfaState, TypedDfaState>::generate() {
    for (auto const& rule : m_rules) {
        for (auto const* capture : rule.get_captures()) {
            std::string const capture_name{capture->get_name()};
            if (m_symbol_id.contains(capture_name)) {
                throw std::invalid_argument("`m_rules` contains capture names that are not unique."
                );
            }
            auto const capture_id{m_symbol_id.size()};
            m_symbol_id.emplace(capture_name, capture_id);
            m_id_symbol.emplace(capture_id, capture_name);

            auto const rule_id{rule.get_variable_id()};
            m_rule_id_to_capture_ids.try_emplace(rule_id);
            m_rule_id_to_capture_ids.at(rule_id).push_back(capture_id);
        }
    }

    finite_automata::Nfa<TypedNfaState> nfa{m_rules};
    for (auto const& [capture, tag_id_pair] : nfa.get_capture_to_tag_id_pair()) {
        std::string const capture_name{capture->get_name()};
        auto const capture_id{m_symbol_id.at(capture_name)};
        m_capture_id_to_tag_id_pair.emplace(capture_id, tag_id_pair);
    }

    // TODO: DFA ignores captures. E.g., treats "capture:user=(?<user_id>\d+)" as "capture:user=\d+"
    m_dfa = std::make_unique<finite_automata::Dfa<TypedDfaState, TypedNfaState>>(nfa);
    auto const* state = m_dfa->get_root();
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        if (nullptr != state->get_dest_state(i)) {
            m_is_first_char[i] = true;
        } else {
            m_is_first_char[i] = false;
        }
    }
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LEXER_TPP
