#ifndef LOG_SURGEON_QUERY_PARSER_QUERY_INTERPRETATION_HPP
#define LOG_SURGEON_QUERY_PARSER_QUERY_INTERPRETATION_HPP

#include <cstdint>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

#include <log_surgeon/Lexer.hpp>

namespace log_surgeon::query_parser {
/**
 * Represents static-text in the query as a token.
 *
 * Stores the raw text as a string and provides comparison operations.
 */
class StaticQueryToken {
public:
    explicit StaticQueryToken(std::string query_substring)
            : m_query_substring(std::move(query_substring)) {}

    auto operator==(StaticQueryToken const& rhs) const -> bool = default;

    auto operator!=(StaticQueryToken const& rhs) const -> bool = default;

    auto operator<(StaticQueryToken const& rhs) const -> bool {
        return m_query_substring < rhs.m_query_substring;
    }

    auto operator>(StaticQueryToken const& rhs) const -> bool {
        return m_query_substring > rhs.m_query_substring;
    }

    auto append(StaticQueryToken const& rhs) -> void {
        m_query_substring += rhs.get_query_substring();
    }

    [[nodiscard]] auto get_query_substring() const -> std::string const& {
        return m_query_substring;
    }

private:
    std::string m_query_substring;
};

/**
 * Represents a variable in the query as a token.
 *
 * Stores the raw text as a string, as well as metadata specifying:
 * - if the variable contains a wildcard,
 * - the length of the variable.
 * Also provides comparison operations.
 */
class VariableQueryToken {
public:
    VariableQueryToken(
            uint32_t const variable_type,
            std::string query_substring,
            bool const has_wildcard,
            bool const is_encoded
    )
            : m_variable_type(variable_type),
              m_query_substring(std::move(query_substring)),
              m_has_wildcard(has_wildcard),
              m_is_encoded(is_encoded) {}

    auto operator==(VariableQueryToken const& rhs) const -> bool = default;

    auto operator!=(VariableQueryToken const& rhs) const -> bool = default;

    /**
     * Lexicographical less-than comparison.
     *
     * Compares member variables in the following order:
     * 1. `m_variable_type`
     * 2. `m_query_substring`
     * 3. `m_has_wildcard` (`false` < `true`)
     * 4. `m_is_encoded` (`false` < `true`)
     *
     * @param rhs The `VariableQueryToken` to compare against.
     * @return true if this object is considered less than rhs, false otherwise.
     */
    auto operator<(VariableQueryToken const& rhs) const -> bool;

    /**
     * Lexicographical greater-than comparison.
     *
     * Compares member variables in the following order:
     * 1. `m_variable_type`
     * 2. `m_query_substring`
     * 3. `m_has_wildcard` (`true` > `false`)
     * 4. `m_is_encoded` (`true` > `false`)
     *
     * @param rhs The `VariableQueryToken` to compare against.
     * @return true if this object is considered greater than rhs, false otherwise.
     */
    auto operator>(VariableQueryToken const& rhs) const -> bool;

    [[nodiscard]] auto get_variable_type() const -> uint32_t { return m_variable_type; }

    [[nodiscard]] auto get_query_substring() const -> std::string const& {
        return m_query_substring;
    }

    [[nodiscard]] auto get_has_wildcard() const -> bool { return m_has_wildcard; }

    [[nodiscard]] auto get_is_encoded_with_wildcard() const -> bool {
        return m_is_encoded && m_has_wildcard;
    }

private:
    uint32_t m_variable_type;
    std::string m_query_substring;
    bool m_has_wildcard{false};
    bool m_is_encoded{false};
};

/**
 * Represents a query as a sequence of static-text and variable tokens.
 *
 * The token sequence is stored in a canonicalized form - e.g., adjacent static tokens are merged -
 * to ensure a unique internal representation for accurate comparison.
 */
class QueryInterpretation {
public:
    QueryInterpretation() = default;

    explicit QueryInterpretation(std::string const& query_substring) {
        append_static_token(query_substring);
    }

    QueryInterpretation(
            uint32_t const variable_type,
            std::string query_substring,
            bool const contains_wildcard,
            bool const is_encoded
    ) {
        append_variable_token(
                variable_type,
                std::move(query_substring),
                contains_wildcard,
                is_encoded
        );
    }

    auto operator==(QueryInterpretation const& rhs) const -> bool {
        return m_logtype == rhs.m_logtype;
    }

    /**
     * Lexicographical less-than comparison.
     *
     * Comparison is performed in the following order:
     * 1. By number of tokens in the logtype (shorter logtypes are considered less).
     * 2. By lexicographical ordering of individual tokens (based on their `<` and `>` operators).
     *
     * @param rhs The `QueryInterpretation` to compare against.
     * @return true if this object is considered less than rhs, false otherwise.
     */
    auto operator<(QueryInterpretation const& rhs) const -> bool;

    auto clear() -> void { m_logtype.clear(); }

    /**
     * Appends the logtype of another `QueryInterpretation` to this one.
     *
     * If the last token in this logtype and the first token in the suffix are both
     * `StaticQueryToken`, they are merged to avoid unnecessary token boundaries. The merged token
     * replaces the last token of this logtype, and the remaining suffix tokens are appended as-is.
     *
     * This merging behavior ensures a canonical internal representation, which is essential for
     * maintaining consistent comparison semantics.
     *
     * @param suffix The `QueryInterpretation` to append.
     */
    auto append_logtype(QueryInterpretation& suffix) -> void;

    auto append_static_token(std::string const& query_substring) -> void {
        StaticQueryToken static_query_token(query_substring);
        if (auto& prev_token = m_logtype.back();
            false == m_logtype.empty() && std::holds_alternative<StaticQueryToken>(prev_token))
        {
            std::get<StaticQueryToken>(prev_token).append(static_query_token);
        } else {
            m_logtype.emplace_back(static_query_token);
        }
    }

    auto append_variable_token(
            uint32_t const variable_type,
            std::string query_substring,
            bool const contains_wildcard,
            bool const is_encoded
    ) -> void {
        m_logtype.emplace_back(VariableQueryToken(
                variable_type,
                std::move(query_substring),
                contains_wildcard,
                is_encoded
        ));
    }

    [[nodiscard]] auto get_logtype() const
            -> std::vector<std::variant<StaticQueryToken, VariableQueryToken>> {
        return m_logtype;
    }

    /**
     * @return A string representation of the QueryInterpretation.
     */
    [[nodiscard]] auto serialize() const -> std::string;

    static constexpr std::string_view cIntVarName = "int";
    static constexpr std::string_view cFloatVarName = "float";

private:
    std::vector<std::variant<StaticQueryToken, VariableQueryToken>> m_logtype;
};
}  // namespace log_surgeon::query_parser

#endif  // LOG_SURGEON_QUERY_PARSER_QUERY_INTERPRETATION_HPP
