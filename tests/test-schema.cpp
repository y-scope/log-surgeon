#include <string>
#include <string_view>
#include <vector>

#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_schema Schema unit tests.
 * @brief Schema related unit tests.
 *
 * These unit tests contain the `Schema` tag.
 */

using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using std::string;
using std::string_view;
using std::vector;

using RegexASTCatByte
        = log_surgeon::finite_automata::RegexASTCat<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTCaptureByte
        = log_surgeon::finite_automata::RegexASTCapture<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTGroupByte
        = log_surgeon::finite_automata::RegexASTGroup<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTLiteralByte
        = log_surgeon::finite_automata::RegexASTLiteral<log_surgeon::finite_automata::ByteNfaState>;
using RegexASTMultiplicationByte = log_surgeon::finite_automata::
        RegexASTMultiplication<log_surgeon::finite_automata::ByteNfaState>;

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding a number variable to a schema.
 */
TEST_CASE("add_number_var", "[Schema]") {
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

    REQUIRE_NOTHROW([&]() -> void {
        std::ignore = dynamic_cast<RegexASTCatByte&>(*schema_var_ast.m_regex_ptr);
    }());
}

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding a variable with a capture group.
 */
TEST_CASE("add_capture_var", "[Schema]") {
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
            = dynamic_cast<RegexASTGroupByte*>(regex_ast_multiplication_ast->get_operand().get());
    REQUIRE(false == regex_ast_group_ast->is_wildcard());
    REQUIRE(1 == regex_ast_group_ast->get_ranges().size());
    REQUIRE('0' == regex_ast_group_ast->get_ranges().at(0).first);
    REQUIRE('9' == regex_ast_group_ast->get_ranges().at(0).second);
}

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding different invalid delimiter strings.
 */
TEST_CASE("add_invalid_delims", "[Schema]") {
    constexpr string_view cInvalidDelimiterString0{"myVar:userID=123"};
    constexpr string_view cInvalidDelimiterString1{"Delimiter:userID=123"};
    constexpr string_view cInvalidDelimiterString2{""};

    Schema schema;
    REQUIRE_THROWS_AS(schema.add_delimiters(cInvalidDelimiterString0), std::invalid_argument);
    REQUIRE_THROWS_AS(schema.add_delimiters(cInvalidDelimiterString1), std::invalid_argument);
    REQUIRE_THROWS_AS(schema.add_delimiters(cInvalidDelimiterString2), std::runtime_error);
}

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding different invalid variables.
 */
TEST_CASE("add_invalid_vars", "[Schema]") {
    constexpr string_view cInvalidVarString0{"myVar:[userID=123"};
    constexpr string_view cInvalidVarString1{"userID=123"};
    constexpr string_view cInvalidVarString2{R"(delimiters: \t\r\n)"};
    constexpr string_view cInvalidVarString3{""};

    Schema schema;
    REQUIRE_THROWS_AS(schema.add_variable(cInvalidVarString0, -1), std::runtime_error);
    REQUIRE_THROWS_AS(schema.add_variable(cInvalidVarString1, -1), std::runtime_error);
    REQUIRE_THROWS_AS(schema.add_variable(cInvalidVarString2, -1), std::invalid_argument);
    REQUIRE_THROWS_AS(schema.add_variable(cInvalidVarString3, -1), std::runtime_error);
}

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding different invalid variable priorities.
 */
TEST_CASE("add_invalid_var_priorities", "[Schema]") {
    constexpr string_view cVarString1{"uId:userID=123"};
    constexpr string_view cVarString2{R"(int:-{0,1}\d+)"};
    constexpr string_view cVarString3{R"(float:-{0,1}\d+\.\d+)"};
    constexpr int32_t invalidPos1{3};
    constexpr int32_t invalidPos2{-2};

    Schema schema;
    schema.add_variable(cVarString1, 0);
    schema.add_variable(cVarString2, 1);
    REQUIRE_THROWS_AS(schema.add_variable(cVarString3, invalidPos1), std::invalid_argument);
    REQUIRE_THROWS_AS(schema.add_variable(cVarString3, invalidPos2), std::invalid_argument);
}

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding a variable and capture group name with an underscore.
 */
TEST_CASE("add_underscore_name", "[Schema]") {
    Schema schema;
    string const var_name = "var_name";
    string const cap_name = "cap_name";
    string const var_schema = var_name + string(":a(?<") + cap_name + string(">_)b");
    schema.add_variable(string_view(var_schema), -1);

    auto const schema_ast = schema.release_schema_ast_ptr();
    REQUIRE(schema_ast->m_schema_vars.size() == 1);
    REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

    auto& schema_var_ast_ptr = schema_ast->m_schema_vars.at(0);
    REQUIRE(nullptr != schema_var_ast_ptr);
    auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
    REQUIRE(var_name == schema_var_ast.m_name);

    REQUIRE_NOTHROW([&]() -> void {
        std::ignore = dynamic_cast<RegexASTCatByte&>(*schema_var_ast.m_regex_ptr);
    }());

    auto const captures{schema_var_ast.m_regex_ptr->get_subtree_positive_captures()};
    REQUIRE(captures.size() == 1);
    REQUIRE(cap_name == captures.at(0)->get_name());
}

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding non-unique capture group names.
 */
TEST_CASE("non_unique_capture_names", "[Schema]") {
    vector<string> const var_names{"var_name1", "var_name2", "var_name3"};
    string const cap_name{"cap_name"};

    for (auto const& var_name : var_names) {
        CAPTURE(var_name);
        Schema schema;
        string const var_schema{var_name + string(":a(?<") + cap_name + string(">_)b")};
        schema.add_variable(string_view(var_schema), -1);
        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

        auto& schema_var_ast_ptr{schema_ast->m_schema_vars.at(0)};
        REQUIRE(nullptr != schema_var_ast_ptr);
        auto& schema_var_ast = dynamic_cast<SchemaVarAST&>(*schema_var_ast_ptr);
        REQUIRE(var_name == schema_var_ast.m_name);

        REQUIRE_NOTHROW([&]() -> void {
            std::ignore = dynamic_cast<RegexASTCatByte&>(*schema_var_ast.m_regex_ptr);
        }());

        auto const captures{schema_var_ast.m_regex_ptr->get_subtree_positive_captures()};
        REQUIRE(captures.size() == 1);
        REQUIRE(cap_name == captures.at(0)->get_name());
    }
}

/**
 * @ingroup unit_tests_schema
 * @brief Create a schema, adding non-escaped hyphens.
 */
TEST_CASE("non_escaped_hyphens", "[Schema]") {
    vector<string> const vars{R"(v1:ID-(?<id>\d{4}))", R"(v2:\d{4}-\d{4}-\d{4})", R"(v3:[a\-z])"};

    Schema schema;
    schema.add_variable(string_view(vars[0]), -1);
    schema.add_variable(string_view(vars[1]), -1);
    schema.add_variable(string_view(vars[2]), -1);
    auto const schema_ast = schema.release_schema_ast_ptr();
    REQUIRE(schema_ast->m_schema_vars.size() == 3);
    REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());

    REQUIRE(nullptr != schema_ast->m_schema_vars.at(0));
    REQUIRE(nullptr != schema_ast->m_schema_vars.at(1));
    REQUIRE(nullptr != schema_ast->m_schema_vars.at(2));
    auto& schema_var_ast0 = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars.at(0));
    auto& schema_var_ast1 = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars.at(1));
    auto& schema_var_ast2 = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars.at(2));
    REQUIRE_NOTHROW([&]() -> void {
        std::ignore = dynamic_cast<RegexASTCatByte&>(*schema_var_ast0.m_regex_ptr);
    }());
    REQUIRE_NOTHROW([&]() -> void {
        std::ignore = dynamic_cast<RegexASTCatByte&>(*schema_var_ast1.m_regex_ptr);
    }());
    REQUIRE_NOTHROW([&]() -> void {
        std::ignore = dynamic_cast<RegexASTGroupByte&>(*schema_var_ast2.m_regex_ptr);
    }());
}
