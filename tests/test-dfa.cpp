#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/Dfa.hpp>
#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

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

TEST_CASE("Test Simple Untagged DFA", "[DFA]") {
    Schema schema;
    string const var_name{"capture"};
    string const var_schema{var_name + ":" + "userID=123"};
    schema.add_variable(var_schema, -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    vector<ByteLexicalRule> rules;
    rules.emplace_back(0, std::move(capture_rule_ast.m_regex_ptr));
    ByteNfa const nfa{rules};
    ByteDfa const dfa{nfa};

    string const expected_serialized_dfa{
            "0:byte_transitions={u-()->1}\n"
            "1:byte_transitions={s-()->2}\n"
            "2:byte_transitions={e-()->3}\n"
            "3:byte_transitions={r-()->4}\n"
            "4:byte_transitions={I-()->5}\n"
            "5:byte_transitions={D-()->6}\n"
            "6:byte_transitions={=-()->7}\n"
            "7:byte_transitions={1-()->8}\n"
            "8:byte_transitions={2-()->9}\n"
            "9:byte_transitions={3-()->10}\n"
            "10:accepting_tags={0},accepting_operations={},byte_transitions={}\n"
    };

    // Compare expected and actual line-by-line
    auto const optional_actual_serialized_dfa{dfa.serialize()};
    REQUIRE(optional_actual_serialized_dfa.has_value());
    auto const& actual_serialized_dfa{optional_actual_serialized_dfa.value()};
    stringstream ss_actual{actual_serialized_dfa};
    stringstream ss_expected{expected_serialized_dfa};
    string actual_line;
    string expected_line;

    CAPTURE(actual_serialized_dfa);
    CAPTURE(expected_serialized_dfa);
    while (getline(ss_actual, actual_line) && getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}

TEST_CASE("Test Complex Untagged DFA", "[DFA]") {
    Schema schema;
    string const var_name{"capture"};
    string const var_schema{var_name + ":" + "Z|(A[abcd]B\\d+C)"};
    schema.add_variable(var_schema, -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    vector<ByteLexicalRule> rules;
    rules.emplace_back(0, std::move(capture_rule_ast.m_regex_ptr));
    ByteNfa const nfa{rules};
    ByteDfa const dfa{nfa};

    string const expected_serialized_dfa{
            "0:byte_transitions={A-()->1,Z-()->2}\n"
            "1:byte_transitions={a-()->3,b-()->3,c-()->3,d-()->3}\n"
            "2:accepting_tags={0},accepting_operations={},byte_transitions={}\n"
            "3:byte_transitions={B-()->4}\n"
            "4:byte_transitions={0-()->5,1-()->5,2-()->5,3-()->5,4-()->5,5-()->5,6-()->5,7-()->5,"
            "8-()->5,9-()->5}\n"
            "5:byte_transitions={0-()->5,1-()->5,2-()->5,3-()->5,4-()->5,5-()->5,6-()->5,7-()->5,"
            "8-()->5,9-()->5,C-()->2}\n"
    };

    // Compare expected and actual line-by-line
    auto const optional_actual_serialized_dfa{dfa.serialize()};
    REQUIRE(optional_actual_serialized_dfa.has_value());
    auto const& actual_serialized_dfa{optional_actual_serialized_dfa.value()};
    stringstream ss_actual{actual_serialized_dfa};
    stringstream ss_expected{expected_serialized_dfa};
    string actual_line;
    string expected_line;

    CAPTURE(actual_serialized_dfa);
    CAPTURE(expected_serialized_dfa);
    while (getline(ss_actual, actual_line) && getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}

TEST_CASE("Test Simple Tagged DFA", "[DFA]") {
    Schema schema;
    string const var_name{"capture"};
    string const var_schema{var_name + ":" + "userID=(?<uID>123)"};

    schema.add_variable(var_schema, -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    vector<ByteLexicalRule> rules;
    rules.emplace_back(0, std::move(capture_rule_ast.m_regex_ptr));
    ByteNfa const nfa{rules};
    ByteDfa const dfa{nfa};

    string const expected_serialized_dfa{
            "0:byte_transitions={u-()->1}\n"
            "1:byte_transitions={s-()->2}\n"
            "2:byte_transitions={e-()->3}\n"
            "3:byte_transitions={r-()->4}\n"
            "4:byte_transitions={I-()->5}\n"
            "5:byte_transitions={D-()->6}\n"
            "6:byte_transitions={=-()->7}\n"
            "7:byte_transitions={1-(4p)->8}\n"
            "8:byte_transitions={2-()->9}\n"
            "9:byte_transitions={3-()->10}\n"
            "10:accepting_tags={0},accepting_operations={2c4,3p},byte_transitions={}\n"
    };

    // Compare expected and actual line-by-line
    auto const optional_actual_serialized_dfa{dfa.serialize()};
    REQUIRE(optional_actual_serialized_dfa.has_value());
    stringstream ss_actual{optional_actual_serialized_dfa.value()};
    stringstream ss_expected{expected_serialized_dfa};
    string actual_line;
    string expected_line;

    CAPTURE(optional_actual_serialized_dfa.value());
    CAPTURE(expected_serialized_dfa);
    while (getline(ss_actual, actual_line) && getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}

TEST_CASE("Test Complex Tagged DFA", "[DFA]") {
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

    string const expected_serialized_dfa{
            "0:byte_transitions={A-()->1,Z-()->2}\n"
            "1:byte_transitions={a-(16p,17p)->3,b-(16p,17p)->3,c-(18p,17p)->4,d-(18p,17p)->4}\n"
            "2:accepting_tags={0},accepting_operations={8n,9n,10n,11n,12n,13n,14n,15n},"
            "byte_transitions={}\n"
            "3:byte_transitions={B-(19p,20n,21n,22p)->5}\n"
            "4:byte_transitions={B-(16n,19n,21p,22p,20c18)->5}\n"
            "5:byte_transitions={0-(27p)->6,1-(27p)->6,2-(27p)->6,3-(27p)->6,4-(27p)->6,5-(27p)->6,"
            "6-(27p)->6,7-(27p)->6,8-(27p)->6,9-(27p)->6}\n"
            "6:byte_transitions={0-()->6,1-()->6,2-()->6,3-()->6,4-()->6,5-()->6,6-()->6,7-()->6,"
            "8-()->6,9-()->6,C-(28p)->7}\n"
            "7:accepting_tags={0},accepting_operations={8c16,9c19,10c20,11c21,12c17,13c22,14c27,"
            "15c28},byte_transitions={}\n"
    };

    // Compare expected and actual line-by-line
    auto const optional_actual_serialized_dfa{dfa.serialize()};
    REQUIRE(optional_actual_serialized_dfa.has_value());
    stringstream ss_actual{optional_actual_serialized_dfa.value()};
    stringstream ss_expected{expected_serialized_dfa};
    string actual_line;
    string expected_line;

    CAPTURE(optional_actual_serialized_dfa.value());
    CAPTURE(expected_serialized_dfa);
    while (getline(ss_actual, actual_line) && getline(ss_expected, expected_line)) {
        REQUIRE(actual_line == expected_line);
    }
    getline(ss_actual, actual_line);
    REQUIRE(actual_line.empty());
    getline(ss_expected, expected_line);
    REQUIRE(expected_line.empty());
}
