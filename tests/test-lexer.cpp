#include <cstdint>
#include <numeric>
#include <ranges>
#include <string>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

using std::string;
using std::u32string;
using std::vector;

using RegexASTCatByte = log_surgeon::finite_automata::RegexASTCat<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTCaptureByte = log_surgeon::finite_automata::RegexASTCapture<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTGroupByte = log_surgeon::finite_automata::RegexASTGroup<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTLiteralByte = log_surgeon::finite_automata::RegexASTLiteral<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTMultiplicationByte = log_surgeon::finite_automata::RegexASTMultiplication<
        log_surgeon::finite_automata::RegexNFAByteState>;
using RegexASTOrByte
        = log_surgeon::finite_automata::RegexASTOr<log_surgeon::finite_automata::RegexNFAByteState>;
using log_surgeon::SchemaVarAST;

auto test_regex_ast(string const& regex, u32string const& expected_serialized_ast) -> void {
    log_surgeon::Schema schema;
    schema.add_variable("capture", regex, -1);
    auto const schema_ast = schema.release_schema_ast_ptr();
    auto const& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);

    auto u32_to_u8 = [](char32_t c) -> std::string {
        std::string u8;
        if (c <= 0x7F) {
            u8 += static_cast<char>(c);
        } else if (c <= 0x7FF) {
            u8 += static_cast<char>((c >> 6) | 0xC0);
            u8 += static_cast<char>((c & 0x3F) | 0x80);
        } else if (c <= 0xFFFF) {
            u8 += static_cast<char>((c >> 12) | 0xE0);
            u8 += static_cast<char>(((c >> 6) & 0x3F) | 0x80);
            u8 += static_cast<char>((c & 0x3F) | 0x80);
        } else {
            u8 += static_cast<char>((c >> 18) | 0xF0);
            u8 += static_cast<char>(((c >> 12) & 0x3F) | 0x80);
            u8 += static_cast<char>(((c >> 6) & 0x3F) | 0x80);
            u8 += static_cast<char>((c & 0x3F) | 0x80);
        }
        return u8;
    };

    auto const actual_u32string = capture_rule_ast.m_regex_ptr->serialize();
    auto const actual_string = fmt::format(
            "{}",
            fmt::join(actual_u32string | std::ranges::views::transform(u32_to_u8), "")
    );

    auto const expected_string = fmt::format(
            "{}",
            fmt::join(expected_serialized_ast | std::ranges::views::transform(u32_to_u8), "")
    );

    REQUIRE(actual_string == expected_string);
}

TEST_CASE("Test the Schema class", "[Schema]") {
    SECTION("Add a number variable to schema") {
        log_surgeon::Schema schema;
        string const var_name = "myNumber";
        schema.add_variable(var_name, "123", -1);
        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars[0];
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        REQUIRE_NOTHROW([&]() { dynamic_cast<RegexASTCatByte&>(*schema_var_ast.m_regex_ptr); }());
    }

    SECTION("Add a capture variable to schema") {
        log_surgeon::Schema schema;
        std::string const var_name = "capture";
        schema.add_variable(var_name, "u(?<uID>[0-9]+)", -1);
        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars[0];
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        auto* regex_ast_cat_ptr = dynamic_cast<RegexASTCatByte*>(schema_var_ast.m_regex_ptr.get());
        REQUIRE(nullptr != regex_ast_cat_ptr);
        REQUIRE(nullptr != regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_cat_ptr->get_right());

        auto* regex_ast_literal
                = dynamic_cast<RegexASTLiteralByte const*>(regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_literal);
        REQUIRE('u' == regex_ast_literal->get_character());

        auto* regex_ast_capture
                = dynamic_cast<RegexASTCaptureByte const*>(regex_ast_cat_ptr->get_right());
        REQUIRE(nullptr != regex_ast_capture);
        REQUIRE("uID" == regex_ast_capture->get_group_name());

        auto* regex_ast_multiplication_ast = dynamic_cast<RegexASTMultiplicationByte*>(
                regex_ast_capture->get_group_regex_ast().get()
        );
        REQUIRE(nullptr != regex_ast_multiplication_ast);
        REQUIRE(1 == regex_ast_multiplication_ast->get_min());
        REQUIRE(0 == regex_ast_multiplication_ast->get_max());
        REQUIRE(regex_ast_multiplication_ast->is_infinite());

        auto* regex_ast_group_ast
                = dynamic_cast<RegexASTGroupByte*>(regex_ast_multiplication_ast->get_operand().get()
                );
        REQUIRE(false == regex_ast_group_ast->is_wildcard());
        REQUIRE(1 == regex_ast_group_ast->get_ranges().size());
        REQUIRE('0' == regex_ast_group_ast->get_ranges()[0].first);
        REQUIRE('9' == regex_ast_group_ast->get_ranges()[0].second);
    }

    SECTION("Test AST with tags") {
        // This test validates the serialization of a regex AST with named capture groups. The
        // serialized output includes tags (<n> for positive matches, <~n> for negative matches) to
        // indicate which capture groups are matched or unmatched at each node.
        test_regex_ast(
                "Z|(A(?<letter>((?<letter1>(a)|(b))|(?<letter2>(c)|(d))))B(?<containerID>\\d+)C)",
                U"(Z<~0><~1><~2><~3>)|(A((((a)|(b))<0><~1>)|(((c)|(d))<1><~0>))<2>B([0-9]{1,inf})<"
                "3>C)"
        );
    }

    SECTION("Test reptition regex") {
        // Repetition without capture groups untagged and tagged AST are the same
        test_regex_ast("a{0,10}", U"()|(a{1,10})");
        test_regex_ast("a{5,10}", U"a{5,10}");
        test_regex_ast("a*", U"()|(a{1,inf})");
        test_regex_ast("a+", U"a{1,inf}");

        // Repetition with capture groups untagged and tagged AST are different
        test_regex_ast("(?<letter>a){0,10}", U"(<~0>)|((a)<0>{1,10})");
        test_regex_ast("(?<letter>a){5,10}", U"(a)<0>{5,10}");
        test_regex_ast("(?<letter>a)*", U"(<~0>)|((a)<0>{1,inf})");
        test_regex_ast("(?<letter>a)+", U"(a)<0>{1,inf}");

        // Capture group with repetition
        test_regex_ast("(?<letter>a{0,10})", U"(()|(a{1,10}))<0>");
    }
}
