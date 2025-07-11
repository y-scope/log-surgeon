#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include <log_surgeon/finite_automata/Dfa.hpp>
#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_dfa DFA unit tests.
 * @brief DFA related unit tests.

 * These unit tests contain the `DFA` tag.
 */

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

namespace {
/**
 * Generates a DFA for the given variable schemas, then serializes the DFA and compares it with
 * `expected_serialized_dfa`.
 *
 * @param var_schemas Vector of variable schemas from which to construct the DFA.
 * @param expected_serialized_dfa Expected serialized string representation of the DFA.
 */
auto test_dfa(std::vector<string> const& var_schemas, string const& expected_serialized_dfa)
        -> void;

auto test_dfa(std::vector<string> const& var_schemas, string const& expected_serialized_dfa)
        -> void {
    Schema schema;
    for (auto const& var_schema : var_schemas) {
        schema.add_variable(var_schema, -1);
    }
    auto const schema_ast = schema.release_schema_ast_ptr();
    vector<ByteLexicalRule> rules;
    for (size_t i{0}; i < var_schemas.size(); i++) {
        auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[i]);
        rules.emplace_back(i, std::move(capture_rule_ast.m_regex_ptr));
    }
    ByteNfa const nfa{rules};
    ByteDfa const dfa{nfa};

    // Compare expected and actual line-by-line
    auto const optional_actual_serialized_dfa = dfa.serialize();
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
}  // namespace

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching a simple variable with no capture group.
 */
TEST_CASE("no_capture_0", "[DFA]") {
    string const var_schema{"var:userID=123"};
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
    test_dfa({var_schema}, expected_serialized_dfa);
}

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching a complex variable with no capture group.
 */
TEST_CASE("no_capture_1", "[DFA]") {
    string const var_schema{"var:Z|(A[abcd]B\\d+C)"};
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
    test_dfa({var_schema}, expected_serialized_dfa);
}

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching a simple variable with a capture group.
 */
TEST_CASE("capture", "[DFA]") {
    string const var_schema{"capture:userID=(?<uID>123)"};
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
    test_dfa({var_schema}, expected_serialized_dfa);
}

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching a complex capture group containing repetition.
 */
TEST_CASE("capture_containing_repetition", "[DFA]") {
    string const var_schema{"capture:Z|(A(?<letter>((?<letter1>(a)|(b))|(?<letter2>(c)|(d))))B(?<"
                            "containerID>\\d+)C)"};
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
    test_dfa({var_schema}, expected_serialized_dfa);
}

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching a multi-valued (repeated) capture group containing repetition.
 */
TEST_CASE("multi_valued_capture_containing_repetition", "[DFA]") {
    string const var_schema{"capture:([a]+=(?<val>1+),)+"};
    string const expected_serialized_dfa{
            "0:byte_transitions={a-()->1}\n"
            "1:byte_transitions={=-()->2,a-()->1}\n"
            "2:byte_transitions={1-(4p)->3}\n"
            "3:byte_transitions={,-(5p)->4,1-()->3}\n"
            "4:accepting_tags={0},accepting_operations={2c4,3c5},byte_transitions={a-()->5}\n"
            "5:byte_transitions={=-()->6,a-()->5}\n"
            "6:byte_transitions={1-(6p)->7}\n"
            "7:byte_transitions={,-(5p,4c6)->4,1-()->7}\n"
    };
    test_dfa({var_schema}, expected_serialized_dfa);
}

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching an integer.
 */
TEST_CASE("int_var", "[DFA]") {
    string const var_schema{"int:\\-{0,1}\\d+"};
    string const expected_serialized_dfa{
            "0:byte_transitions={--()->1,0-()->2,1-()->2,2-()->2,3-()->2,4-()->2,5-()->2,6-()->2,7-"
            "()->2,8-()->2,9-()->2}\n"
            "1:byte_transitions={0-()->2,1-()->2,2-()->2,3-()->2,4-()->2,5-()->2,6-()->2,7-()->2,8-"
            "()->2,9-()->2}\n"
            "2:accepting_tags={0},accepting_operations={},byte_transitions={0-()->2,1-()->2,2-()->"
            "2,3-()->2,4-()->2,5-()->2,6-()->2,7-()->2,8-()->2,9-()->2}\n"
    };
    test_dfa({var_schema}, expected_serialized_dfa);
}

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching a key-value pair.
 */
TEST_CASE("kv_pair_var", "[DFA]") {
    string const var_schema{R"(keyValuePair:[A]+=(?<val>[=AB]*A[=AB]*))"};
    string const expected_serialized_dfa{
            "0:byte_transitions={A-()->1}\n"
            "1:byte_transitions={=-()->2,A-()->1}\n"
            "2:byte_transitions={=-(4p)->3,A-(4p)->4,B-(4p)->3}\n"
            "3:byte_transitions={=-()->3,A-()->4,B-()->3}\n"
            "4:accepting_tags={0},accepting_operations={2c4,3p},byte_transitions={=-()->5,A-()->4,"
            "B-()->5}\n"
            "5:accepting_tags={0},accepting_operations={2c4,3p},byte_transitions={=-()->5,A-()->4,"
            "B-()->5}\n"
    };
    test_dfa({var_schema}, expected_serialized_dfa);
}

/**
 * @ingroup unit_tests_dfa
 * @brief Create a DFA for matching two overlapping variables.
 */
TEST_CASE("two_overlapping_vars", "[DFA]") {
    string const var_schema1{R"(keyValuePair:[A]+=(?<val>[=AB]*A[=AB]*))"};
    string const var_schema2{R"(hasA:[AB]*[A][=AB]*)"};
    string const expected_serialized_dfa{
            "0:byte_transitions={A-()->1,B-()->2}\n"
            "1:accepting_tags={1},accepting_operations={2c0,3c1},byte_transitions={=-()->3,A-()->1,"
            "B-()->4}\n"
            "2:byte_transitions={A-()->5,B-()->2}\n"
            "3:accepting_tags={1},accepting_operations={2c0,3c1},byte_transitions={=-(4p)->6,"
            "A-(4p)->7,B-(4p)->6}\n"
            "4:accepting_tags={1},accepting_operations={2c0,3c1},byte_transitions={=-()->8,A-()->5,"
            "B-()->4}\n"
            "5:accepting_tags={1},accepting_operations={2c0,3c1},byte_transitions={=-()->8,A-()->5,"
            "B-()->4}\n"
            "6:accepting_tags={1},accepting_operations={2c0,3c1},byte_transitions={=-()->6,A-()->7,"
            "B-()->6}\n"
            "7:accepting_tags={0,1},accepting_operations={2c4,3p,2c0,3c1},"
            "byte_transitions={=-()->9,A-()->7,B-()->9}\n"
            "8:accepting_tags={1},accepting_operations={2c0,3c1},byte_transitions={=-()->8,A-()->8,"
            "B-()->8}\n"
            "9:accepting_tags={0,1},accepting_operations={2c4,3p,2c0,3c1},"
            "byte_transitions={=-()->9,A-()->7,B-()->9}\n"
    };
    test_dfa({var_schema1, var_schema2}, expected_serialized_dfa);
}
