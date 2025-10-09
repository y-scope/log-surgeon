#ifndef LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_HPP
#define LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_HPP

#include <cstdint>
#include <set>
#include <string>
#include <vector>

#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/wildcard_query_parser/ExpressionView.hpp>
#include <log_surgeon/wildcard_query_parser/QueryInterpretation.hpp>

namespace log_surgeon::wildcard_query_parser {
class Query {
public:
    explicit Query(std::string const& query_string);

    /**
     * Generates all k-token interpretations of the n-character query string, where 1 <= k <= n.
     *
     * 1. Interpret each substring [a,b) as a single token (k=1).
     *    - Substrings adjacent to greedy wildcards (`*`) must be interpreted as if they include
     *      them. To implement this, `get_all_single_token_interpretations` extends all substrings
     *      to include adjacent greedy wildcards.
     *      - Example: consider query "a*b" and variable type `hasNum` ("\w*\d+\w*"):
     *        - Without extension:
     *          - "a" -> static-text
     *          - "b" -> static-text
     *          - "a*" -> <hasNum>(a*)
     *          - "*b" -> <hasNum>(*b)
     *        - Multi-token interpretations (via step 2 below):
     *          - {a*b},
     *          - {<hasNum>(a*)b},
     *          - {a<hasNum>(*b)}.
     *        - None of these match a string like "a1 c 1b", which has interpretation
     *          {<hasNum>(a1) c <hasNum>(1b)}. By interpreting "a" as "a*" and "b" as "*b", the '*'
     *          is preserved, allowing for interpretation {<hasNum>(a*)*<hasNum>(*b)}, which matches
     *          {<hasNum>(a1) c <hasNum>(1b)}.
     *      - Note: the extension only applies to greedy wildcards, not non-greedy wildcards (`?`),
     *        as "a?b" != "a??b".
     *    - Substrings of length >= 2 that begin or end with a greedy wildcard are skipped as they
     *      are redundant.
     *      - Example: in "a*b", substring [0,1) extends to "a*", therefore substring [0,2) "a*" is
     *        redundant. This avoids producing interpretation {<hasNum>(a*)b}, which is a subset of
     *        {<hasNum>(a*)*b}.
     *      - Note: The length >= 2 requirement avoids skipping 1-length greedy substrings ("*") as
     *        they are never redundant (i.e., no 0-length substring exists to extend).
     *
     * 2. Let I(a) be the set of all k-token interpretations of substring [0,a), where 1 <= k <= a.
     *    - Let T(a,b) be the set of all valid single-token interpretations of substring [a,b).
     *    - We can then compute I(a) recursively:
     *
     *        I(a) = T(0,a)
     *               U (I(1) x T(1,a))
     *               U (I(2) x T(2,a))
     *               ...
     *               U (I(a-1) x T(a-1,a))
     *
     *      where x denotes the cross product: all combinations of prefix interpretations from I(i)
     *      and suffix interpretations from T(i,a).
     *
     * 3. Use dynamic programming to compute I(n) efficiently:
     *    - Instead of generating all possible combinations naively, we store only unique
     *      interpretations by recursively building up the combinations as shown below.
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

    [[nodiscard]] auto get_processed_query_string() const -> std::string const& {
        return m_processed_query_string;
    }

private:
    /**
     * Generates all single-token interpretations for a given expression view using the provided
     * lexer.
     *
     * Adjacent greedy wildcards are treated as part of the view when applicable. This extended
     * treatment is referred to as the "extended view". This behavior ensures correctness in the
     * multi-token interpretation algorithm; see `get_all_multi_token_interpretations` for a
     * detailed explanation.
     *
     * A single-token interpretation can be either:
     * - A static token (literal text), or
     * - A variable token (e.g., int, float, hasNumber) as defined by the lexer's schema. Each
     *   matching variable type produces a unique interpretation.
     *
     * Rules:
     * - If the original view is malformed (e.g., has dangling escape characters):
     *   - No valid interpretations are returned.
     * - Else, if the original view is an isolated greedy wildcard "*":
     *   - The only interpretation is a static token for "*".
     * - Else, if the original view is not surrounded by delimiters or wildcards (cannot form a
     *   variable):
     *   - The only interpretation is a static token for the extended view.
     * - Else, if the extended view does not match any variable type:
     *   - The only interpretation is a static token for the extended view.
     * - Else, if the extended view contains a wildcard:
     *   - The interpretations include a static token for the extended view, plus a variable token
     *     for each type matching the extended view.
     * - Else:
     *   - The only interpretation is a variable token corresponding to the highest-priority
     *     variable type that matches the extended view.
     *
     * @param original_view The view to interpret.
     * @param lexer The lexer used to determine variable types and delimiters.
     * @return A vector of `QueryInterpretation` objects representing all valid single-token
     * interpretations for the given view.
     */
    [[nodiscard]] static auto get_all_single_token_interpretations(
            ExpressionView const& original_view,
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

    std::string m_processed_query_string;
};
}  // namespace log_surgeon::wildcard_query_parser

#endif  // LOG_SURGEON_WILDCARD_QUERY_PARSER_QUERY_HPP
