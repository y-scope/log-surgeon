#include "QueryInterpretation.hpp"

#include <cstdint>
#include <string>
#include <utility>
#include <variant>

#include <log_surgeon/Lexer.hpp>

#include <fmt/format.h>

using log_surgeon::lexers::ByteLexer;
using std::string;
using std::vector;

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
    if (m_is_encoded != rhs.m_is_encoded) {
        return rhs.m_is_encoded;
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
    if (m_is_encoded != rhs.m_is_encoded) {
        return m_is_encoded;
    }
    return false;
}

void QueryInterpretation::append_logtype(QueryInterpretation& suffix) {
    auto const& first_new_token = suffix.m_logtype[0];
    if (auto& prev_token = m_logtype.back();
        false == m_logtype.empty() && std::holds_alternative<StaticQueryToken>(prev_token)
        && false == suffix.m_logtype.empty()
        && std::holds_alternative<StaticQueryToken>(first_new_token))
    {
        std::get<StaticQueryToken>(prev_token).append(std::get<StaticQueryToken>(first_new_token));
        m_logtype.insert(m_logtype.end(), suffix.m_logtype.begin() + 1, suffix.m_logtype.end());
    } else {
        m_logtype.insert(m_logtype.end(), suffix.m_logtype.begin(), suffix.m_logtype.end());
    }
}

auto QueryInterpretation::operator<(QueryInterpretation const& rhs) const -> bool {
    if (m_logtype.size() < rhs.m_logtype.size()) {
        return true;
    }
    if (m_logtype.size() > rhs.m_logtype.size()) {
        return false;
    }
    for (uint32_t i{0}; i < m_logtype.size(); ++i) {
        if (m_logtype[i] < rhs.m_logtype[i]) {
            return true;
        }
        if (m_logtype[i] > rhs.m_logtype[i]) {
            return false;
        }
    }
    return false;
}

auto QueryInterpretation::serialize() const -> string {
    vector<string> token_strings;
    vector<string> has_wildcard_strings;
    vector<string> is_encoded_strings;

    for (auto const& token : m_logtype) {
        if (std::holds_alternative<StaticQueryToken>(token)) {
            token_strings.emplace_back(std::get<StaticQueryToken>(token).get_query_substring());
            has_wildcard_strings.emplace_back("0");
            is_encoded_strings.emplace_back("0");
        } else {
            auto const& var = std::get<VariableQueryToken>(token);
            token_strings.emplace_back(
                    fmt::format("<{}>({})", var.get_variable_type(), var.get_query_substring())
            );
            has_wildcard_strings.emplace_back(var.get_has_wildcard() ? "1" : "0");
            is_encoded_strings.emplace_back(var.get_is_encoded_with_wildcard() ? "1" : "0");
        }
    }

    return fmt::format(
            "logtype='{}', has_wildcard='{}', is_encoded_with_wildcard='{}'",
            fmt::join(token_strings, ""),
            fmt::join(has_wildcard_strings, ""),
            fmt::join(is_encoded_strings, "")
    );
}
}  // namespace log_surgeon::query_parser
