#include <codecvt>
#include <cstdint>
#include <locale>
#include <string>
#include <string_view>

#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

#include <catch2/catch_test_macros.hpp>

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

TEST_CASE("Test regex with capture groups", "[Regex]") {
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
            U"(Z<~letter1><~letter2><~letter><~containerID>)|("
                "A("
                    "(((a)|(b))<letter1><~letter2>)|"
                    "(((c)|(d))<letter2><~letter1>)"
                ")<letter>B("
                    "([0-9]){1,inf}"
                ")<containerID>C"
            ")"
            // clang-format on
    );
}

TEST_CASE("Test regex with repetition", "[Regex]") {
    // Repetition without capture groups untagged and tagged AST are the same
    test_regex_ast("capture:a{0,10}", U"()|((a){1,10})");
    test_regex_ast("capture:a{5,10}", U"(a){5,10}");
    test_regex_ast("capture:a*", U"()|((a){1,inf})");
    test_regex_ast("capture:a+", U"(a){1,inf}");
}

TEST_CASE("Test regex with repetition and captures", "[Regex]") {
    // Repetition with capture groups untagged and tagged AST are different
    test_regex_ast("capture:(?<letter>a){0,10}", U"(<~letter>)|(((a)<letter>){1,10})");
    test_regex_ast("capture:(?<letter>a{0,10})", U"(()|((a){1,10}))<letter>");
    test_regex_ast("capture:(?<letter>a){5,10}", U"((a)<letter>){5,10}");
    test_regex_ast("capture:(?<letter>a{5,10})", U"((a){5,10})<letter>");
    test_regex_ast("capture:(?<letter>a)*", U"(<~letter>)|(((a)<letter>){1,inf})");
    test_regex_ast("capture:(?<letter>a)+", U"((a)<letter>){1,inf}");
}

TEST_CASE("Test complex regex with repetition and captures", "[Regex]") {
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
            U"("
                U"(<~letterA><~letterB>)|(("
                    U"((a)<letterA><~letterB>)|"
                    U"((b)<letterB><~letterA>)"
                U"){1,inf})"
            U"<~letterC><~letterD>)|("
                U"(<~letterC><~letterD>)|(("
                    U"((c)<letterC><~letterD>)|"
                    U"((d)<letterD><~letterC>)"
                U"){1,10})"
            U"<~letterA><~letterB>)"
            // clang-format on
    );
}
