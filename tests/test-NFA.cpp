#include <cstdint>
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
using std::string;
using std::stringstream;
using std::vector;

using ByteLexicalRule = log_surgeon::LexicalRule<RegexNFAByteState>;
using ByteNFA = log_surgeon::finite_automata::RegexNFA<RegexNFAByteState>;
using RegexASTCatByte = log_surgeon::finite_automata::RegexASTCat<RegexNFAByteState>;
using RegexASTCaptureByte = log_surgeon::finite_automata::RegexASTCapture<RegexNFAByteState>;
using RegexASTGroupByte = log_surgeon::finite_automata::RegexASTGroup<RegexNFAByteState>;
using RegexASTLiteralByte = log_surgeon::finite_automata::RegexASTLiteral<RegexNFAByteState>;
using RegexASTMultiplicationByte
        = log_surgeon::finite_automata::RegexASTMultiplication<RegexNFAByteState>;
using RegexASTOrByte = log_surgeon::finite_automata::RegexASTOr<RegexNFAByteState>;

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
    vector<ByteLexicalRule> rules;
    rules.emplace_back(0, std::move(capture_rule_ast.m_regex_ptr));
    ByteNFA const nfa{std::move(rules)};

    // Compare against expected output
    string expected_serialized_nfa = "0:byte_transitions={A-->1,Z-->2},"
                                     "epsilon_transitions={},"
                                     "positive_tagged_start_transitions={},"
                                     "positive_tagged_end_transitions={},"
                                     "negative_tagged_transition={}\n";
    expected_serialized_nfa += "1:byte_transitions={a-->3,b-->3,c-->4,d-->4},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa
            += "2:byte_transitions={},"
               "epsilon_transitions={},"
               "positive_tagged_start_transitions={},"
               "positive_tagged_end_transitions={},"
               "negative_tagged_transition={5[letter1,letter2,letter,containerID]}\n";
    expected_serialized_nfa += "3:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={6[letter1]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "4:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={7[letter2]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "5:accepting_tag=0,byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "6:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={8[letter2]}\n";
    expected_serialized_nfa += "7:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={8[letter1]}\n";
    expected_serialized_nfa += "8:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={9[letter]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "9:byte_transitions={B-->10},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "10:byte_transitions={0-->11,1-->11,2-->11,3-->11,4-->11,5-->11,6-->"
                               "11,7-->11,8-->11,9-->11},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "11:byte_transitions={0-->11,1-->11,2-->11,3-->11,4-->11,5-->11,6-->"
                               "11,7-->11,8-->11,9-->11},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={12[containerID]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "12:byte_transitions={C-->5},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";

    // Compare expected and actual line-by-line
    auto const actual_serialized_nfa = nfa.serialize();
    stringstream ss_actual{actual_serialized_nfa};
    stringstream ss_expected{expected_serialized_nfa};
    string actual_line;
    string expected_line;

    CAPTURE(actual_serialized_nfa);
    CAPTURE(expected_serialized_nfa);
    while (getline(ss_actual, actual_line) && getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}
