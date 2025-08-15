#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_EXPRESSION_VIEW_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_EXPRESSION_VIEW_HPP

#include <cstddef>
#include <span>
#include <string>
#include <string_view>
#include <utility>

#include <log_surgeon/wildcard_query_parser/Expression.hpp>
#include <log_surgeon/wildcard_query_parser/ExpressionCharacter.hpp>

namespace log_surgeon::wildcard_query_parser {
/**
 * A lightweight, non-owning view into a contiguous subrange of an `Expression`.
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
class ExpressionView {
public:
    ExpressionView(Expression const& expression, size_t begin_idx, size_t end_idx);

    /**
     * Tries to extend the view to include adjacent greedy wildcards from the original expression.
     * @return A pair containing:
     * - True if there exists adjacent greedy wildcards in the original expression, false otherwise.
     * - A copy of this view, with any greedy wildcard extensions that could be made.
     */
    [[nodiscard]] auto extend_to_adjacent_greedy_wildcards() const
            -> std::pair<bool, ExpressionView>;

    [[nodiscard]] auto starts_or_ends_with_greedy_wildcard() const -> bool {
        return false == m_chars.empty()
               && (m_chars[0].is_greedy_wildcard() || m_chars.back().is_greedy_wildcard());
    }

    /**
     * Checks whether this `ExpressionView` represents a well-formed subrange.
     *
     * A subrange is well-formed if:
     * - It does not start immediately after an escaped character in the original expression.
     * - It does not end on an escape character.
     *
     * By these rules, an empty substring is always well-formed.
     *
     * These constraints ensure well-formed substrings are consistent with the original intention of
     * the `Expression`. For example, given the search query "* \*text\* *":
     * - The substring "*text" is not well-formed, as it incorrectly indicates a literal wildcard.
     * - The substring "text\" is not well-formed, as a single `\` has no clear meaning.
     *
     * @return `true` if the substring is well-formed, `false` otherwise.
     */
    [[nodiscard]] auto is_well_formed() const -> bool;

    /**
     * Builds a regex string representing this view.
     *
     * Transformations:
     * - Greedy wildcard (`*`) -> `.*`.
     * - Non-greedy wildcard (`?`) -> `.`.
     * - All other characters preserved literally, escaping (with `\`) as needed for regex syntax.
     *
     * @return a pair containing:
     * - `std::string` storing the regex string.
     * - `bool` indicating whether the regex string contains any wildcards.
     */
    [[nodiscard]] auto generate_regex_string() const -> std::pair<std::string, bool>;

    [[nodiscard]] auto get_search_string() const -> std::string_view { return m_search_string; }

private:
    [[nodiscard]] auto get_indices() const -> std::pair<size_t, size_t> {
        auto const& full_chars{m_expression->get_chars()};
        auto const* begin_ptr{m_chars.data()};
        auto begin_idx{static_cast<size_t>(begin_ptr - full_chars.data())};
        return {begin_idx, begin_idx + m_chars.size()};
    }

    Expression const* m_expression;
    std::span<ExpressionCharacter const> m_chars;
    std::string_view m_search_string;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_EXPRESSION_VIEW_HPP
