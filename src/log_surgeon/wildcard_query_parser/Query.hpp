#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_HPP

#include <cstdint>
#include <set>
#include <string>
#include <vector>

#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/wildcard_query_parser/ExpressionView.hpp>
#include <log_surgeon/wildcard_query_parser/QueryInterpretation.hpp>

namespace log_surgeon::wildcard_query_parser {
class Query {
public:
    explicit Query(std::string const& query_string);

    /**
     * Generates all multi-token interpretations of the n-length query string (single-token
     * interpretations of the query string belong to this set):
     *
     * 1. Interpret each substring [a,b) as a single token (1-length interpretation).
     *    - Denote T(a,b) to be the set of all valid single-token interpretations of substring [a,b).
     *
     *    - Substrings adjacent to greedy wildcards must be interpreted as if they include them.
     *      - Example: query "a*b" is equivalent to "a***b". For a lexer with a `hasNum` variable
     *        type ("\w*\d*\w*"), without extensions, the only interpretations would be:
     *          {<static>(a*b)},
     *          {<hasNum>(a*) <static>(b)},
     *          {<static>(a) <hasNum>(*b)}.
     *        However, a string like "a1 abc 1b" is also matched by "a*b", and  requires the
     *        interpretation {<hasNum>(a*) <static>(*) <hasNum>(*b)}. Extension ensures such cases
     *        are captured.
     *      - Note: isolated greedy wildcard (`*`) are never extended as the `Query` collapses
     *        repeated greedy wildcards.
     *      - Note: non-greedy wildcards (`?`) are not extended as "a?b" is not equivalent to "a??b".
     *
     *    - Substrings that begin or end with a wildcard are skipped as they are redundant.
     *      - Example: in "a*b", substring (0,1] extends to "a*", therefore substring (0,2] "a*" is
     *        redundant. In other words, a decomposition like "a*" + "b"  is a subset of the more
     *        general "a*" + "*" + "*b".
     *
     * 2. Let I(a) be the set of all multi-length interpretations of substring [0,a).
     *    - We can compute I(a) recursively using previously computed sets:
     *
     *      I(a) = T(0,a)
     *             U (I(1) x T(1,a))
     *             U (I(2) x T(2,a))
     *             ...
     *             U (I(a-1) x T(a-1,a))
     *
     *      where x denotes the cross product: all combinations of prefix interpretations from I(i)
     *      and suffix interpretations from T(i,a).
     *
     * 3. Use dynamic programming to compute I(n) efficiently:
     *    - Instead of generating all possible combinations naively (O(2^n * k^n)), we store only
     *      unique interpretations, reducing complexity to roughly O(k^n), where k is the number of
     *      unique token types.
     *    - Compute I(n) iteratively in increasing order of substring length:
     *      - Compute T(0,1), then I(1)
     *      - Compute T(0,2), T(1,2), then I(2)
     *      - Compute T(0,3), T(1,3), T(2,3), then I(3)
     *      - ...
     *      - Compute T(0,n), ..., T(n-1,n), then I(n)
     *
     * @param lexer The lexer used to determine variable types and delimiters.
     * @return A set of `QueryInterpretation` representing all valid multi-token interpretations of
     * the full query string.
     */
    [[nodiscard]] auto get_all_multi_token_interpretations(lexers::ByteLexer const& lexer) const
            -> std::set<QueryInterpretation>;

private:
    /**
     * Generates all single-token interpretations for a given expression view matching a given lexer.
     *
     * A single-token interpretation can be one of:
     * - A static token (literal text).
     * - A variable token (e.g., int, float, hasNumber) as defined by the lexer's schema. Each
     * unique variable types is considered a distinct interpretation.
     *
     * Rules:
     * - If the substring is malformed (has hanging escape characters):
     *   - There are no valid interpretations.
     * - Else if the substring:
     *   - Is an isolated greedy wildcard, `*, or
     *   - Is not surrounded by delimiters or wildcards (lexer won't consider it a variable), or
     *   - Does not match any variable.
     * - Then:
     *   - The only interpretation is a static token.
     * - Else, if the substring contains a wildcard:
     *   - The interpretations include a static token, plus a variable token for each matching type.
     * - Else:
     *   - The only interpretation is the variable token corresponding to the highest priority match.
     *
     * @param expression_view The view of the substring to interpret.
     * @param lexer The lexer used to determine variable types and delimiters.
     * @return A vector of `Queryinterpretation` objects representing all valid single-token
     * interpretations for the given substring.
     */
    [[nodiscard]] static auto get_all_single_token_interpretations(
            ExpressionView const& expression_view,
            lexers::ByteLexer const& lexer
    ) -> std::vector<QueryInterpretation>;

    /**
     * Determines the set of variable types matched by the lexer for all strings generated from the
     * input regex.
     *
     * Generates a DFA from the input regex and computes its intersection with the lexer's DFA.
     *
     * @param regex_string The input regex string for which to find matching variable types.
     * @param lexer The lexer whose DFA is used for matching.
     * @return The set of all matching variable type IDs.
     */
    [[nodiscard]] static auto
    get_matching_variable_types(std::string const& regex_string, lexers::ByteLexer const& lexer)
            -> std::set<uint32_t>;

    std::string m_query_string;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_HPP
