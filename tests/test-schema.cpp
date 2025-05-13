#include <string>
#include <string_view>

#include <catch2/catch_test_macros.hpp>
#include <fmt/core.h>

#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using std::string;
using std::string_view;

using RegexASTCatByte
        = log_surgeon::finite_automata::RegexASTCat<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTCaptureByte
        = log_surgeon::finite_automata::RegexASTCapture<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTGroupByte
        = log_surgeon::finite_automata::RegexASTGroup<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTLiteralByte
        = log_surgeon::finite_automata::RegexASTLiteral<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTMultiplicationByte = log_surgeon::finite_automata::RegexASTMultiplication<
        log_surgeon::finite_automata::ByteNfaState>;

TEST_CASE("Test the Schema class", "[Schema]") {
    SECTION("Add a number variable to schema") {
        Schema schema;
        string const var_name = "myNumber";
        string const var_schema = var_name + string(":") + string("123");
        schema.add_variable(string_view(var_schema), -1);

        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars.at(0);
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        REQUIRE_NOTHROW([&]() {
            (void)dynamic_cast<RegexASTCatByte&>(*schema_var_ast.m_regex_ptr);
        }());
    }

    SECTION("Add a capture variable to schema") {
        Schema schema;
        std::string const var_name = "capture";
        string const var_schema = var_name + string(":") + string("u(?<uID>[0-9]+)");
        schema.add_variable(var_schema, -1);

        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr = schema_ast->m_schema_vars.at(0);
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        auto const* regex_ast_cat_ptr
                = dynamic_cast<RegexASTCatByte*>(schema_var_ast.m_regex_ptr.get());
        REQUIRE(nullptr != regex_ast_cat_ptr);
        REQUIRE(nullptr != regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_cat_ptr->get_right());

        auto const* regex_ast_literal
                = dynamic_cast<RegexASTLiteralByte const*>(regex_ast_cat_ptr->get_left());
        REQUIRE(nullptr != regex_ast_literal);
        REQUIRE('u' == regex_ast_literal->get_character());

        auto const* regex_ast_capture
                = dynamic_cast<RegexASTCaptureByte const*>(regex_ast_cat_ptr->get_right());
        REQUIRE(nullptr != regex_ast_capture);
        REQUIRE("uID" == regex_ast_capture->get_capture_name());

        auto const* regex_ast_multiplication_ast = dynamic_cast<RegexASTMultiplicationByte*>(
                regex_ast_capture->get_capture_regex_ast().get()
        );
        REQUIRE(nullptr != regex_ast_multiplication_ast);
        REQUIRE(1 == regex_ast_multiplication_ast->get_min());
        REQUIRE(0 == regex_ast_multiplication_ast->get_max());
        REQUIRE(regex_ast_multiplication_ast->is_infinite());

        auto const* regex_ast_group_ast
                = dynamic_cast<RegexASTGroupByte*>(regex_ast_multiplication_ast->get_operand().get()
                );
        REQUIRE(false == regex_ast_group_ast->is_wildcard());
        REQUIRE(1 == regex_ast_group_ast->get_ranges().size());
        REQUIRE('0' == regex_ast_group_ast->get_ranges().at(0).first);
        REQUIRE('9' == regex_ast_group_ast->get_ranges().at(0).second);
    }
}
