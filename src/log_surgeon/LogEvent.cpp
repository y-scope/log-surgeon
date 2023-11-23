#include "LogEvent.hpp"

#include <memory>
#include <string>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/LogParser.hpp>
#include <log_surgeon/LogParserOutputBuffer.hpp>
#include <log_surgeon/Token.hpp>

namespace log_surgeon {
LogEventView::LogEventView(LogParser const& log_parser)
        : m_log_parser{log_parser},
          m_log_var_occurrences{log_parser.m_lexer.m_id_symbol.size()} {
    m_log_output_buffer = std::make_unique<LogParserOutputBuffer>();
}

auto LogEventView::deep_copy() const -> LogEvent {
    return {*this};
}

auto LogEventView::reset() -> void {
    for (std::vector<Token*>& log_var_occ : m_log_var_occurrences) {
        log_var_occ.clear();
    }
    m_log_output_buffer->reset();
    m_multiline = false;
}

[[nodiscard]] auto LogEventView::get_timestamp() const -> Token* {
    if (m_log_output_buffer->has_timestamp()) {
        return &m_log_output_buffer->get_mutable_token(0);
    }
    return nullptr;
}

[[nodiscard]] auto LogEventView::to_string() const -> std::string {
    std::string raw_log;
    uint32_t start = 0;
    if (false == m_log_output_buffer->has_timestamp()) {
        start = 1;
    }
    for (uint32_t i = start; i < m_log_output_buffer->pos(); i++) {
        Token& token = m_log_output_buffer->get_mutable_token(i);
        raw_log += token.to_string_view();
    }
    return raw_log;
}

auto LogEventView::get_logtype() const -> std::string {
    std::string logtype;
    for (uint32_t i = 1; i < m_log_output_buffer->pos(); i++) {
        Token& token = m_log_output_buffer->get_mutable_token(i);
        if (token.m_type_ids_ptr->at(0) == (int)log_surgeon::SymbolID::TokenUncaughtStringID) {
            logtype += token.to_string_view();
        } else {
            if ((int)log_surgeon::SymbolID::TokenNewlineId != token.m_type_ids_ptr->at(0)) {
                logtype += token.get_delimiter();
            }
            logtype += "<";
            logtype += m_log_parser.get_id_symbol(token.m_type_ids_ptr->at(0));
            logtype += ">";
        }
    }
    return logtype;
}

LogEvent::LogEvent(LogEventView const& src) : LogEventView{src.get_log_parser()} {
    set_multiline(src.is_multiline());
    m_log_output_buffer->set_has_timestamp(src.m_log_output_buffer->has_timestamp());
    m_log_output_buffer->set_has_delimiters(src.m_log_output_buffer->has_delimiters());
    uint32_t start = 0;
    if (nullptr == src.get_timestamp()) {
        start = 1;
    }
    uint32_t buffer_size{0};
    for (uint32_t i = start; i < src.get_log_output_buffer()->pos(); i++) {
        Token const& token = src.get_log_output_buffer()->get_token(i);
        buffer_size += token.get_length();
    }
    if (0 >= buffer_size) {
        throw std::runtime_error("token buffer_size <= 0");
    }
    m_buffer.resize(buffer_size);
    uint32_t curr_pos = 0;
    for (uint32_t i = start; i < src.get_log_output_buffer()->pos(); i++) {
        Token& token = src.get_log_output_buffer()->get_mutable_token(i);
        uint32_t start_pos = curr_pos;
        for (char const& c : token.to_string_view()) {
            m_buffer[curr_pos] = c;
            curr_pos++;
        }
        Token copied_token{
                start_pos,
                curr_pos,
                m_buffer.data(),
                buffer_size,
                0,
                token.m_type_ids_ptr};
        m_log_output_buffer->set_curr_token(copied_token);
        m_log_output_buffer->advance_to_next_token();
    }
    for (uint32_t i = 0; i < get_log_output_buffer()->pos(); i++) {
        Token& token = get_log_output_buffer()->get_mutable_token(i);
        std::vector<int> const& token_types = *token.m_type_ids_ptr;
        add_token(token_types[0], &token);
    }
}
}  // namespace log_surgeon
