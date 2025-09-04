#include "QueryInterpretation.hpp"

#include <compare>
#include <string>
#include <utility>
#include <variant>
#include <vector>

#include <fmt/format.h>
#include <fmt/ranges.h>

#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>
#include <log_surgeon/wildcard_query_parser/VariableQueryToken.hpp>

using std::string;
using std::strong_ordering;
using std::vector;
using std::weak_ordering;

namespace log_surgeon::wildcard_query_parser {
auto QueryInterpretation::operator<=>(QueryInterpretation const& rhs) const -> strong_ordering {
    // `<=>` for a `variant` returns a `weak_ordering`. However, we statically assert the types used
    // in `m_tokens` have `<=>` which returns `strong_ordering`. Therefore, we can convert the
    // result of `<=>` between two `m_tokens` from `weak_ordering` to `strong_ordering`.
    auto const tokens_weak_cmp{m_tokens <=> rhs.m_tokens};
    if (weak_ordering::less == tokens_weak_cmp) {
        return strong_ordering::less;
    }
    if (weak_ordering::greater == tokens_weak_cmp) {
        return strong_ordering::greater;
    }
    return strong_ordering::equal;
}

void QueryInterpretation::append_query_interpretation(QueryInterpretation const& suffix) {
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

auto QueryInterpretation::append_static_token(std::string const& query_substring) -> void {
    if (query_substring.empty()) {
        return;
    }

    StaticQueryToken static_query_token(query_substring);
    if (m_tokens.empty()) {
        m_tokens.emplace_back(std::move(static_query_token));
        return;
    }

    auto& prev_token = m_tokens.back();
    if (std::holds_alternative<StaticQueryToken>(prev_token)) {
        std::get<StaticQueryToken>(prev_token).append(static_query_token);
    } else {
        m_tokens.emplace_back(std::move(static_query_token));
    }
}

auto QueryInterpretation::serialize() const -> string {
    vector<string> token_strings;
    vector<string> contains_wildcard_strings;

    for (auto const& token : m_tokens) {
        if (std::holds_alternative<StaticQueryToken>(token)) {
            token_strings.emplace_back(std::get<StaticQueryToken>(token).get_query_substring());
            contains_wildcard_strings.emplace_back("0");
        } else {
            auto const& var = std::get<VariableQueryToken>(token);
            token_strings.emplace_back(
                    fmt::format("<{}>({})", var.get_variable_type(), var.get_query_substring())
            );
            contains_wildcard_strings.emplace_back(var.get_contains_wildcard() ? "1" : "0");
        }
    }

    return fmt::format(
            "logtype='{}', contains_wildcard='{}'",
            fmt::join(token_strings, ""),
            fmt::join(contains_wildcard_strings, "")
    );
}
}  // namespace log_surgeon::wildcard_query_parser
