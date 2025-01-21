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
using log_surgeon::finite_automata::ByteDfaState;
using log_surgeon::finite_automata::ByteNfaState;
using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using std::string;
using std::stringstream;
using std::vector;

using ByteDfa = log_surgeon::finite_automata::Dfa<ByteDfaState, ByteNfaState>;
using ByteLexicalRule = log_surgeon::LexicalRule<ByteNfaState>;
using ByteNfa = log_surgeon::finite_automata::Nfa<ByteNfaState>;

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
    ByteDfa const dfa{nfa};

    // Compare against expected output
    // capture order(tags in brackets): letter1(0,1), letter2(2,3), letter(4,5), containerID(6,7)
    string expected_serialized_nfa = "0:byte_transitions={A-()->1,Z-()->2}\n";
    expected_serialized_nfa += "1:byte_transitions={a-(16p,17p)->3,b-(16p,17p)->3,c-(17p,18p)->4,"
                               "d-(17p,18p)->4}\n";
    expected_serialized_nfa += "2:accepting_tags={0},byte_transitions={}\n";
    expected_serialized_nfa += "3:byte_transitions={B-(19p,20n,21n,22p)->5}\n";
    expected_serialized_nfa += "4:byte_transitions={B-(23n,24n,25p,26p)->5}\n";
    expected_serialized_nfa += "5:byte_transitions={0-(27p)->6,1-(27p)->6,2-(27p)->6,3-(27p)->6,"
                               "4-(27p)->6,5-(27p)->6,6-(27p)->6,7-(27p)->6,8-(27p)->6,"
                               "9-(27p)->6}\n";
    expected_serialized_nfa += "6:byte_transitions={0-()->6,1-()->6,2-()->6,3-()->6,4-()->6,5-()->6,"
                               "6-()->6,7-()->6,8-()->6,9-()->6,C-(28p)->7}\n";
    expected_serialized_nfa += "7:accepting_tags={0},byte_transitions={}\n";

    // Compare expected and actual line-by-line
    auto const actual_serialized_dfa{dfa.serialize()};
    stringstream ss_actual{actual_serialized_dfa};
    stringstream ss_expected{expected_serialized_nfa};
    string actual_line;
    string expected_line;

    CAPTURE(actual_serialized_dfa);
    CAPTURE(expected_serialized_nfa);
    while (getline(ss_actual, actual_line) && getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}
