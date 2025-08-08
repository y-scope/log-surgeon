#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_INTERPRETATION_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_INTERPRETATION_HPP

#include <compare>
#include <cstdint>
#include <string>
#include <utility>
#include <variant>
#include <vector>

#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>
#include <log_surgeon/wildcard_query_parser/VariableQueryToken.hpp>

namespace log_surgeon::wildcard_query_parser {
/**
 * Represents a query as a sequence of static-text and variable tokens.
 *
 * The token sequence is stored in a canonicalized form - e.g., adjacent static tokens are merged -
 * to ensure a unique internal representation for accurate comparison.
 */
class QueryInterpretation {
public:
    QueryInterpretation() = default;

    explicit QueryInterpretation(std::string const& query_substring) {
        append_static_token(query_substring);
    }

    QueryInterpretation(
            uint32_t const variable_type,
            std::string query_substring,
            bool const contains_wildcard
    ) {
        append_variable_token(variable_type, std::move(query_substring), contains_wildcard);
    }

    // Must be defined if `operator<=>` is not defaulted.
    auto operator==(QueryInterpretation const& rhs) const -> bool {
        return (*this <=> rhs) == std::strong_ordering::equal;
    }

    /**
     * Lexicographical three-way comparison operator.
     *
     * Compares `m_tokens` lexicographically using their three-way comparison.
     *
     * @param rhs The `QueryInterpretation` to compare against.
     * @return The relative ordering of `this` with respect to `rhs`.
     */
    auto operator<=>(QueryInterpretation const& rhs) const -> std::strong_ordering;

    auto clear() -> void { m_tokens.clear(); }

    /**
     * Appends the logtype of another `QueryInterpretation` to this one.
     *
     * If the last token in this logtype and the first token in the suffix are both
     * `StaticQueryToken`, they are merged to avoid unnecessary token boundaries. The merged token
     * replaces the last token of this logtype, and the remaining suffix tokens are appended as-is.
     *
     * This merging behavior ensures a canonical internal representation, which is essential for
     * maintaining consistent comparison semantics.
     *
     * @param suffix The `QueryInterpretation` to append.
     */
    auto append_query_interpretation(QueryInterpretation const& suffix) -> void;

    /**
     * Appends a static query substring to the current interpretation.
     *
     * If the input string is empty, the function returns immediately. Otherwise, it attempts to
     * merge the new static substring into the last token if the last token is a
     * `StaticQueryToken`. If merging is not possible, a new `StaticQueryToken` is created from the
     * input substring and it is added as a new entry in the token vector.
     *
     * @param query_substring The static portion of the query to append.
     */
    auto append_static_token(std::string const& query_substring) -> void;

    auto append_variable_token(
            uint32_t const variable_type,
            std::string query_substring,
            bool const contains_wildcard
    ) -> void {
        m_tokens.emplace_back(
                VariableQueryToken(variable_type, std::move(query_substring), contains_wildcard)
        );
    }

    [[nodiscard]] auto get_logtype() const
            -> std::vector<std::variant<StaticQueryToken, VariableQueryToken>> {
        return m_tokens;
    }

    /**
     * @return A string representation of the QueryInterpretation.
     */
    [[nodiscard]] auto serialize() const -> std::string;

private:
    std::vector<std::variant<StaticQueryToken, VariableQueryToken>> m_tokens;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_INTERPRETATION_HPP
