#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_VARIABLE_QUERY_TOKEN_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_VARIABLE_QUERY_TOKEN_HPP

#include <compare>
#include <cstdint>
#include <string>
#include <utility>

namespace log_surgeon::wildcard_query_parser {
/**
 * Represents a variable in the query as a token.
 *
 * Stores a substring from the query with metadata specifying:
 * 1. The variable type.
 * 2. If the variable contains a wildcard.
 */
class VariableQueryToken {
public:
    VariableQueryToken(
            uint32_t const variable_type,
            std::string query_substring,
            bool const contains_wildcard,
            bool const contains_captures
    )
            : m_variable_type(variable_type),
              m_query_substring(std::move(query_substring)),
              m_contains_wildcard(contains_wildcard),
              m_contains_captures(contains_captures) {}

    // Must be defined if `operator<=>` is not defaulted.
    auto operator==(VariableQueryToken const& rhs) const -> bool {
        return (*this <=> rhs) == std::strong_ordering::equal;
    }

    /**
     * Lexicographical three-way comparison operator.
     *
     * Compares member variables in the following order:
     * 1. `m_variable_type`
     * 2. `m_query_substring`
     * 3. `m_contains_wildcard` (with `false` considered less than `true`)
     * 4. `m_contains_captures` (with `false` considered less than `true`)
     *
     * @param rhs The `VariableQueryToken` to compare against.
     * @return The relative ordering of `this` with respect to `rhs`.
     */
    auto operator<=>(VariableQueryToken const& rhs) const -> std::strong_ordering;

    [[nodiscard]] auto get_variable_type() const -> uint32_t { return m_variable_type; }

    [[nodiscard]] auto get_query_substring() const -> std::string const& {
        return m_query_substring;
    }

    [[nodiscard]] auto get_contains_wildcard() const -> bool { return m_contains_wildcard; }

    [[nodiscard]] auto get_contains_captures() const -> bool { return m_contains_captures; }

private:
    uint32_t m_variable_type;
    std::string m_query_substring;
    bool m_contains_wildcard{false};
    bool m_contains_captures{false};
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_VARIABLE_QUERY_TOKEN_HPP
