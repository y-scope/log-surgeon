#include "Query.hpp"

#include <cstddef>
#include <cstdint>
#include <iterator>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include <log_surgeon/finite_automata/Dfa.hpp>
#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>
#include <log_surgeon/wildcard_query_parser/Expression.hpp>
#include <log_surgeon/wildcard_query_parser/ExpressionView.hpp>
#include <log_surgeon/wildcard_query_parser/QueryInterpretation.hpp>

using log_surgeon::finite_automata::ByteDfaState;
using log_surgeon::finite_automata::ByteNfaState;
using log_surgeon::lexers::ByteLexer;
using std::set;
using std::string;
using std::vector;

using ByteDfa = log_surgeon::finite_automata::Dfa<ByteDfaState, ByteNfaState>;
using ByteLexicalRule = log_surgeon::LexicalRule<ByteNfaState>;
using ByteNfa = log_surgeon::finite_automata::Nfa<ByteNfaState>;

namespace log_surgeon::wildcard_query_parser {
Query::Query(string const& query_string) {
    m_processed_query_string.reserve(query_string.size());
    Expression const expression(query_string);

    bool prev_is_greedy_wildcard{false};
    for (auto const& c : expression.get_chars()) {
        if (c.is_greedy_wildcard()) {
            if (false == prev_is_greedy_wildcard) {
                m_processed_query_string.push_back(c.value());
            }
            prev_is_greedy_wildcard = true;
            continue;
        }
        m_processed_query_string.push_back(c.value());
        prev_is_greedy_wildcard = false;
    }
}

auto Query::get_all_multi_token_interpretations(ByteLexer const& lexer) const
        -> std::set<QueryInterpretation> {
    if (m_processed_query_string.empty()) {
        return {};
    }

    Expression const expression{m_processed_query_string};
    vector<set<QueryInterpretation>> query_interpretations(expression.length());
    for (size_t end_idx = 1; end_idx <= expression.length(); ++end_idx) {
        for (size_t begin_idx = 0; begin_idx < end_idx; ++begin_idx) {
            ExpressionView const expression_view{expression, begin_idx, end_idx};
            if ("*" != expression_view.get_search_string()
                && expression_view.starts_or_ends_with_greedy_wildcard())
            {
                continue;
            }

            auto const single_token_interpretations{
                    get_all_single_token_interpretations(expression_view, lexer)
            };
            if (single_token_interpretations.empty()) {
                continue;
            }

            if (begin_idx == 0) {
                query_interpretations[end_idx - 1].insert(
                        std::make_move_iterator(single_token_interpretations.begin()),
                        std::make_move_iterator(single_token_interpretations.end())
                );
            } else {
                for (auto const& prefix : query_interpretations[begin_idx - 1]) {
                    for (auto const& suffix : single_token_interpretations) {
                        QueryInterpretation combined{prefix};
                        combined.append_query_interpretation(suffix);
                        query_interpretations[end_idx - 1].insert(std::move(combined));
                    }
                }
            }
        }
    }
    return query_interpretations.back();
}

auto Query::get_all_single_token_interpretations(
        ExpressionView const& original_view,
        ByteLexer const& lexer
) -> std::vector<QueryInterpretation> {
    vector<QueryInterpretation> interpretations;
    auto const extended_view{original_view.extend_to_adjacent_greedy_wildcards().second};

    if (false == original_view.is_well_formed()) {
        return interpretations;
    }
    if ("*" == original_view.get_search_string()) {
        interpretations.emplace_back("*");
        return interpretations;
    }
    if (false == original_view.is_surrounded_by_delims_or_wildcards(lexer.get_delim_table())) {
        interpretations.emplace_back(string{extended_view.get_search_string()});
        return interpretations;
    }

    auto const [regex_string, contains_wildcard]{extended_view.generate_regex_string()};

    auto const matching_var_type_ids{get_matching_variable_types(regex_string, lexer)};
    if (matching_var_type_ids.empty() || contains_wildcard) {
        interpretations.emplace_back(string{extended_view.get_search_string()});
    }

    for (auto const variable_type_id : matching_var_type_ids) {
        bool const contains_captures{lexer.get_captures_from_rule_id(variable_type_id).has_value()};
        interpretations.emplace_back(
                variable_type_id,
                string{extended_view.get_search_string()},
                contains_wildcard,
                contains_captures
        );
        if (false == contains_wildcard) {
            break;
        }
    }
    return interpretations;
}

auto Query::get_matching_variable_types(string const& regex_string, ByteLexer const& lexer)
        -> set<uint32_t> {
    Schema schema;
    schema.add_variable("search:" + regex_string, -1);
    auto const schema_ast = schema.release_schema_ast_ptr();
    auto& rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    vector<ByteLexicalRule> rules;
    rules.emplace_back(0, std::move(rule_ast.m_regex_ptr));
    ByteNfa const nfa{rules};

    auto var_types = lexer.get_nfa()->get_intersect(&nfa);
    return var_types;
}
}  // namespace log_surgeon::wildcard_query_parser
