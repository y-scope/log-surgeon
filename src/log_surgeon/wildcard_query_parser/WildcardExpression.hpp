#ifndef LOG_SURGEON_QUERY_PARSER_WILDCARD_EXPRESSION_HPP
#define LOG_SURGEON_QUERY_PARSER_WILDCARD_EXPRESSION_HPP

#include <string>
#include <vector>

#include <log_surgeon/wildcard_query_parser/WIldcardCharacter.hpp>

namespace log_surgeon::wildcard_query_parser {
/**
 * An expression for matching strings. The expression supports two types of wildcards:
 * - '*' matches zero or more characters
 * - '?' matches any single character
 *
 * To match a literal '*' or '?', the expression should escape it with a backslash (`\`).
 */
class WildcardExpression {
public:
    explicit WildcardExpression(std::string processed_search_string);

    [[nodiscard]] auto get_chars() const -> std::vector<WildcardCharacter> const& {
        return m_chars;
    }

    [[nodiscard]] auto get_string() const -> std::string const& {
        return m_processed_search_string;
    }

private:
    bool m_contains_wildcard;
    std::vector<WildcardCharacter> m_chars;
    std::string m_processed_search_string;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_QUERY_PARSER_WILDCARD_EXPRESSION_HPP
