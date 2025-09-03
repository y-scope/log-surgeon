#include "ExpressionView.hpp"

#include <algorithm>
#include <array>
#include <cstddef>
#include <span>
#include <string>
#include <string_view>
#include <utility>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/SchemaParser.hpp>
#include <log_surgeon/wildcard_query_parser/Expression.hpp>

using std::string;

namespace log_surgeon::wildcard_query_parser {
ExpressionView::ExpressionView(Expression const& expression, size_t begin_idx, size_t end_idx)
        : m_expression{&expression} {
    std::span const full_span{m_expression->get_chars()};
    end_idx = std::min(end_idx, full_span.size());
    begin_idx = std::min(begin_idx, end_idx);
    m_chars = full_span.subspan(begin_idx, end_idx - begin_idx);
    std::string_view const full_view{m_expression->get_search_string()};
    m_search_string = full_view.substr(begin_idx, end_idx - begin_idx);
}

auto ExpressionView::extend_to_adjacent_greedy_wildcards() const
        -> std::pair<bool, ExpressionView> {
    auto [begin_idx, end_idx]{get_indices()};
    bool is_extended{false};

    std::span const full_span{m_expression->get_chars()};

    if (begin_idx > 0 && full_span[begin_idx - 1].is_greedy_wildcard()) {
        --begin_idx;
        is_extended = true;
    }
    if (end_idx < full_span.size() && full_span[end_idx].is_greedy_wildcard()) {
        ++end_idx;
        is_extended = true;
    }
    ExpressionView const wildcard_expression_view{*m_expression, begin_idx, end_idx};
    return {is_extended, wildcard_expression_view};
}

auto ExpressionView::is_surrounded_by_delims(std::array<bool, cSizeOfByte> const& delim_table) const
        -> bool {
    auto const [begin_idx, end_idx]{get_indices()};

    bool has_left_boundary{false};
    if (0 == begin_idx) {
        has_left_boundary = true;
    } else {
        auto const& preceding_char{m_expression->get_chars()[begin_idx - 1]};
        has_left_boundary = preceding_char.is_delim_or_wildcard(delim_table)
                            || (false == m_chars.empty() && m_chars.front().is_greedy_wildcard());
    }

    bool has_right_boundary{false};
    if (m_expression->length() == end_idx) {
        has_right_boundary = true;
    } else {
        auto const& succeeding_char{m_expression->get_chars()[end_idx]};
        if (succeeding_char.is_escape()) {
            if (m_expression->length() > end_idx + 1) {
                auto const& logical_succeeding_char{m_expression->get_chars()[end_idx + 1]};
                has_right_boundary = logical_succeeding_char.is_delim(delim_table);
            }
        } else {
            has_right_boundary = succeeding_char.is_delim_or_wildcard(delim_table);
        }
        has_right_boundary = has_right_boundary
                             || (false == m_chars.empty() && m_chars.back().is_greedy_wildcard());
    }

    return has_left_boundary && has_right_boundary;
}

auto ExpressionView::is_well_formed() const -> bool {
    if (m_chars.empty()) {
        return true;
    }
    auto const [begin_idx, end_idx]{get_indices()};
    if (begin_idx > 0 && m_expression->get_chars()[begin_idx - 1].is_escape()) {
        return false;
    }
    if (m_chars.back().is_escape()) {
        return false;
    }
    return true;
}

auto ExpressionView::generate_regex_string() const -> std::pair<string, bool> {
    string regex_string;
    regex_string.reserve(m_chars.size() * 2);
    bool regex_contains_wildcard{false};

    for (auto const& expression_char : m_chars) {
        if (expression_char.is_escape()) {
            continue;
        }
        auto const& value{expression_char.value()};
        if (expression_char.is_greedy_wildcard()) {
            regex_string += ".*";
            regex_contains_wildcard = true;
        } else if (expression_char.is_non_greedy_wildcard()) {
            regex_string += ".";
            regex_contains_wildcard = true;
        } else if (SchemaParser::get_special_regex_characters().contains(value)) {
            regex_string += "\\";
            regex_string += value;
        } else {
            regex_string += value;
        }
    }
    return {regex_string, regex_contains_wildcard};
}
}  // namespace log_surgeon::wildcard_query_parser
