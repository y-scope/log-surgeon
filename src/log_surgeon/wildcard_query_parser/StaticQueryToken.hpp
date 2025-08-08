#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_STATIC_QUERY_TOKEN_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_STATIC_QUERY_TOKEN_HPP

#include <compare>
#include <string>
#include <utility>

namespace log_surgeon::wildcard_query_parser {
/**
 * Represents static-text in the query as a token.
 *
 * Stores a substring from the query.
 */
class StaticQueryToken {
public:
    explicit StaticQueryToken(std::string query_substring)
            : m_query_substring(std::move(query_substring)) {}

    auto operator<=>(StaticQueryToken const& rhs) const -> std::strong_ordering = default;

    auto append(StaticQueryToken const& rhs) -> void {
        m_query_substring += rhs.get_query_substring();
    }

    [[nodiscard]] auto get_query_substring() const -> std::string const& {
        return m_query_substring;
    }

private:
    std::string m_query_substring;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_STATIC_QUERY_TOKEN_HPP
