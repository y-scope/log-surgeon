#include "WildcardExpressionView.hpp"

#include <algorithm>
#include <cstddef>
#include <span>
#include <string>
#include <string_view>
#include <utility>

#include <log_surgeon/SchemaParser.hpp>
#include <log_surgeon/wildcard_query_parser/WildcardExpression.hpp>

using std::string;

namespace log_surgeon::wildcard_query_parser {
WildcardExpressionView::WildcardExpressionView(
        WildcardExpression const& expression,
        size_t begin_idx,
        size_t end_idx
)
        : m_expression{&expression} {
    std::span const full_span{m_expression->get_chars()};
    end_idx = std::min(end_idx, full_span.size());
    begin_idx = std::min(begin_idx, end_idx);
    m_chars = full_span.subspan(begin_idx, end_idx - begin_idx);
    std::string_view const full_view{m_expression->get_search_string()};
    m_search_string = full_view.substr(begin_idx, end_idx - begin_idx);
}

auto WildcardExpressionView::extend_to_adjacent_greedy_wildcards() const -> std::pair<bool, WildcardExpressionView> {
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
    WildcardExpressionView wildcard_expression_view{*m_expression, begin_idx, end_idx};
    return {is_extended, wildcard_expression_view};
}

auto WildcardExpressionView::is_well_formed() const -> bool {
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

auto WildcardExpressionView::generate_regex_string() const -> std::pair<string, bool> {
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
