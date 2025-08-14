#ifndef LOG_SURGEON_QUERY_PARSER_WILDCARD_CHARACTER_HPP
#define LOG_SURGEON_QUERY_PARSER_WILDCARD_CHARACTER_HPP

#include <cstdint>

namespace log_surgeon::wildcard_query_parser {
class WildcardCharacter {
public:
    WildcardCharacter(char const value, CharType const type) : m_value{value}, m_type{type} {}

    [[nodiscard]] auto value() const -> char { return m_value; }

    [[nodiscard]] auto is_greedy_wildcard() const -> bool {
        return CharType::GreedyWildcard == m_type;
    }

    [[nodiscard]] auto is_non_greedy_wildcard() const -> bool {
        return CharType::NonGreedyWildcard == m_type;
    }

    [[nodiscard]] auto is_escape() const -> bool { return CharType::Escape == m_type; }

private:
    enum class CharType : uint8_t {
        Normal,
        GreedyWildcard,
        NonGreedyWildcard,
        Escape
    };

    char m_value;
    CharType m_type;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_QUERY_PARSER_WILDCARD_CHARACTER_HPP
