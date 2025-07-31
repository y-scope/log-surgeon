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

void QueryInterpretation::append_query_interpretation(QueryInterpretation& suffix) {
    if (suffix.m_tokens.empty()) {
        return;
    }
    if (m_tokens.empty()) {
        m_tokens = suffix.m_tokens;
        return;
    }

    auto& last_old_token = m_tokens.back();
    auto const& first_new_token = suffix.m_tokens[0];
    if (std::holds_alternative<StaticQueryToken>(last_old_token)
        && std::holds_alternative<StaticQueryToken>(first_new_token))
    {
        std::get<StaticQueryToken>(last_old_token)
                .append(std::get<StaticQueryToken>(first_new_token));
        m_tokens.insert(m_tokens.end(), suffix.m_tokens.begin() + 1, suffix.m_tokens.end());
    } else {
        m_tokens.insert(m_tokens.end(), suffix.m_tokens.begin(), suffix.m_tokens.end());
    }
}

auto QueryInterpretation::operator<(QueryInterpretation const& rhs) const -> bool {
    if (m_tokens.size() < rhs.m_tokens.size()) {
        return true;
    }
    if (m_tokens.size() > rhs.m_tokens.size()) {
        return false;
    }
    for (uint32_t i{0}; i < m_tokens.size(); ++i) {
        if (m_tokens[i] < rhs.m_tokens[i]) {
            return true;
        }
        if (m_tokens[i] > rhs.m_tokens[i]) {
            return false;
        }
    }
    return false;
}

auto QueryInterpretation::serialize() const -> string {
    vector<string> token_strings;
    vector<string> has_wildcard_strings;

    for (auto const& token : m_tokens) {
        if (std::holds_alternative<StaticQueryToken>(token)) {
            token_strings.emplace_back(std::get<StaticQueryToken>(token).get_query_substring());
            has_wildcard_strings.emplace_back("0");
        } else {
            auto const& var = std::get<VariableQueryToken>(token);
            token_strings.emplace_back(
                    fmt::format("<{}>({})", var.get_variable_type(), var.get_query_substring())
            );
            has_wildcard_strings.emplace_back(var.get_has_wildcard() ? "1" : "0");
        }
    }

    return fmt::format(
            "logtype='{}', has_wildcard='{}'",
            fmt::join(token_strings, ""),
            fmt::join(has_wildcard_strings, "")
    );
}
}  // namespace log_surgeon::query_parser
