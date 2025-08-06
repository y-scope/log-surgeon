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
 * Stores the raw log as a string with metadata specifying:
 * 1. The variable type.
 * 2. If the variable contains a wildcard.
 */
class VariableQueryToken {
public:
    VariableQueryToken(
            uint32_t const variable_type,
            std::string query_substring,
            bool const has_wildcard
    )
            : m_variable_type(variable_type),
              m_query_substring(std::move(query_substring)),
              m_has_wildcard(has_wildcard) {}

    /**
     * Lexicographical three-way comparison operator.
     *
     * Compares member variables in the following order:
     * 1. `m_variable_type`
     * 2. `m_query_substring`
     * 3. `m_has_wildcard` (with `false` considered less than `true`)
     *
     * @param rhs The `VariableQueryToken` to compare against.
     * @return The relative ordering of `this` with respect to `rhs`.
     */
    auto operator<=>(VariableQueryToken const& rhs) const -> std::strong_ordering;


    [[nodiscard]] auto get_variable_type() const -> uint32_t { return m_variable_type; }

    [[nodiscard]] auto get_query_substring() const -> std::string const& {
        return m_query_substring;
    }

    [[nodiscard]] auto get_has_wildcard() const -> bool { return m_has_wildcard; }

private:
    uint32_t m_variable_type;
    std::string m_query_substring;
    bool m_has_wildcard{false};
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_VARIABLE_QUERY_TOKEN_HPP
