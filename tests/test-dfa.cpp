#include <cstdint>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Dfa.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

using log_surgeon::cSizeOfByte;
using log_surgeon::finite_automata::ByteNfaState;
using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using std::string;
using std::stringstream;
using std::vector;

using ByteLexicalRule = log_surgeon::LexicalRule<ByteNfaState>;
using ByteNfa = log_surgeon::finite_automata::Nfa<ByteNfaState>;
using RegexASTCatByte = log_surgeon::finite_automata::RegexASTCat<ByteNfaState>;
using RegexASTCaptureByte = log_surgeon::finite_automata::RegexASTCapture<ByteNfaState>;
using RegexASTGroupByte = log_surgeon::finite_automata::RegexASTGroup<ByteNfaState>;
using RegexASTLiteralByte = log_surgeon::finite_automata::RegexASTLiteral<ByteNfaState>;
using RegexASTMultiplicationByte
        = log_surgeon::finite_automata::RegexASTMultiplication<ByteNfaState>;
using RegexASTOrByte = log_surgeon::finite_automata::RegexASTOr<ByteNfaState>;

TEST_CASE("Test DFA", "[DFA]") {
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
    ByteNfa const nfa{rules};

    // Compare against expected output
    // capture order(tags in brackets): letter1(0,1), letter2(2,3), letter(4,5), containerID(6,7)
    string expected_serialized_nfa = "0:byte_transitions={A-->1,Z-->2},"
                                     "spontaneous_transition={}\n";
    expected_serialized_nfa += "1:byte_transitions={},"
                               "spontaneous_transition={3[set:4]}\n";
    expected_serialized_nfa += "2:byte_transitions={},"
                               "spontaneous_transition={4[negate:0,1,2,3,4,5,6,7]}\n";
    expected_serialized_nfa += "3:byte_transitions={},"
                               "spontaneous_transition={5[set:0],6[set:2]}\n";
    expected_serialized_nfa += "4:accepting_tag=0,byte_transitions={},"
                               "spontaneous_transition={}\n";
    expected_serialized_nfa += "5:byte_transitions={a-->7,b-->7},"
                               "spontaneous_transition={}\n";
    expected_serialized_nfa += "6:byte_transitions={c-->8,d-->8},"
                               "spontaneous_transition={}\n";
    expected_serialized_nfa += "7:byte_transitions={},"
                               "spontaneous_transition={9[set:1]}\n";
    expected_serialized_nfa += "8:byte_transitions={},"
                               "spontaneous_transition={10[set:3]}\n";
    expected_serialized_nfa += "9:byte_transitions={},"
                               "spontaneous_transition={11[negate:2,3]}\n";
    expected_serialized_nfa += "10:byte_transitions={},"
                               "spontaneous_transition={11[negate:0,1]}\n";
    expected_serialized_nfa += "11:byte_transitions={},"
                               "spontaneous_transition={12[set:5]}\n";
    expected_serialized_nfa += "12:byte_transitions={B-->13},"
                               "spontaneous_transition={}\n";
    expected_serialized_nfa += "13:byte_transitions={},"
                               "spontaneous_transition={14[set:6]}\n";
    expected_serialized_nfa += "14:byte_transitions={0-->15,1-->15,2-->15,3-->15,4-->15,5-->15,6-->"
                               "15,7-->15,8-->15,9-->15},"
                               "spontaneous_transition={}\n";
    expected_serialized_nfa += "15:byte_transitions={0-->15,1-->15,2-->15,3-->15,4-->15,5-->15,6-->"
                               "15,7-->15,8-->15,9-->15},"
                               "spontaneous_transition={16[set:7]}\n";
    expected_serialized_nfa += "16:byte_transitions={C-->4},"
                               "spontaneous_transition={}\n";

    // Compare expected and actual line-by-line
    auto const optional_actual_serialized_nfa = nfa.serialize();
    REQUIRE(optional_actual_serialized_nfa.has_value());
    stringstream ss_actual{optional_actual_serialized_nfa.value()};
    stringstream ss_expected{expected_serialized_nfa};
    string actual_line;
    string expected_line;

    CAPTURE(optional_actual_serialized_nfa.value());
    CAPTURE(expected_serialized_nfa);
    while (getline(ss_actual, actual_line) && getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}
