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
#include <log_surgeon/parser_types.hpp>
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
    Expression const expression(query_string);
    bool prev_is_escape{false};
    string unhandled_wildcard_sequence;
    bool unhandled_wildcard_sequence_contains_greedy_wildcard{false};
    for (auto c : expression.get_chars()) {
        if (false == unhandled_wildcard_sequence.empty() && false == c.is_greedy_wildcard()
            && false == c.is_non_greedy_wildcard())
        {
            if (unhandled_wildcard_sequence_contains_greedy_wildcard) {
                m_query_string.push_back('*');
            } else {
                m_query_string += unhandled_wildcard_sequence;
            }
            unhandled_wildcard_sequence.clear();
            unhandled_wildcard_sequence_contains_greedy_wildcard = false;
        }

        if (prev_is_escape) {
            m_query_string.push_back(c.value());
            prev_is_escape = false;
        } else if (c.is_escape()) {
            prev_is_escape = true;
            m_query_string.push_back(c.value());
        } else if (c.is_greedy_wildcard()) {
            unhandled_wildcard_sequence.push_back(c.value());
            unhandled_wildcard_sequence_contains_greedy_wildcard = true;
        } else if (c.is_non_greedy_wildcard()) {
            unhandled_wildcard_sequence.push_back(c.value());
        } else {
            m_query_string.push_back(c.value());
        }
    }
    if (false == unhandled_wildcard_sequence.empty()) {
        if (unhandled_wildcard_sequence_contains_greedy_wildcard) {
            m_query_string.push_back('*');
        } else {
            m_query_string += unhandled_wildcard_sequence;
        }
    }
}

auto Query::get_all_multi_token_interpretations(ByteLexer const& lexer) const
        -> std::set<QueryInterpretation> {
    Expression const expression{m_query_string};
    vector<set<QueryInterpretation>> query_interpretations(expression.length());

    for (size_t end_idx = 1; end_idx <= expression.length(); ++end_idx) {
        for (size_t begin_idx = 0; begin_idx < end_idx; ++begin_idx) {
            ExpressionView const expression_view{expression, begin_idx, end_idx};
            if (expression_view.starts_or_ends_with_greedy_wildcard()) {
                continue;
            }

            auto const extended_view{expression_view.extend_to_adjacent_greedy_wildcards().second};
            auto const single_token_interpretations{
                    get_all_single_token_interpretations(extended_view, lexer)
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
        ExpressionView const& expression_view,
        ByteLexer const& lexer
) -> std::vector<QueryInterpretation> {
    vector<QueryInterpretation> interpretations;

    if (false == expression_view.is_well_formed()) {
        return interpretations;
    }
    if ("*" == expression_view.get_search_string()) {
        interpretations.emplace_back("*");
        return interpretations;
    }
    if (false == expression_view.is_surrounded_by_delims_or_wildcards(lexer.get_delim_table())) {
        interpretations.emplace_back(string{expression_view.get_search_string()});
        return interpretations;
    }

    auto const [regex_string, contains_wildcard]{expression_view.generate_regex_string()};

    auto const matching_var_type_ids{get_matching_variable_types(regex_string, lexer)};
    if (matching_var_type_ids.empty() || contains_wildcard) {
        interpretations.emplace_back(string{expression_view.get_search_string()});
    }

    for (auto const variable_type_id : matching_var_type_ids) {
        interpretations.emplace_back(
                variable_type_id,
                string{expression_view.get_search_string()},
                contains_wildcard
        );
        if (false == contains_wildcard) {
            break;
        }
    }
    return interpretations;
}

auto Query::get_matching_variable_types(string const& regex_string, ByteLexer const& lexer)
        -> set<uint32_t> {
    NonTerminal::m_next_children_start = 0;

    Schema schema;
    schema.add_variable("search:" + regex_string, -1);
    auto const schema_ast = schema.release_schema_ast_ptr();
    auto& rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    vector<ByteLexicalRule> rules;
    rules.emplace_back(0, std::move(rule_ast.m_regex_ptr));
    // TODO: Optimize NFA creation.
    ByteNfa const nfa{rules};
    // TODO: Optimize DFA creation.
    ByteDfa const dfa{nfa};

    // TODO: Could optimize to use a forward/reverse lexer in a lot of cases.
    auto var_types = lexer.get_dfa()->get_intersect(&dfa);
    return var_types;
}
}  // namespace log_surgeon::wildcard_query_parser
