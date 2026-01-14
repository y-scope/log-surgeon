#include "LogEvent.hpp"

#include <cstddef>
#include <cstdint>
#include <iterator>
#include <memory>
#include <optional>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>

#include <ystdlib/error_handling/ErrorCode.hpp>
#include <ystdlib/error_handling/Result.hpp>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Capture.hpp>
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
            logtype.append(token_view.to_string_view());
            continue;
        }

        bool is_first_token{};
        if (m_log_output_buffer->has_header()) {
            is_first_token = 0 == i;
        } else {
            is_first_token = 1 == i;
        }
        if (static_cast<uint32_t>(SymbolId::TokenNewline) != rule_id && false == is_first_token) {
            logtype += token_view.release_delimiter();
        }

        auto const matches{get_capture_matches(token_view)};
        if (matches.has_error()) {
            logtype.append("<" + m_log_parser.get_id_symbol(rule_id) + ">");
            continue;
        }
        auto pos{token_view.get_start_pos()};
        for (auto const& match : matches.value()) {
            if (match.m_leaf) {
                logtype.append(token_view.get_sub_token(pos, match.m_pos.m_start).to_string_view());
                logtype.append("<" + match.m_capture->get_name() + ">");
                pos = match.m_pos.m_end;
            }
        }
        logtype.append(token_view.get_sub_token(pos, token_view.get_end_pos()).to_string_view());
    }
    return logtype;
}

auto LogEventView::get_capture_matches(Token const& root_var) const
        -> ystdlib::error_handling::Result<std::vector<Token::CaptureMatch>> {
    auto captures{
            get_log_parser().m_lexer.get_captures_from_rule_id(root_var.get_type_ids()->at(0))
    };
    if (false == captures.has_value()) {
        return LogEventErrorCode{LogEventErrorCodeEnum::NoCaptureGroups};
    }

    auto cmp{[](Token::CaptureMatch const& a, Token::CaptureMatch const& b) -> bool {
        if (a.m_pos.m_start != b.m_pos.m_start) {
            return a.m_pos.m_start < b.m_pos.m_start;
        }
        return a.m_pos.m_end > b.m_pos.m_end;
    }};
    std::set<Token::CaptureMatch, decltype(cmp)> ordered_matches;
    for (size_t i{0}; i < captures->size(); ++i) {
        auto const* const capture{captures->at(i)};
        auto position{get_capture_position(root_var, capture)};
        if (position.has_error()
            && LogEventErrorCode{LogEventErrorCodeEnum::NoCaptureGroupMatch} == position.error())
        {
            continue;
        }
        ordered_matches.emplace(capture, position.value(), true);
    }
    if (ordered_matches.empty()) {
        return {{}};
    }

    std::vector<Token::CaptureMatch> matches;
    matches.reserve(ordered_matches.size());
    auto const last_match{std::prev(ordered_matches.end())};
    for (auto match = ordered_matches.begin(); match != last_match; ++match) {
        auto next_match{std::next(match)};
        auto leaf{false};
        if (match->m_pos.m_end <= next_match->m_pos.m_start) {
            leaf = true;
        }
        matches.emplace_back(match->m_capture, match->m_pos, leaf);
    }
    matches.emplace_back(last_match->m_capture, last_match->m_pos, true);
    return matches;
}

auto LogEventView::get_capture_position(
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
        return LogEventErrorCode{LogEventErrorCodeEnum::NoCaptureGroupMatch};
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

using log_surgeon::LogEventErrorCodeEnum;

using LogEventErrorCategory = ystdlib::error_handling::ErrorCategory<LogEventErrorCodeEnum>;

template <>
auto LogEventErrorCategory::name() const noexcept -> char const* {
    return "log_surgeon::LogEvent";
}

template <>
auto LogEventErrorCategory::message(LogEventErrorCodeEnum error_enum) const -> std::string {
    switch (error_enum) {
        case LogEventErrorCodeEnum::NoCaptureGroups:
            return "LogEvent NoCaptureGroup";
        case LogEventErrorCodeEnum::NoCaptureGroupMatch:
            return "LogEvent NoCaptureGroupMatch";
        default:
            return "Unrecognized LogEventErrorCode";
    }
}
