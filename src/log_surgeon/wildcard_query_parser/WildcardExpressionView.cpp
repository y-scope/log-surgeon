#include "WildcardExpressionView.hpp"

#include <algorithm>
#include <cstddef>
#include <span>
#include <string>
#include <string_view>
#include <utility>

#include <log_surgeon/SchemaParser.hpp>

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
    std::string_view const full_view{m_expression->get_string()};
    m_search_string = full_view.substr(begin_idx, end_idx - begin_idx);
}

auto WildcardExpressionView::extend_to_adjacent_greedy_wildcards() const -> WildcardExpressionView {
    auto [begin_idx, end_idx]{get_indicies()};

    std::span const full_span{m_expression->get_chars()};

    if (begin_idx > 0 && full_span[begin_idx - 1].is_greedy_wildcard()) {
        --begin_idx;
    }
    if (end_idx < full_span.size() && full_span[end_idx].is_greedy_wildcard()) {
        ++end_idx;
    }
    return {*m_expression, begin_idx, end_idx};
}

auto WildcardExpressionView::is_well_formed() const -> bool {
    if (m_chars.empty()) {
        // Empty substring is trivially well-formed as it has no characters to violate requirements.
        return true;
    }
    auto const [begin_idx, end_idx]{get_indicies()};
    if (begin_idx > 0 && m_expression->get_chars()[begin_idx - 1].is_escape()) {
        // Substring starting immediately after an escape char is invalid.
        return false;
    }
    if (m_chars.back().is_escape()) {
        // Substring ending on an escape char is invalid.
        return false;
    }
    return true;
}

auto WildcardExpressionView::generate_regex_string() const -> std::pair<string, bool> {
    string regex_string;
    bool regex_contains_wildcard{false};

    for (auto const& wildcard_char : m_chars) {
        if (wildcard_char.is_escape()) {
            continue;
        }
        auto const& value{wildcard_char.value()};
        if (wildcard_char.is_greedy_wildcard()) {
            regex_string += ".*";
            regex_contains_wildcard = true;
        } else if (wildcard_char.is_non_greedy_wildcard()) {
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
