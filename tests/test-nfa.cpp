#include <cstdint>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/Constants.hpp>
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
    ByteNfa const nfa{rules};
    
    // Compare against expected output
    // capture order(tags in brackets): letter1(0,1), letter2(2,3), letter(4,5), containerID(6,7)
    string expected_serialized_nfa = "0:byte_transitions={A-->1,Z-->2},"
                                     "epsilon_transitions={},"
                                     "positive_tagged_start_transitions={},"
                                     "positive_tagged_end_transitions={},"
                                     "negative_tagged_transition={}\n";
    expected_serialized_nfa += "1:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={3[4]},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa
            += "2:byte_transitions={},"
               "epsilon_transitions={},"
               "positive_tagged_start_transitions={},"
               "positive_tagged_end_transitions={},"
               "negative_tagged_transition={4[0,1,2,3,4,5,6,7]}\n";
    expected_serialized_nfa += "3:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={5[0],6[2]},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "4:accepting_tag=0,byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "5:byte_transitions={a-->7,b-->7},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "6:byte_transitions={c-->8,d-->8},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "7:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={9[1]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "8:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={10[3]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "9:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={11[2,3]}\n";
    expected_serialized_nfa += "10:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={11[0,1]}\n";
    expected_serialized_nfa += "11:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={12[5]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "12:byte_transitions={B-->13},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "13:byte_transitions={},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={14[6]},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "14:byte_transitions={0-->15,1-->15,2-->15,3-->15,4-->15,5-->15,6-->"
                               "15,7-->15,8-->15,9-->15},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "15:byte_transitions={0-->15,1-->15,2-->15,3-->15,4-->15,5-->15,6-->"
                               "15,7-->15,8-->15,9-->15},"
                               "epsilon_transitions={},"
                               "positive_tagged_start_transitions={},"
                               "positive_tagged_end_transitions={16[7]},"
                               "negative_tagged_transition={}\n";
    expected_serialized_nfa += "16:byte_transitions={C-->4},"
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
