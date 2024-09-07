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
        REQUIRE(4 == all_tags.size());
        REQUIRE(capture_rule_ast.m_regex_ptr->has_capture_groups());

        // OR0(Z | CAT1)
        auto* regex_ast_or0 = dynamic_cast<RegexASTOrByte*>(capture_rule_ast.m_regex_ptr.get());
        REQUIRE(nullptr != regex_ast_or0);
        REQUIRE(nullptr != regex_ast_or0->get_left());
        REQUIRE(nullptr != regex_ast_or0->get_right());
        REQUIRE(regex_ast_or0->get_negative_tags().empty());

        // CAT1(CAT2 + C)
        auto* regex_ast_cat1 = dynamic_cast<RegexASTCatByte*>(regex_ast_or0->get_right().get());
        REQUIRE(nullptr != regex_ast_cat1);
        REQUIRE(nullptr != regex_ast_cat1->get_left());
        REQUIRE(nullptr != regex_ast_cat1->get_right());
        REQUIRE(regex_ast_cat1->get_negative_tags().empty());

        // LITERAL1(C)
        auto* regex_ast_literal1
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_cat1->get_right().get());
        REQUIRE(nullptr != regex_ast_literal1);
        REQUIRE('C' == regex_ast_literal1->get_character());
        REQUIRE(regex_ast_literal1->get_negative_tags().empty());

        // CAT2(CAT3 + CAPTURE1)
        auto* regex_ast_cat2 = dynamic_cast<RegexASTCatByte*>(regex_ast_cat1->get_left().get());
        REQUIRE(nullptr != regex_ast_cat2);
        REQUIRE(nullptr != regex_ast_cat2->get_left());
        REQUIRE(nullptr != regex_ast_cat2->get_right());
        REQUIRE(regex_ast_cat2->get_negative_tags().empty());

        // CAPTURE1(MULTIPLICATION1)
        auto* regex_ast_capture1
                = dynamic_cast<RegexASTCaptureByte*>(regex_ast_cat2->get_right().get());
        REQUIRE(nullptr != regex_ast_capture1);
        REQUIRE(nullptr != regex_ast_capture1->get_group_regex_ast());
        REQUIRE("containerID" == regex_ast_capture1->get_group_name());
        REQUIRE(3 == regex_ast_capture1->get_tag());
        REQUIRE(regex_ast_capture1->get_negative_tags().empty());

        // MULTIPLICATION1(GROUP1)
        auto* regex_ast_multiplication1 = dynamic_cast<RegexASTMultiplicationByte*>(
                regex_ast_capture1->get_group_regex_ast().get()
        );
        REQUIRE(nullptr != regex_ast_multiplication1);
        REQUIRE(nullptr != regex_ast_multiplication1->get_operand());
        REQUIRE(regex_ast_multiplication1->is_infinite());
        REQUIRE(1 == regex_ast_multiplication1->get_min());
        REQUIRE(regex_ast_multiplication1->get_negative_tags().empty());

        // GROUP1([0-9])
        auto* regex_ast_group1
                = dynamic_cast<RegexASTGroupByte*>(regex_ast_multiplication1->get_operand().get());
        REQUIRE(nullptr != regex_ast_group1);
        REQUIRE(false == regex_ast_group1->is_wildcard());
        REQUIRE(false == regex_ast_group1->get_negate());
        REQUIRE(regex_ast_group1->get_ranges().size() == 1);
        REQUIRE(regex_ast_group1->get_ranges()[0].first == '0');
        REQUIRE(regex_ast_group1->get_ranges()[0].second == '9');
        REQUIRE(regex_ast_group1->get_negative_tags().empty());

        // CAT3(CAT4 + B)
        auto* regex_ast_cat3 = dynamic_cast<RegexASTCatByte*>(regex_ast_cat2->get_left().get());
        REQUIRE(nullptr != regex_ast_cat3);
        REQUIRE(nullptr != regex_ast_cat3->get_left());
        REQUIRE(nullptr != regex_ast_cat3->get_right());
        REQUIRE(regex_ast_cat3->get_negative_tags().empty());

        // LITERAL2(B)
        auto* regex_ast_literal2
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_cat3->get_right().get());
        REQUIRE(nullptr != regex_ast_literal2);
        REQUIRE('B' == regex_ast_literal2->get_character());
        REQUIRE(regex_ast_literal2->get_negative_tags().empty());

        // CAT4(A + CAPTURE2)
        auto* regex_ast_cat4 = dynamic_cast<RegexASTCatByte*>(regex_ast_cat3->get_left().get());
        REQUIRE(nullptr != regex_ast_cat4);
        REQUIRE(nullptr != regex_ast_cat4->get_left());
        REQUIRE(nullptr != regex_ast_cat4->get_right());
        REQUIRE(regex_ast_cat4->get_negative_tags().empty());

        // CAPTURE2(OR1)
        auto* regex_ast_capture2
                = dynamic_cast<RegexASTCaptureByte*>(regex_ast_cat4->get_right().get());
        REQUIRE(nullptr != regex_ast_capture2);
        REQUIRE(nullptr != regex_ast_capture2->get_group_regex_ast());
        REQUIRE("letter" == regex_ast_capture2->get_group_name());
        REQUIRE(0 == regex_ast_capture2->get_tag());
        REQUIRE(regex_ast_capture2->get_negative_tags().empty());

        // OR1(CAPTURE4 | CAPTURE3)
        auto* regex_ast_or1
                = dynamic_cast<RegexASTOrByte*>(regex_ast_capture2->get_group_regex_ast().get());
        REQUIRE(nullptr != regex_ast_or1);
        REQUIRE(nullptr != regex_ast_or1->get_left());
        REQUIRE(nullptr != regex_ast_or1->get_right());
        REQUIRE(regex_ast_capture2->get_negative_tags().empty());

        // CAPTURE3(OR2)
        auto* regex_ast_capture3
                = dynamic_cast<RegexASTCaptureByte*>(regex_ast_or1->get_right().get());
        REQUIRE(nullptr != regex_ast_capture3);
        REQUIRE(nullptr != regex_ast_capture3->get_group_regex_ast());
        REQUIRE("letter2" == regex_ast_capture3->get_group_name());
        REQUIRE(2 == regex_ast_capture3->get_tag());
        REQUIRE(std::vector<uint32_t>{1} == regex_ast_capture3->get_negative_tags());

        // OR2(c | d)
        auto* regex_ast_or2
                = dynamic_cast<RegexASTOrByte*>(regex_ast_capture3->get_group_regex_ast().get());
        REQUIRE(nullptr != regex_ast_or2);
        REQUIRE(nullptr != regex_ast_or2->get_left());
        REQUIRE(nullptr != regex_ast_or2->get_right());
        REQUIRE(regex_ast_or2->get_negative_tags().empty());

        // LITERAL3(d)
        auto* regex_ast_literal3
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_or2->get_right().get());
        REQUIRE(nullptr != regex_ast_literal3);
        REQUIRE('d' == regex_ast_literal3->get_character());
        REQUIRE(regex_ast_literal3->get_negative_tags().empty());

        // LITERAL4(c)
        auto* regex_ast_literal4
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_or2->get_left().get());
        REQUIRE(nullptr != regex_ast_literal4);
        REQUIRE('c' == regex_ast_literal4->get_character());
        REQUIRE(regex_ast_literal4->get_negative_tags().empty());

        // CAPTURE4(OR3)
        auto* regex_ast_capture4
                = dynamic_cast<RegexASTCaptureByte*>(regex_ast_or1->get_left().get());
        REQUIRE(nullptr != regex_ast_capture4);
        REQUIRE(nullptr != regex_ast_capture4->get_group_regex_ast());
        REQUIRE("letter1" == regex_ast_capture4->get_group_name());
        REQUIRE(1 == regex_ast_capture4->get_tag());
        REQUIRE(std::vector<uint32_t>{2} == regex_ast_capture4->get_negative_tags());

        // OR3(a | b)
        auto* regex_ast_or3
                = dynamic_cast<RegexASTOrByte*>(regex_ast_capture4->get_group_regex_ast().get());
        REQUIRE(nullptr != regex_ast_or3);
        REQUIRE(nullptr != regex_ast_or3->get_left());
        REQUIRE(nullptr != regex_ast_or3->get_right());
        REQUIRE(regex_ast_or3->get_negative_tags().empty());

        // LITERAL5(b)
        auto* regex_ast_literal5
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_or3->get_right().get());
        REQUIRE(nullptr != regex_ast_literal5);
        REQUIRE('b' == regex_ast_literal5->get_character());
        REQUIRE(regex_ast_literal5->get_negative_tags().empty());

        // LITERAL6(a)
        auto* regex_ast_literal6
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_or3->get_left().get());
        REQUIRE(nullptr != regex_ast_literal6);
        REQUIRE('a' == regex_ast_literal6->get_character());
        REQUIRE(regex_ast_literal6->get_negative_tags().empty());

        // LITERAL7(A)
        auto* regex_ast_literal7
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_cat4->get_left().get());
        REQUIRE(nullptr != regex_ast_literal7);
        REQUIRE('A' == regex_ast_literal7->get_character());
        REQUIRE(regex_ast_literal7->get_negative_tags().empty());

        // LITERAL8(Z)
        auto* regex_ast_literal8
                = dynamic_cast<RegexASTLiteralByte*>(regex_ast_or0->get_left().get());
        REQUIRE(nullptr != regex_ast_literal8);
        REQUIRE('Z' == regex_ast_literal8->get_character());
        REQUIRE(std::vector<uint32_t>{0, 1, 2, 3} == regex_ast_literal8->get_negative_tags());

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
