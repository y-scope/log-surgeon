#ifndef LOG_SURGEON_QUERY_PARSER_WILDCARD_EXPRESSION_VIEW_HPP
#define LOG_SURGEON_QUERY_PARSER_WILDCARD_EXPRESSION_VIEW_HPP

#include <cstddef>
#include <span>
#include <string>
#include <string_view>

#include <log_surgeon/wildcard_query_parser/WildcardExpression.hpp>

namespace log_surgeon::wildcard_query_parser {
/**
 * A lightweight, non-owning view into a contiguous subrange of a WildcardExpression.
 *
 * This class provides a span to the underlying character vector and a view into the corresponding
 * search string. It ensures that these are always valid by clamping the provided indices to the
 * expression's length.
 *
 * Utilities include:
 * - Generating a regex string for the view.
 * - Checking if the view starts or ends with a greedy wildcard.
 * - Extending the view to include adjacent greedy wildcards.
 */
class WildcardExpressionView {
public:
    /**
     * Creates a view of the range [`begin_idx`, `end_idx`) in the given wildcard expression.
     *
     * NOTE: To ensure validity, `end_idx` is limited to `wildcard_expression.length()`, and then
     * `begin_idx` is limited to `end_idx`.
     * @param expression
     * @param begin_idx
     * @param end_idx
     */
    WildcardExpressionView(
            WildcardExpression const& expression,
            size_t begin_idx,
            size_t end_idx
    );

    /**
     * @return A copy of this view, but extended to include adjacent greedy wildcards.
     */
    [[nodiscard]] auto extend_to_adjacent_greedy_wildcards() const -> WildcardExpressionView;

    [[nodiscard]] auto starts_or_ends_with_greedy_wildcard() const -> bool {
        return false == m_chars.empty()
               && (m_chars[0].is_greedy_wildcard() || m_chars.back().is_greedy_wildcard());
    }

    [[nodiscard]] auto generate_regex_string() const -> std::string;
  
    [[nodiscard]] auto get_string() const -> std::string_view {
        return m_search_string;
    }

private:
    [[nodiscard]] auto get_indicies() const -> std::pair<size_t, size_t> {
        auto const& full_chars{m_expression->get_chars()};
        auto const begin_ptr{m_chars.data()};
        auto begin_idx{static_cast<size_t>(begin_ptr - full_chars.data())};
        return {begin_idx, begin_idx + m_chars.size()};
    }
  
    WildcardExpression const* m_expression;
    std::span<const WildcardCharacter> m_chars;
    std::string_view m_search_string;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_QUERY_PARSER_WILDCARD_EXPRESSION_VIEW_HPP
