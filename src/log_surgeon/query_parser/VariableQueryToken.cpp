#include <log_surgeon/query_parser/VariableQueryToken.hpp>

namespace log_surgeon::query_parser {
auto VariableQueryToken::operator<(VariableQueryToken const& rhs) const -> bool {
    if (m_variable_type < rhs.m_variable_type) {
        return true;
    }
    if (m_variable_type > rhs.m_variable_type) {
        return false;
    }
    if (m_query_substring < rhs.m_query_substring) {
        return true;
    }
    if (m_query_substring > rhs.m_query_substring) {
        return false;
    }
    if (m_has_wildcard != rhs.m_has_wildcard) {
        return rhs.m_has_wildcard;
    }
    return false;
}

auto VariableQueryToken::operator>(VariableQueryToken const& rhs) const -> bool {
    if (m_variable_type > rhs.m_variable_type) {
        return true;
    }
    if (m_variable_type < rhs.m_variable_type) {
        return false;
    }
    if (m_query_substring > rhs.m_query_substring) {
        return true;
    }
    if (m_query_substring < rhs.m_query_substring) {
        return false;
    }
    if (m_has_wildcard != rhs.m_has_wildcard) {
        return m_has_wildcard;
    }
    return false;
}
}  // namespace log_surgeon::query_parser
