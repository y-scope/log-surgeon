#include <codecvt>
#include <cstdint>
#include <locale>
#include <string>
#include <string_view>

#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_regex_ast Regex AST unit tests.
 * @brief Capture related unit tests.

 * These unit tests contain the `Regex` tag.
 */

using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using std::codecvt_utf8;
using std::string;
using std::string_view;
using std::u32string;
using std::wstring_convert;

namespace {
/**
 * Generates an AST for the given `var_schema` string. Then serialize the AST and compare it with
 * the `expected_serialize_ast`.
 * @param var_schema
 * @param expected_serialized_ast
 */
auto test_regex_ast(string_view var_schema, u32string const& expected_serialized_ast) -> void;

/**
 * Converts the characters in a 32-byte unicode string into 4 bytes to generate a 8-byte unicode
 * string.
 * @param u32_str
 * @return The resulting utf8 string.
 */
[[nodiscard]] auto u32string_to_string(u32string const& u32_str) -> string;

auto test_regex_ast(string_view const var_schema, u32string const& expected_serialized_ast)
        -> void {
    Schema schema;
    schema.add_variable(var_schema, -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    auto const* capture_rule_ast
            = dynamic_cast<SchemaVarAST*>(schema_ast->m_schema_vars.at(0).get());
    REQUIRE(capture_rule_ast != nullptr);

    auto const actual_string = u32string_to_string(capture_rule_ast->m_regex_ptr->serialize());
    auto const expected_string = u32string_to_string(expected_serialized_ast);
    REQUIRE(actual_string == expected_string);
}

auto u32string_to_string(u32string const& u32_str) -> string {
    wstring_convert<codecvt_utf8<char32_t>, char32_t> converter;
    return converter.to_bytes(u32_str.data(), u32_str.data() + u32_str.size());
}
}  // namespace

/**
 * @ingroup unit_tests_regex_ast
 * @brief Create an AST from a regex with a capture group.
 */
TEST_CASE("capture", "[Regex]") {
    test_regex_ast(
            // clang-format off
            "capture:"
                "Z|("
                    "A(?<letter>("
                        "(?<letter1>(a)|(b))|"
                        "(?<letter2>(c)|(d))"
                    "))B("
                        "?<containerID>\\d+"
                    ")C"
                ")",
            U"((Z<~letter1><~letter2><~letter><~containerID>)|("
                "A(("
                    "((((a)|(b)))<letter1><~letter2>)|"
                    "((((c)|(d)))<letter2><~letter1>)"
                "))<letter>B("
                    "([0-9]){1,inf}"
                ")<containerID>C"
            "))"
            // clang-format on
    );
}

/**
 * @ingroup unit_tests_regex_ast
 * @brief Create ASTs from regexes with repetition.
 */
TEST_CASE("repetition", "[Regex]") {
    test_regex_ast("repetition:a{0,10}", U"(()|((a){1,10}))");
    test_regex_ast("repetition:a{5,10}", U"(a){5,10}");
    test_regex_ast("repetition:a*", U"(()|((a){1,inf}))");
    test_regex_ast("repetition:a+", U"(a){1,inf}");
}

/**
 * @ingroup unit_tests_regex_ast
 * @brief Create ASTs from simple regexes with a capture group containing repetition.
 */
TEST_CASE("capture_containing_repetition", "[Regex]") {
    test_regex_ast("capture:(?<letter>a{0,10})", U"((()|((a){1,10})))<letter>");
    test_regex_ast("capture:(?<letter>a{5,10})", U"((a){5,10})<letter>");
}

/**
 * @ingroup unit_tests_regex_ast
 * @brief Create ASTs from simple regexes with a multi-valued (repeated) capture group.
 */
TEST_CASE("multi_valued_capture_0", "[Regex]") {
    test_regex_ast("capture:(?<letter>a){0,10}", U"((<~letter>)|(((a)<letter>){1,10}))");
    test_regex_ast("capture:(?<letter>a){5,10}", U"((a)<letter>){5,10}");
    test_regex_ast("capture:(?<letter>a)*", U"((<~letter>)|(((a)<letter>){1,inf}))");
    test_regex_ast("capture:(?<letter>a)+", U"((a)<letter>){1,inf}");
}

/**
 * @ingroup unit_tests_regex_ast
 * @brief Create an AST from a complex regex with multi-valued (repeated) capture groups.
 */
TEST_CASE("multi_valued_capture_1", "[Regex]") {
    test_regex_ast(
            // clang-format off
            "capture:"
                "("
                    "("
                        "(?<letterA>a)|"
                        "(?<letterB>b)"
                    ")*"
                ")|("
                    "("
                        "(?<letterC>c)|"
                        "(?<letterD>d)"
                    "){0,10}"
                ")",
            U"((("
                "(<~letterA><~letterB>)|((("
                    "((a)<letterA><~letterB>)|"
                    "((b)<letterB><~letterA>)"
                ")){1,inf})"
            "<~letterC><~letterD>))|(("
                "(<~letterC><~letterD>)|((("
                    "((c)<letterC><~letterD>)|"
                    "((d)<letterD><~letterC>)"
                ")){1,10})"
            "<~letterA><~letterB>)))"
            // clang-format on
    );
}

/**
 * @ingroup unit_tests_regex_ast
 * @brief Test order of operations.
 */
TEST_CASE("order_of_operations", "[Regex]") {
    test_regex_ast("var:abc|def", U"((abc)|(def))");

    test_regex_ast("var:a|\\d+", U"((a)|(([0-9]){1,inf}))");
    test_regex_ast("var:a*|b+", U"(((()|((a){1,inf})))|((b){1,inf}))");

    test_regex_ast("var:(a|b)c", U"((a)|(b))c");
    test_regex_ast("var:(a|b)+c*", U"(((a)|(b))){1,inf}(()|((c){1,inf}))");

    test_regex_ast("var:a{2,5}|b", U"(((a){2,5})|(b))");
    test_regex_ast("var:(ab){1,3}|cd", U"(((ab){1,3})|(cd))");

    test_regex_ast("var:.\\d+", U"[*]([0-9]){1,inf}");
    test_regex_ast("var:.\\d+|cd", U"(([*]([0-9]){1,inf})|(cd))");

    test_regex_ast("var:a|b|c", U"((((a)|(b)))|(c))");
    test_regex_ast("var:(a|b)(c|d)", U"((a)|(b))((c)|(d))");
    test_regex_ast("var:(a|b)|(c|d)", U"((((a)|(b)))|(((c)|(d))))");

    test_regex_ast("var:a|(b|c)*", U"((a)|((()|((((b)|(c))){1,inf}))))");
    test_regex_ast("var:(a|b)+(c|d)*", U"(((a)|(b))){1,inf}(()|((((c)|(d))){1,inf}))");
    test_regex_ast("var:(a|b)c+|d*", U"((((a)|(b))(c){1,inf})|((()|((d){1,inf}))))");
}

/**
 * @ingroup unit_tests_regex_ast
 * @brief Test regex shorthands.
 */
TEST_CASE("regex_shorthands", "[Regex]") {
    test_regex_ast("var:\\d", U"[0-9]");
    test_regex_ast("var:\\D", U"[^0-9]");

    test_regex_ast("var:\\s", U"[ - ,\\t-\\t,\\r-\\r,\\n-\\n,\\v-\\v,\\f-\\f]");
    test_regex_ast("var:\\S", U"[^ - ,\\t-\\t,\\r-\\r,\\n-\\n,\\v-\\v,\\f-\\f]");

    test_regex_ast("var:\\w", U"[a-z,A-Z,0-9,_-_]");
    test_regex_ast("var:\\W", U"[^a-z,A-Z,0-9,_-_]");

    test_regex_ast("var:a?", U"(a){0,1}");
    test_regex_ast("var:a*", U"(()|((a){1,inf}))");
    test_regex_ast("var:a+", U"(a){1,inf}");
    test_regex_ast("var:a{0,5}", U"(()|((a){1,5}))");
    test_regex_ast("var:a{3,5}", U"(a){3,5}");
    test_regex_ast("var:a{3,0}", U"(a){3,inf}");
}
