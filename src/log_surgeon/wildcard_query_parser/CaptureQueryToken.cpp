#include "CaptureQueryToken.hpp"

#include <compare>

using std::strong_ordering;

namespace log_surgeon::wildcard_query_parser {
auto CaptureQueryToken::operator<=>(CaptureQueryToken const& rhs) const -> strong_ordering {
    auto const variable_type_cmp{m_name <=> rhs.m_name};
    if (std::strong_ordering::equal != variable_type_cmp) {
        return variable_type_cmp;
    }

    auto const query_substring_cmp{m_query_substring <=> rhs.m_query_substring};
    if (std::strong_ordering::equal != query_substring_cmp) {
        return query_substring_cmp;
    }

    // bool does not have a <=> operator, so we have to manually order it:
    return static_cast<int>(m_contains_wildcard) <=> static_cast<int>(rhs.m_contains_wildcard);
}
}  // namespace log_surgeon::wildcard_query_parser
