#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_EXPRESSION_CHARACTER_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_EXPRESSION_CHARACTER_HPP

#include <cstdint>

namespace log_surgeon::wildcard_query_parser {
class ExpressionCharacter {
public:
    enum class Type : uint8_t {
        Normal,
        GreedyWildcard,
        NonGreedyWildcard,
        Escape
    };

    ExpressionCharacter(char const value, Type const type) : m_value{value}, m_type{type} {}

    [[nodiscard]] auto value() const -> char { return m_value; }

    [[nodiscard]] auto is_greedy_wildcard() const -> bool { return Type::GreedyWildcard == m_type; }

    [[nodiscard]] auto is_non_greedy_wildcard() const -> bool {
        return Type::NonGreedyWildcard == m_type;
    }

    [[nodiscard]] auto is_escape() const -> bool { return Type::Escape == m_type; }

private:
    char m_value;
    Type m_type;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_EXPRESSION_CHARACTER_HPP
