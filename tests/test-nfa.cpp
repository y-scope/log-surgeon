#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/Nfa.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

using log_surgeon::finite_automata::ByteNfaState;
using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using std::string;
using std::stringstream;
using std::vector;

using ByteLexicalRule = log_surgeon::LexicalRule<ByteNfaState>;
using ByteNfa = log_surgeon::finite_automata::Nfa<ByteNfaState>;

namespace {
/**
 * Generates an NFA for the given `var_schema` string, then serializes the NFA and compares it with
 * `expected_serialized_nfa`.
 *
 * @param var_schema Variable schema from which to construct the NFA.
 * @param expected_serialized_nfa Expected serialized string representation of the NFA.
 */
auto test_nfa(string const& var_schema, string const& expected_serialized_nfa) -> void;

auto test_nfa(string const& var_schema, string const& expected_serialized_nfa) -> void {
    Schema schema;
    schema.add_variable(var_schema, -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    vector<ByteLexicalRule> rules;
    rules.emplace_back(0, std::move(capture_rule_ast.m_regex_ptr));
    ByteNfa const nfa{rules};

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
}  // namespace

TEST_CASE("Test simple NFA", "[NFA]") {
    string const var_schema{"capture:userID=(?<uid>123)"};
    string const expected_serialized_nfa{
            "0:byte_transitions={u-->1},spontaneous_transition={}\n"
            "1:byte_transitions={s-->2},spontaneous_transition={}\n"
            "2:byte_transitions={e-->3},spontaneous_transition={}\n"
            "3:byte_transitions={r-->4},spontaneous_transition={}\n"
            "4:byte_transitions={I-->5},spontaneous_transition={}\n"
            "5:byte_transitions={D-->6},spontaneous_transition={}\n"
            "6:byte_transitions={=-->7},spontaneous_transition={}\n"
            "7:byte_transitions={},spontaneous_transition={8[0p]}\n"
            "8:byte_transitions={1-->9},spontaneous_transition={}\n"
            "9:byte_transitions={2-->10},spontaneous_transition={}\n"
            "10:byte_transitions={3-->11},spontaneous_transition={}\n"
            "11:byte_transitions={},spontaneous_transition={12[1p]}\n"
            "12:accepting_tag=0,byte_transitions={},spontaneous_transition={}\n"
    };
    test_nfa(var_schema, expected_serialized_nfa);
}

TEST_CASE("Test Complex NFA", "[NFA]") {
    string const var_schema{"capture:Z|(A(?<letter>((?<letter1>(a)|(b))|(?<letter2>(c)|(d))))B(?"
                            "<containerID>\\d+)C)"};
    // tags: letter1(0,1), letter2(2,3), letter(4,5), containerID(6,7)
    string const expected_serialized_nfa{
            "0:byte_transitions={A-->1,Z-->2},spontaneous_transition={}\n"
            "1:byte_transitions={},spontaneous_transition={3[4p]}\n"
            "2:byte_transitions={},spontaneous_transition={4[0n,1n,2n,3n,4n,5n,6n,7n]}\n"
            "3:byte_transitions={},spontaneous_transition={5[0p],6[2p]}\n"
            "4:accepting_tag=0,byte_transitions={},spontaneous_transition={}\n"
            "5:byte_transitions={a-->7,b-->7},spontaneous_transition={}\n"
            "6:byte_transitions={c-->8,d-->8},spontaneous_transition={}\n"
            "7:byte_transitions={},spontaneous_transition={9[1p]}\n"
            "8:byte_transitions={},spontaneous_transition={10[3p]}\n"
            "9:byte_transitions={},spontaneous_transition={11[2n,3n]}\n"
            "10:byte_transitions={},spontaneous_transition={11[0n,1n]}\n"
            "11:byte_transitions={},spontaneous_transition={12[5p]}\n"
            "12:byte_transitions={B-->13},spontaneous_transition={}\n"
            "13:byte_transitions={},spontaneous_transition={14[6p]}\n"
            "14:byte_transitions={0-->15,1-->15,2-->15,3-->15,4-->15,5-->15,6-->15,7-->15,8-->15,9-"
            "->15},spontaneous_transition={}\n"
            "15:byte_transitions={0-->15,1-->15,2-->15,3-->15,4-->15,5-->15,6-->15,7-->15,8-->15,9-"
            "->15},spontaneous_transition={16[7p]}\n"
            "16:byte_transitions={C-->4},spontaneous_transition={}\n"
    };
    test_nfa(var_schema, expected_serialized_nfa);
}

TEST_CASE("Test simple repetition NFA", "[NFA]") {
    string const var_schema{"capture:a*(?<one>1)+"};
    string const expected_serialized_nfa{
            "0:byte_transitions={a-->1},spontaneous_transition={1[]}\n"
            "1:byte_transitions={a-->1},spontaneous_transition={2[0p+]}\n"
            "2:byte_transitions={1-->3},spontaneous_transition={}\n"
            "3:byte_transitions={},spontaneous_transition={4[1p+]}\n"
            "4:accepting_tag=0,byte_transitions={},spontaneous_transition={5[0p+]}\n"
            "5:byte_transitions={1-->6},spontaneous_transition={}\n"
            "6:byte_transitions={},spontaneous_transition={4[1p+]}\n"
    };
    test_nfa(var_schema, expected_serialized_nfa);
}

TEST_CASE("Test complex repetition NFA", "[NFA]") {
    string const var_schema{"capture:(a*(?<one>1))+"};
    string const expected_serialized_nfa{
            "0:byte_transitions={a-->1},spontaneous_transition={1[]}\n"
            "1:byte_transitions={a-->1},spontaneous_transition={2[0p+]}\n"
            "2:byte_transitions={1-->3},spontaneous_transition={}\n"
            "3:byte_transitions={},spontaneous_transition={4[1p+]}\n"
            "4:accepting_tag=0,byte_transitions={a-->5},spontaneous_transition={5[]}\n"
            "5:byte_transitions={a-->5},spontaneous_transition={6[0p+]}\n"
            "6:byte_transitions={1-->7},spontaneous_transition={}\n"
            "7:byte_transitions={},spontaneous_transition={4[1p+]}\n"
    };
    test_nfa(var_schema, expected_serialized_nfa);
}

TEST_CASE("Test more complex repetition NFA", "[NFA]") {
    string const var_schema{"capture:(a+=(?<val>1+),)+"};
    string const expected_serialized_nfa{
            "0:byte_transitions={a-->1},spontaneous_transition={}\n"
            "1:byte_transitions={=-->2,a-->1},spontaneous_transition={}\n"
            "2:byte_transitions={},spontaneous_transition={3[0p+]}\n"
            "3:byte_transitions={1-->4},spontaneous_transition={}\n"
            "4:byte_transitions={1-->4},spontaneous_transition={5[1p+]}\n"
            "5:byte_transitions={,-->6},spontaneous_transition={}\n"
            "6:accepting_tag=0,byte_transitions={a-->7},spontaneous_transition={}\n"
            "7:byte_transitions={=-->8,a-->7},spontaneous_transition={}\n"
            "8:byte_transitions={},spontaneous_transition={9[0p+]}\n"
            "9:byte_transitions={1-->10},spontaneous_transition={}\n"
            "10:byte_transitions={1-->10},spontaneous_transition={11[1p+]}\n"
            "11:byte_transitions={,-->6},spontaneous_transition={}\n"
    };
    test_nfa(var_schema, expected_serialized_nfa);
}

TEST_CASE("Test integer NFA", "[NFA]") {
    string const var_schema{"int:\\-{0,1}\\d+"};
    string const expected_serialized_nfa{
            "0:byte_transitions={--->1},spontaneous_transition={1[]}\n"
            "1:byte_transitions={0-->2,1-->2,2-->2,3-->2,4-->2,5-->2,6-->2,7-->2,8-->2,9-->2},"
            "spontaneous_transition={}\n"
            "2:accepting_tag=0,byte_transitions={0-->2,1-->2,2-->2,3-->2,4-->2,5-->2,6-->2,7-->2,8-"
            "->2,9-->2},spontaneous_transition={}\n"
    };
    test_nfa(var_schema, expected_serialized_nfa);
}

TEST_CASE("Test key-value pair, value capture NFA", "[NFA]") {
    string const var_schema{R"(keyValuePair:[A]+=(?<val>[=AB]*A[=AB]*))"};
    string const expected_serialized_nfa{
            "0:byte_transitions={A-->1},spontaneous_transition={}\n"
            "1:byte_transitions={=-->2,A-->1},spontaneous_transition={}\n"
            "2:byte_transitions={},spontaneous_transition={3[0p]}\n"
            "3:byte_transitions={=-->4,A-->4,B-->4},spontaneous_transition={4[]}\n"
            "4:byte_transitions={=-->4,A-->4,A-->5,B-->4},spontaneous_transition={}\n"
            "5:byte_transitions={=-->6,A-->6,B-->6},spontaneous_transition={6[]}\n"
            "6:byte_transitions={=-->6,A-->6,B-->6},spontaneous_transition={7[1p]}\n"
            "7:accepting_tag=0,byte_transitions={},spontaneous_transition={}\n"
    };
    test_nfa(var_schema, expected_serialized_nfa);
}
