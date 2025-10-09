#include "Expression.hpp"

#include <string>
#include <utility>

#include <log_surgeon/wildcard_query_parser/ExpressionCharacter.hpp>

namespace log_surgeon::wildcard_query_parser {
Expression::Expression(std::string search_string) : m_search_string(std::move(search_string)) {
    m_chars.reserve(m_search_string.size());
    for (auto const c : m_search_string) {
        auto type{ExpressionCharacter::Type::Normal};
        if (m_chars.empty() || false == m_chars.back().is_escape()) {
            if ('*' == c) {
                type = ExpressionCharacter::Type::GreedyWildcard;
            } else if ('?' == c) {
                type = ExpressionCharacter::Type::NonGreedyWildcard;
            } else if ('\\' == c) {
                type = ExpressionCharacter::Type::Escape;
            }
        }
        m_chars.emplace_back(c, type);
    }
}
}  // namespace log_surgeon::wildcard_query_parser
