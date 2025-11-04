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
auto Lexer<TypedNfaState, TypedDfaState>::scan(ParserInputBuffer& input_buffer)
        -> std::pair<ErrorCode, std::optional<Token>> {
    if (m_asked_for_more_data) {
        m_asked_for_more_data = false;
    } else {
        m_state = m_dfa->get_root();
        if (m_match) {
            m_first_delimiter_pos = std::nullopt;
            m_match = false;
            m_last_match_pos = m_match_pos;
            m_last_match_line = m_match_line;
            Token token{
                    m_start_pos,
                    m_match_pos,
                    input_buffer.storage().get_active_buffer(),
                    input_buffer.storage().size(),
                    m_match_line,
                    m_type_ids,
                    m_dfa->release_reg_handler()
            };
            return {ErrorCode::Success, token};
        }
        if (m_first_delimiter_pos.has_value()) {
            input_buffer.set_log_fully_consumed(false);
            input_buffer.set_pos(m_first_delimiter_pos.value());
        }
        m_start_pos = input_buffer.storage().pos();
        m_match_pos = input_buffer.storage().pos();
        m_match_line = m_line;
        m_type_ids = nullptr;
        m_first_delimiter_pos = std::nullopt;
    }
    while (true) {
        auto prev_byte_buf_pos{input_buffer.storage().pos()};
        auto next_char{utf8::cCharErr};
        if (auto const err{input_buffer.get_next_character(next_char)}; ErrorCode::Success != err) {
            m_asked_for_more_data = true;
            return {err, std::nullopt};
        }
        if (false == m_first_delimiter_pos.has_value() && m_is_delimiter[next_char]
            && prev_byte_buf_pos != m_last_match_pos)
        {
            m_first_delimiter_pos = prev_byte_buf_pos;
        }

        if ((m_is_delimiter[next_char] || input_buffer.log_fully_consumed()
             || false == m_has_delimiters)
            && m_state->is_accepting())
        {
            m_dfa->process_reg_ops(m_state->get_accepting_reg_ops(), prev_byte_buf_pos);
            m_match = true;
            m_type_ids = &m_state->get_matching_variable_ids();
            m_match_pos = prev_byte_buf_pos;
            m_match_line = m_line;
        }
        process_char(next_char, prev_byte_buf_pos);

        if ('\n' == next_char) {
            m_line++;
            // The newline character itself needs to be treated as a match for non-timestamped logs.
            if (m_has_delimiters && false == m_match) {
                m_state = m_dfa->get_root();
                process_char(next_char, prev_byte_buf_pos);
                m_match = true;
                m_type_ids = &m_state->get_matching_variable_ids();
                m_start_pos = prev_byte_buf_pos;
                m_match_pos = input_buffer.storage().pos();
                m_match_line = m_line;
                continue;
            }
        }

        if (input_buffer.log_fully_consumed() || nullptr == m_state) {
            if (m_match) {
                input_buffer.set_log_fully_consumed(false);
                input_buffer.set_pos(m_match_pos);
                m_line = m_match_line;
                if (m_last_match_pos != m_start_pos) {
                    Token token{
                            m_last_match_pos,
                            m_start_pos,
                            input_buffer.storage().get_active_buffer(),
                            input_buffer.storage().size(),
                            m_last_match_line,
                            &cTokenUncaughtStringTypes
                    };
                    return {ErrorCode::Success, token};
                }
                m_first_delimiter_pos = std::nullopt;
                m_match = false;
                m_last_match_pos = m_match_pos;
                m_last_match_line = m_match_line;
                Token token{
                        m_start_pos,
                        m_match_pos,
                        input_buffer.storage().get_active_buffer(),
                        input_buffer.storage().size(),
                        m_match_line,
                        m_type_ids,
                        m_dfa->release_reg_handler()
                };
                return {ErrorCode::Success, token};
            }
            if (input_buffer.log_fully_consumed() && input_buffer.storage().pos() == m_start_pos) {
                if (m_last_match_pos != m_start_pos) {
                    if (m_first_delimiter_pos.has_value()) {
                        Token token{
                                m_last_match_pos,
                                m_first_delimiter_pos.value(),
                                input_buffer.storage().get_active_buffer(),
                                input_buffer.storage().size(),
                                m_last_match_line,
                                &cTokenUncaughtStringTypes
                        };
                        m_last_match_pos = m_first_delimiter_pos.value();
                        return {ErrorCode::Success, token};
                    } else {
                        m_match_pos = input_buffer.storage().pos();
                        m_type_ids = &cTokenEndTypes;
                        m_match = true;
                        Token token{
                                m_last_match_pos,
                                m_start_pos,
                                input_buffer.storage().get_active_buffer(),
                                input_buffer.storage().size(),
                                m_last_match_line,
                                &cTokenUncaughtStringTypes
                        };
                        return {ErrorCode::Success, token};
                    }
                }
                Token token{
                        input_buffer.storage().pos(),
                        input_buffer.storage().pos(),
                        input_buffer.storage().get_active_buffer(),
                        input_buffer.storage().size(),
                        m_line,
                        &cTokenEndTypes
                };
                return {ErrorCode::Success, token};
            }
            if (m_first_delimiter_pos.has_value()) {
                Token token{
                        m_last_match_pos,
                        m_first_delimiter_pos.value(),
                        input_buffer.storage().get_active_buffer(),
                        input_buffer.storage().size(),
                        m_last_match_line,
                        &cTokenUncaughtStringTypes
                };
                m_last_match_pos = m_first_delimiter_pos.value();
                return {ErrorCode::Success, token};
            }
            m_first_delimiter_pos = std::nullopt;

            // TODO: remove timestamp from m_is_fist_char so that m_is_delimiter check not needed
            m_state = m_dfa->get_root();
            while (false == input_buffer.log_fully_consumed()
                   && (false == m_is_first_char_of_a_variable[next_char]
                       || false == m_is_delimiter[next_char]))
            {
                prev_byte_buf_pos = input_buffer.storage().pos();
                if (auto err{input_buffer.get_next_character(next_char)}; ErrorCode::Success != err)
                {
                    m_asked_for_more_data = true;
                    return {err, std::nullopt};
                }
            }
            input_buffer.set_pos(prev_byte_buf_pos);
            m_start_pos = prev_byte_buf_pos;
        }
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
        auto const& optional_transition{state->get_transition(next_char)};
        if (next_char == '\n') {
            m_line++;
            if (m_has_delimiters && !m_match) {
                auto const* dest_state{
                        m_dfa->get_root()->get_transition(next_char)->get_dest_state()
                };
                m_match = true;
                m_type_ids = &(dest_state->get_matching_variable_ids());
                m_start_pos = prev_byte_buf_pos;
                m_match_pos = input_buffer.storage().pos();
                m_match_line = m_line;
            }
        }
        if (input_buffer.log_fully_consumed() || false == optional_transition.has_value()) {
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
                        auto const* dest_state{state->get_transition(byte)->get_dest_state()};
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
                            auto const& optional_wildcard_transition{
                                    current_state->get_transition(byte)
                            };
                            if (false == optional_wildcard_transition.has_value()) {
                                unvisited_states.push(nullptr);
                                continue;
                            }
                            auto const* dest_state{optional_wildcard_transition->get_dest_state()};
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
        state = optional_transition->get_dest_state();
    }
}

template <typename TypedNfaState, typename TypedDfaState>
auto Lexer<TypedNfaState, TypedDfaState>::increase_buffer_capacity(ParserInputBuffer& input_buffer)
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
    m_first_delimiter_pos = std::nullopt;
    m_state = nullptr;
}

template <typename TypedNfaState, typename TypedDfaState>
void
Lexer<TypedNfaState, TypedDfaState>::prepend_start_of_file_char(ParserInputBuffer& input_buffer) {
    m_state = m_dfa->get_root()->get_transition(utf8::cCharStartOfFile)->get_dest_state();
    m_asked_for_more_data = true;
    m_start_pos = input_buffer.storage().pos();
    m_match_pos = input_buffer.storage().pos();
    m_match_line = m_line;
    m_type_ids = nullptr;
}

template <typename TypedNfaState, typename TypedDfaState>
void Lexer<TypedNfaState, TypedDfaState>::set_delimiters(std::vector<uint32_t> const& delimiters) {
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
auto Lexer<TypedNfaState, TypedDfaState>::get_highest_priority_rule(rule_id_t const rule_id)
        -> finite_automata::RegexAST<TypedNfaState>* {
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
                throw std::invalid_argument(
                        "`m_rules` contains capture names that are not unique."
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

    m_dfa = std::make_unique<finite_automata::Dfa<TypedDfaState, TypedNfaState>>(nfa);

    auto const* state = m_dfa->get_root();
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        m_is_first_char_of_a_variable[i] = state->get_transition(i).has_value();
    }
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LEXER_TPP
