#include "QueryInterpretation.hpp"

#include <algorithm>
#include <compare>
#include <string>
#include <variant>
#include <vector>

#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>
#include <log_surgeon/wildcard_query_parser/VariableQueryToken.hpp>

#include <fmt/core.h>
#include <fmt/format.h>

using log_surgeon::lexers::ByteLexer;
using std::declval;
using std::lexicographical_compare_three_way;
using std::same_as;
using std::string;
using std::strong_ordering;
using std::variant;
using std::vector;
using std::weak_ordering;

namespace log_surgeon::wildcard_query_parser {
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

// Helper to ensure variant is strongly ordered.
template <typename T> struct IsStronglyOrderedVariant;

template <typename... Ts> struct IsStronglyOrderedVariant<variant<Ts...>> {
    static constexpr bool cValue{(same_as<decltype(declval<Ts>() <=> declval<Ts>()),strong_ordering>
        && ...)};
};

auto QueryInterpretation::operator<=>(QueryInterpretation const& rhs) const -> strong_ordering {
    // Make sure the variants types are strongly ordered.
    static_assert(
        IsStronglyOrderedVariant<decltype(m_tokens)::value_type>::cValue,
        "All variant types in `m_tokens` must have `operator<=>` returning `std::strong_ordering`."
    );

    // Can't return `<=>` directly as `variant` is weakly ordered regardless of its types.
    auto const tokens_weak_cmp{m_tokens <=> rhs.m_tokens};
    if (weak_ordering::less == tokens_weak_cmp) {
        return strong_ordering::less;
    }
    if (weak_ordering::greater == tokens_weak_cmp) {
        return strong_ordering::greater;
    }
    return strong_ordering::equal;
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
}  // namespace log_surgeon::wildcard_query_parser
