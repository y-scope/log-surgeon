#include "WildcardExpression.hpp"

#include <string>
#include <utility>

#include <log_surgeon/wildcard_query_parser/WildcardCharacter.hpp>

namespace log_surgeon::wildcard_query_parser {
WildcardExpression::WildcardExpression(std::string processed_search_string)
        : m_processed_search_string(std::move(processed_search_string)) {
    m_chars.reserve(m_processed_search_string.size());
    for (auto const c : m_processed_search_string) {
        auto type{WildcardCharacter::Type::Normal};
        if (m_chars.empty() || false == m_chars.back().is_escape()) {
            if ('*' == c) {
                type = WildcardCharacter::Type::GreedyWildcard;
            } else if ('?' == c) {
                type = WildcardCharacter::Type::NonGreedyWildcard;
            } else if ('\\' == c) {
                type = WildcardCharacter::Type::Escape;
            }
        }
        m_chars.emplace_back(c, type);
    }
}
}  // namespace log_surgeon::wildcard_query_parser
