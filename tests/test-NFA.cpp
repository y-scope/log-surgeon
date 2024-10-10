#include <cstdint>
#include <map>
#include <queue>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

using log_surgeon::cSizeOfByte;
using log_surgeon::finite_automata::RegexNFAByteState;
using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using std::queue;
using std::string;
using std::unordered_map;
using std::unordered_set;

using ByteLexicalRule = log_surgeon::LexicalRule<RegexNFAByteState>;
using ByteNFA = log_surgeon::finite_automata::RegexNFA<RegexNFAByteState>;
using RegexASTCatByte = log_surgeon::finite_automata::RegexASTCat<RegexNFAByteState>;
using RegexASTCaptureByte = log_surgeon::finite_automata::RegexASTCapture<RegexNFAByteState>;
using RegexASTGroupByte = log_surgeon::finite_automata::RegexASTGroup<RegexNFAByteState>;
using RegexASTLiteralByte = log_surgeon::finite_automata::RegexASTLiteral<RegexNFAByteState>;
using RegexASTMultiplicationByte
        = log_surgeon::finite_automata::RegexASTMultiplication<RegexNFAByteState>;
using RegexASTOrByte = log_surgeon::finite_automata::RegexASTOr<RegexNFAByteState>;

namespace {
/**
 * Add a destination state to the queue and set of visited states if it has not yet been visited.
 * @param dest_state
 * @param visited_states
 * @param state_queue
 */
auto add_to_queue_and_visited(
        RegexNFAByteState const* dest_state,
        queue<RegexNFAByteState const*>& state_queue,
        unordered_set<RegexNFAByteState const*>& visited_states
) -> void;

auto add_to_queue_and_visited(
        RegexNFAByteState const* dest_state,
        queue<RegexNFAByteState const*>& state_queue,
        unordered_set<RegexNFAByteState const*>& visited_states
) -> void {
    if (visited_states.insert(dest_state).second) {
        state_queue.push(dest_state);
    }
}
}  // namespace

TEST_CASE("Test NFA", "[NFA]") {
    Schema schema;
    string const var_name{"capture"};
    string const var_schema{
            var_name + ":"
            + "Z|(A(?<letter>((?<letter1>(a)|(b))|(?<letter2>(c)|(d))))B(?"
              "<containerID>\\d+)C)"
    };
    schema.add_variable(var_schema, -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    ByteNFA nfa;
    ByteLexicalRule rule{0, std::move(capture_rule_ast.m_regex_ptr)};
    rule.add_to_nfa(&nfa);

    std::queue<RegexNFAByteState const*> state_queue;
    std::unordered_set<RegexNFAByteState const*> visited_states;

    // Assign state IDs
    unordered_map<RegexNFAByteState const*, uint32_t> state_ids;
    auto const* root = nfa.get_root();
    state_queue.push(root);
    visited_states.insert(root);
    while (false == state_queue.empty()) {
        auto const* current_state = state_queue.front();
        state_queue.pop();
        state_ids.insert({current_state, state_ids.size()});
        for (uint32_t idx = 0; idx < cSizeOfByte; idx++) {
            for (auto const* dest_state : current_state->get_byte_transitions(idx)) {
                add_to_queue_and_visited(dest_state, state_queue, visited_states);
            }
        }
        for (auto const* dest_state : current_state->get_epsilon_transitions()) {
            add_to_queue_and_visited(dest_state, state_queue, visited_states);
        }
        for (auto const& positive_tagged_transition :
             current_state->get_positive_tagged_transitions())
        {
            add_to_queue_and_visited(
                    positive_tagged_transition.get_dest_state(),
                    state_queue,
                    visited_states
            );
        }
        for (auto const& negative_tagged_transition :
             current_state->get_negative_tagged_transitions())
        {
            add_to_queue_and_visited(
                    negative_tagged_transition.get_dest_state(),
                    state_queue,
                    visited_states
            );
        }
    }

    // Serialize NFA
    std::string serialized_nfa;
    state_queue.push(root);
    visited_states.clear();
    visited_states.insert(root);
    while (false == state_queue.empty()) {
        auto const* current_state = state_queue.front();
        state_queue.pop();
        serialized_nfa += std::to_string(state_ids.at(current_state)) + ":";
        if (current_state->is_accepting()) {
            serialized_nfa += "accepting_tag="
                              + std::to_string(current_state->get_matching_variable_id()) + ",";
        }
        serialized_nfa += "byte_transitions={";
        for (uint32_t idx = 0; idx < cSizeOfByte; idx++) {
            for (auto const* dest_state : current_state->get_byte_transitions(idx)) {
                serialized_nfa += std::string(1, static_cast<char>(idx)) + "-->"
                                  + std::to_string(state_ids.find(dest_state)->second) + ",";
                add_to_queue_and_visited(dest_state, state_queue, visited_states);
            }
        }
        serialized_nfa += "},epsilon_transitions={";
        for (auto const* dest_state : current_state->get_epsilon_transitions()) {
            serialized_nfa += std::to_string(state_ids.at(dest_state)) + ",";
            add_to_queue_and_visited(dest_state, state_queue, visited_states);
        }
        serialized_nfa += "},positive_tagged_transitions={";
        for (auto const& positive_tagged_transition :
             current_state->get_positive_tagged_transitions())
        {
            serialized_nfa
                    += std::to_string(state_ids.at(positive_tagged_transition.get_dest_state()));
            serialized_nfa += "[" + std::to_string(positive_tagged_transition.get_tag()) + "],";
            add_to_queue_and_visited(
                    positive_tagged_transition.get_dest_state(),
                    state_queue,
                    visited_states
            );
        }
        serialized_nfa += "},negative_tagged_transitions={";
        for (auto const& negative_tagged_transition :
             current_state->get_negative_tagged_transitions())
        {
            serialized_nfa
                    += std::to_string(state_ids.at(negative_tagged_transition.get_dest_state()));
            serialized_nfa += "[";
            for (auto const& tag : negative_tagged_transition.get_tags()) {
                serialized_nfa += std::to_string(tag) + ",";
            }
            serialized_nfa += "],";
            add_to_queue_and_visited(
                    negative_tagged_transition.get_dest_state(),
                    state_queue,
                    visited_states
            );
        }
        serialized_nfa += "}";
        serialized_nfa += "\n";
    }

    // Compare against expected output
    std::string expected_serialized_nfa = "0:byte_transitions={A-->1,Z-->2,},"
                                          "epsilon_transitions={},"
                                          "positive_tagged_transitions={},"
                                          "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "1:byte_transitions={a-->3,b-->3,c-->4,d-->4,},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "2:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={5[0,1,2,3,],}\n";
    expected_serialized_nfa += "3:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={6[0],},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "4:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={7[1],},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "5:accepting_tag=0,byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "6:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={8[1,],}\n";
    expected_serialized_nfa += "7:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={8[0,],}\n";
    expected_serialized_nfa += "8:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={9[2],},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "9:byte_transitions={B-->10,},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "10:byte_transitions={0-->11,1-->11,2-->11,3-->11,4-->11,5-->11,6-->"
                               "11,7-->11,8-->11,9-->11,},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "11:byte_transitions={0-->11,1-->11,2-->11,3-->11,4-->11,5-->11,6-->"
                               "11,7-->11,8-->11,9-->11,},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={12[3],},"
                               "negative_tagged_transitions={}\n";
    expected_serialized_nfa += "12:byte_transitions={C-->5,},"
                               "epsilon_transitions={},"
                               "positive_tagged_transitions={},"
                               "negative_tagged_transitions={}\n";

    // Compare expected and actual line-by-line
    std::stringstream ss_actual(serialized_nfa);
    std::stringstream ss_expected(expected_serialized_nfa);
    std::string actual_line;
    std::string expected_line;
    while (std::getline(ss_actual, actual_line) && std::getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    std::getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    std::getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}
