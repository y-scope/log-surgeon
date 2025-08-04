#ifndef LOG_SURGEON_QUERY_PARSER_STATIC_QUERY_TOKEN_HPP
#define LOG_SURGEON_QUERY_PARSER_STATIC_QUERY_TOKEN_HPP

#include <string>
#include <utility>

namespace log_surgeon::query_parser {
/**
 * Represents static-text in the query as a token.
 *
 * Stores the raw log as a string.
 */
class StaticQueryToken {
public:
    explicit StaticQueryToken(std::string query_substring)
            : m_query_substring(std::move(query_substring)) {}

    auto operator==(StaticQueryToken const& rhs) const -> bool = default;

    auto operator!=(StaticQueryToken const& rhs) const -> bool = default;

    auto operator<(StaticQueryToken const& rhs) const -> bool {
        return m_query_substring < rhs.m_query_substring;
    }

    auto operator>(StaticQueryToken const& rhs) const -> bool {
        return m_query_substring > rhs.m_query_substring;
    }

    auto append(StaticQueryToken const& rhs) -> void {
        m_query_substring += rhs.get_query_substring();
    }

    [[nodiscard]] auto get_query_substring() const -> std::string const& {
        return m_query_substring;
    }

private:
    std::string m_query_substring;
};
}  // namespace log_surgeon::query_parser

#endif  // LOG_SURGEON_QUERY_PARSER_STATIC_QUERY_TOKEN_HPP
