#include "LogEvent.hpp"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

#include <ystdlib/error_handling/Result.hpp>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Capture.hpp>
#include <log_surgeon/finite_automata/PrefixTree.hpp>
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

[[nodiscard]] auto LogEventView::get_timestamp() const -> std::optional<std::string> const& {
    return m_log_output_buffer->get_timestamp();
}

[[nodiscard]] auto LogEventView::to_string() const -> std::string {
    std::string raw_log;
    uint32_t start = 0;
    if (false == m_log_output_buffer->has_header()) {
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
    uint32_t buffer_start{1};
    if (m_log_output_buffer->has_header()) {
        buffer_start = 0;
    }
    for (uint32_t i{buffer_start}; i < m_log_output_buffer->pos(); ++i) {
        auto token_view{m_log_output_buffer->get_mutable_token(i)};
        auto const rule_id{token_view.get_type_ids()->at(0)};
        if (static_cast<uint32_t>(SymbolId::TokenUncaughtString) == rule_id) {
            logtype += token_view.to_string_view();
        } else {
            bool is_first_token;
            if (m_log_output_buffer->has_header()) {
                is_first_token = 0 == i;
            } else {
                is_first_token = 1 == i;
            }
            if (static_cast<uint32_t>(SymbolId::TokenNewline) != rule_id && false == is_first_token)
            {
                logtype += token_view.release_delimiter();
            }
            auto const& optional_captures{m_log_parser.m_lexer.get_captures_from_rule_id(rule_id)};
            if (optional_captures.has_value()) {
                auto capture_view{token_view};
                auto const& captures{optional_captures.value()};
                for (auto const capture : captures) {
                    auto const [reg_start_id, reg_end_id]{
                            m_log_parser.m_lexer.get_reg_ids_from_capture(capture)
                    };
                    auto const start_positions{
                            capture_view.get_reversed_reg_positions(reg_start_id)
                    };
                    auto const end_positions{capture_view.get_reversed_reg_positions(reg_end_id)};

                    auto const& capture_name{capture->get_name()};
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

auto LogEventView::get_capture_matches(Token const& root_var) const
        -> ystdlib::error_handling::Result<std::vector<Token::CaptureMatch>> {
    auto captures{
            get_log_parser().m_lexer.get_captures_from_rule_id(root_var.get_type_ids()->at(0))
    };
    if (false == captures.has_value()) {
        return std::make_error_code(std::errc::invalid_argument);
    }

    try {
        std::sort(captures->begin(), captures->end(), [&](auto& a, auto& b) -> bool {
            static constexpr std::string_view cErrorFmt{
                    "Could not get positions for capture: {}, error: {} ({})"
            };

            auto const a_pos_result{get_capture_positions(root_var, a)};
            if (a_pos_result.has_error()) {
                throw std::runtime_error(
                        fmt::format(
                                cErrorFmt,
                                a->get_name(),
                                a_pos_result.error().category().name(),
                                a_pos_result.error().message()
                        )
                );
            }
            auto const [a_start_pos, a_end_pos]{a_pos_result.value()};

            auto const b_pos_result{get_capture_positions(root_var, b)};
            if (b_pos_result.has_error()) {
                throw std::runtime_error(
                        fmt::format(
                                cErrorFmt,
                                b->get_name(),
                                b_pos_result.error().category().name(),
                                b_pos_result.error().message()
                        )
                );
            }
            auto const [b_start_pos, b_end_pos]{b_pos_result.value()};

            if (a_start_pos != b_start_pos) {
                return a_start_pos < b_start_pos;
            }
            return a_end_pos > b_end_pos;
        });
    } catch (std::runtime_error const& exception) {
        return std::make_error_code(std::errc::invalid_argument);
    }

    std::vector<Token::CaptureMatch> matches;
    for (size_t i{0}; i < captures->size() - 1; i++) {
        auto const* const capture{captures->at(i)};
        auto [cur_start_pos,
              cur_end_pos]{YSTDLIB_ERROR_HANDLING_TRYX(get_capture_positions(root_var, capture))};
        auto [unused, next_end_pos]{
                YSTDLIB_ERROR_HANDLING_TRYX(get_capture_positions(root_var, captures->at(i + 1)))
        };
        auto leaf{false};
        if (next_end_pos > cur_end_pos) {
            leaf = true;
        }
        matches.emplace_back(
                capture,
                Token::CaptureMatchPosition{cur_start_pos, cur_end_pos},
                leaf
        );
    }
    auto [start_pos,
          end_pos]{YSTDLIB_ERROR_HANDLING_TRYX(get_capture_positions(root_var, captures->back()))};
    matches.emplace_back(captures->back(), Token::CaptureMatchPosition{start_pos, end_pos}, true);
    return matches;
}

auto LogEventView::get_capture_positions(
        Token const& root_variable,
        finite_automata::Capture const* const& capture
) const -> ystdlib::error_handling::Result<Token::CaptureMatchPosition> {
    auto const [start_reg_id, end_reg_id]{
            get_log_parser().m_lexer.get_reg_ids_from_capture(capture)
    };
    auto const start_positions{root_variable.get_reversed_reg_positions(start_reg_id)};
    auto const end_positions{root_variable.get_reversed_reg_positions(end_reg_id)};
    if (start_positions.empty() || 0 > start_positions[0] || end_positions.empty()
        || 0 > end_positions[0])
    {
        return std::make_error_code(std::errc::protocol_error);
    }
    return {start_positions[0], end_positions[0]};
}

LogEvent::LogEvent(LogEventView const& src) : LogEventView{src.get_log_parser()} {
    set_multiline(src.is_multiline());
    m_log_output_buffer->set_has_header(src.m_log_output_buffer->has_header());
    m_log_output_buffer->set_timestamp(src.m_log_output_buffer->get_timestamp());
    m_log_output_buffer->set_has_delimiters(src.m_log_output_buffer->has_delimiters());
    uint32_t start{0};
    if (false == src.m_log_output_buffer->has_header()) {
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
