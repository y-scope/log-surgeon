#include "WildcardExpression.hpp"

#include <algorithm>
#include <string>
#include <utility>

namespace log_surgeon::wildcard_query_parser {
WildcardExpression::WildcardExpression(std::string processed_search_string)
        : m_processed_search_string(std::move(processed_search_string)) {
    m_chars.reserve(m_processed_search_string.size());
    for (auto const& c : m_processed_search_string) {
        auto type{CharType::Normal};
        if (m_chars.empty() || false == m_chars.back().is_escape()) {
            if ('*' == c) {
                type = CharType::GreedyWildcard;
            } else if ('?' == c) {
                type = CharType::NonGreedyWildcard;
            } else if ('\\' == c) {
                type = CharType::Escape;
            }
        }
        m_chars.emplace_back(c, type);
    }
}
}  // namespace log_surgeon::wildcard_query_parser
