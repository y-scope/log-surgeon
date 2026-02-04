#include "VariableQueryToken.hpp"

#include <compare>

using std::strong_ordering;

namespace log_surgeon::wildcard_query_parser {
auto VariableQueryToken::operator<=>(VariableQueryToken const& rhs) const -> strong_ordering {
    auto const variable_type_cmp{m_variable_type <=> rhs.m_variable_type};
    if (std::strong_ordering::equal != variable_type_cmp) {
        return variable_type_cmp;
    }

    auto const query_substring_cmp{m_query_substring <=> rhs.m_query_substring};
    if (std::strong_ordering::equal != query_substring_cmp) {
        return query_substring_cmp;
    }

    // bool does not have a <=> operator, so we have to manually order it:
    auto const wildcard_cmp{
            static_cast<int>(m_contains_wildcard) <=> static_cast<int>(rhs.m_contains_wildcard)
    };
    if (std::strong_ordering::equal != wildcard_cmp) {
        return wildcard_cmp;
    }

    return static_cast<int>(m_contains_captures) <=> static_cast<int>(rhs.m_contains_captures);
}
}  // namespace log_surgeon::wildcard_query_parser
