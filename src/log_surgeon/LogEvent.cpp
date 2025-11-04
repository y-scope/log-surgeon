#include "LogEvent.hpp"

#include <cstdint>
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
        auto& token = m_log_output_buffer->get_mutable_token(i);
        raw_log += token.to_string_view();
    }
    return raw_log;
}

auto LogEventView::get_logtype() const -> std::string {
    std::string logtype;
    if (m_log_output_buffer->has_timestamp()) {
        logtype += "<timestamp>";
    }
    for (uint32_t i{1}; i < m_log_output_buffer->pos(); ++i) {
        auto token_view{m_log_output_buffer->get_mutable_token(i)};
        auto const rule_id{token_view.get_type_ids()->at(0)};
        if (static_cast<uint32_t>(SymbolId::TokenUncaughtString) == rule_id) {
            logtype += token_view.to_string_view();
        } else {
            bool const is_first_token{false == m_log_output_buffer->has_timestamp() && 1 == i};
            if (static_cast<uint32_t>(SymbolId::TokenNewline) != rule_id && false == is_first_token)
            {
                logtype += token_view.release_delimiter();
            }
            if (auto const& optional_capture_ids{
                        m_log_parser.m_lexer.get_capture_ids_from_rule_id(rule_id)
                };
                optional_capture_ids.has_value())
            {
                auto capture_view{token_view};
                auto const& capture_ids{optional_capture_ids.value()};
                for (auto const capture_id : capture_ids) {
                    auto const& optional_reg_id_pair{
                            m_log_parser.m_lexer.get_reg_ids_from_capture_id(capture_id)
                    };
                    if (false == optional_reg_id_pair.has_value()) {
                        continue;
                    }
                    auto const start_positions{
                            capture_view.get_reversed_reg_positions(optional_reg_id_pair->first)
                    };
                    auto const end_positions{
                            capture_view.get_reversed_reg_positions(optional_reg_id_pair->second)
                    };

                    auto capture_name{m_log_parser.get_id_symbol(capture_id)};
                    if (false == start_positions.empty() && -1 < start_positions[0]
                        && false == end_positions.empty() && -1 < end_positions[0])
                    {
                        capture_view.set_end_pos(start_positions[0]);
                        logtype.append(capture_view.to_string_view());
                        logtype.append("<" + capture_name + ">");
                        capture_view.set_start_pos(end_positions[0]);
                    }
                }
                capture_view.set_end_pos(token_view.get_end_pos());
                logtype.append(capture_view.to_string_view());
            } else {
                logtype += "<" + m_log_parser.get_id_symbol(rule_id) + ">";
            }
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
        // TODO: this is bad the token class should handle this copy, now the regs are missing
        Token copied_token{
                start_pos,
                curr_pos,
                m_buffer.data(),
                buffer_size,
                0,
                token.get_type_ids()
        };
        m_log_output_buffer->set_curr_token(copied_token);
        m_log_output_buffer->advance_to_next_token();
    }
    for (uint32_t i = 0; i < get_log_output_buffer()->pos(); i++) {
        Token& token = get_log_output_buffer()->get_mutable_token(i);
        auto const& token_types{*token.get_type_ids()};
        add_token(token_types[0], &token);
    }
}
}  // namespace log_surgeon
