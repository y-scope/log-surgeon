#include <cstdint>
#include <string>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

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

TEST_CASE("Test the Schema class", "[Schema]") {
    SECTION("Add a number variable to schema") {
        log_surgeon::Schema schema;
        std::string const var_name = "myNumber";
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
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_cat_ptr->get_left().get());
        REQUIRE(nullptr != regex_ast_literal);
        REQUIRE('u' == regex_ast_literal->get_character());

        auto* regex_ast_capture
                = dynamic_cast<RegexASTCaptureByte*>(regex_ast_cat_ptr->get_right().get());
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

    SECTION("Check if AST variable has capture group") {
        log_surgeon::Schema schema;
        schema.add_variable("number", "123", -1);
        schema.add_variable("capture", "user_id=(?<userID>[0-9]+)", -1);
        auto const schema_ast = schema.release_schema_ast_ptr();
        auto& number_var_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
        REQUIRE(false == number_var_ast.m_regex_ptr->has_capture_groups());
        auto& capture_var_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[1]);
        REQUIRE(capture_var_ast.m_regex_ptr->has_capture_groups());
    }

    SECTION("Test AST structure after adding tags to capture AST") {
        log_surgeon::Schema schema;
        schema.add_variable(
                "capture",
                "Z|(A(?<letter>((?<letter1>(a)|(b))|(?<letter2>(c)|(d))))B(?<containerID>\\d+)C)",
                -1
        );
        auto const schema_ast = schema.release_schema_ast_ptr();
        auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
        std::vector<uint32_t> all_tags;
        capture_rule_ast.m_regex_ptr->add_tags(all_tags);

        std::string expected_serialized_string
                = "(Z)|(A(?<letter>((?<letter1>(a)|(b)))|((?<letter2>(c)|"
                  "(d))))B(?<containerID>[0-9]{1,inf})C)";
        REQUIRE(capture_rule_ast.m_regex_ptr->serialize(false) == expected_serialized_string);

        std::string expected_serialized_string_with_tags
                = "(Z<~0><~1><~2><~3>)|(A((((a)|(b))<1><~2>)|(((c)|(d))<2><~1>))<0>B([0-9]{1,inf})<"
                  "3>C)";
        REQUIRE(capture_rule_ast.m_regex_ptr->serialize(true)
                == expected_serialized_string_with_tags);
    }
}
