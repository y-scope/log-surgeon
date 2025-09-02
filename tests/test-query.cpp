#include <cstddef>
#include <set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <log_surgeon/Lexer.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>
#include <log_surgeon/wildcard_query_parser/Query.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_query `Query` unit tests.
 * @brief Unit tests for `Query` construction and interpretation.

 * These unit tests contain the `Query` tag.
 */

using log_surgeon::lexers::ByteLexer;
using log_surgeon::Schema;
using log_surgeon::SchemaVarAST;
using log_surgeon::wildcard_query_parser::Query;
using std::set;
using std::string;
using std::string_view;
using std::vector;

namespace {
/**
 * Creates a query from the given query string and tests that its processed query string and
 * interpretations match the expected values.
 *
 * @param raw_query_string The search query.
 * @param expected_processed_query_string The processed search query.
 * @param schema_rules A vector of strings, each string representing a schema rule.
 * @param expected_serialized_interpretations  The expected set of serialized interpretations.
 */
auto test_query(
        string_view raw_query_string,
        string_view expected_processed_query_string,
        vector<string> const& schema_rules,
        set<string> const& expected_serialized_interpretations
) -> void;

/**
 * Initializes a `ByteLexer` with space as a delimiter and the given `schema_rules`.
 *
 * @param schema_rules A vector of strings, each string representing a schema rule.
 * @return The initialized `ByteLexer`.
 */
auto make_test_lexer(vector<string> const& schema_rules) -> ByteLexer;

auto test_query(
        string_view const raw_query_string,
        string_view const expected_processed_query_string,
        vector<string> const& schema_rules,
        set<string> const& expected_serialized_interpretations
) -> void {
    auto const lexer{make_test_lexer(schema_rules)};

    Query const query{string(raw_query_string)};
    REQUIRE(expected_processed_query_string == query.get_processed_query_string());

    auto const interpretations{query.get_all_multi_token_interpretations(lexer)};
    set<string> serialized_interpretations;
    for (auto const& interpretation : interpretations) {
        serialized_interpretations.insert(interpretation.serialize());
    }

    REQUIRE(expected_serialized_interpretations == serialized_interpretations);
}

auto make_test_lexer(vector<string> const& schema_rules) -> ByteLexer {
    ByteLexer lexer;
    lexer.set_delimiters({' '});

    Schema schema;
    for (auto const& schema_rule : schema_rules) {
        schema.add_variable(schema_rule, -1);
    }

    auto const schema_ast = schema.release_schema_ast_ptr();
    REQUIRE(nullptr != schema_ast);
    REQUIRE(schema_rules.size() == schema_ast->m_schema_vars.size());
    for (size_t i{0}; i < schema_ast->m_schema_vars.size(); ++i) {
        REQUIRE(nullptr != schema_ast->m_schema_vars[i]);
        auto& capture_rule_ast{dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[i])};
        lexer.add_rule(i, std::move(capture_rule_ast.m_regex_ptr));
    }

    lexer.generate();
    return lexer;
}
}  // namespace

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests an empty `Query`.
 */
TEST_CASE("empty_query", "[Query]") {
    constexpr string_view cRawQueryString;
    constexpr string_view cProcessedQueryString;
    vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations;

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a greedy wildcard `Query`.
 */
TEST_CASE("greedy_wildcard_query", "[Query]") {
    constexpr string_view cRawQueryString{"*"};
    constexpr string_view cProcessedQueryString{"*"};
    vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations{"logtype='*', contains_wildcard='0'"};

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with repeated greedy wildcards.
 */
TEST_CASE("repeated_greedy_wildcard_query", "[Query]") {
    constexpr string_view cRawQueryString{"a**b"};
    constexpr string_view cProcessedQueryString{"a*b"};
    vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations{
            "logtype='a*b', contains_wildcard='0'",
            "logtype='a***b', contains_wildcard='0'",
            "logtype='<0>(a*)**b', contains_wildcard='10'",
            "logtype='<0>(a*)*<0>(*b)', contains_wildcard='101'",
            "logtype='<0>(a*b)', contains_wildcard='1'",
            "logtype='a**<0>(*b)', contains_wildcard='01'"
    };

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with a non-greedy wildcard followed by a greedy wildcard.
 */
TEST_CASE("short_wildcard_sequence_query", "[Query]") {
    constexpr string_view cRawQueryString{"a?*b"};
    constexpr string_view cProcessedQueryString{"a*b"};
    vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations{
            "logtype='a*b', contains_wildcard='0'",
            "logtype='a***b', contains_wildcard='0'",
            "logtype='<0>(a*)**b', contains_wildcard='10'",
            "logtype='<0>(a*)*<0>(*b)', contains_wildcard='101'",
            "logtype='<0>(a*b)', contains_wildcard='1'",
            "logtype='a**<0>(*b)', contains_wildcard='01'"
    };

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with a long mixed wildcard sequence.
 */
TEST_CASE("long_mixed_wildcard_sequence_query", "[Query]") {
    constexpr string_view cRawQueryString{"a?*?*?*?b"};
    constexpr string_view cProcessedQueryString{"a*b"};
    vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations{
            "logtype='a*b', contains_wildcard='0'",
            "logtype='a***b', contains_wildcard='0'",
            "logtype='<0>(a*)**b', contains_wildcard='10'",
            "logtype='<0>(a*)*<0>(*b)', contains_wildcard='101'",
            "logtype='<0>(a*b)', contains_wildcard='1'",
            "logtype='a**<0>(*b)', contains_wildcard='01'"
    };

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with a long non-greedy wildcard sequence.
 */
TEST_CASE("long_non_greedy_wildcard_sequence_query", "[Query]") {
    constexpr string_view cRawQueryString{"a????b"};
    constexpr string_view cProcessedQueryString{"a????b"};
    vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations{
            R"(logtype='a????b', contains_wildcard='0')",

            R"(logtype='<0>(a?)???b', contains_wildcard='10')",
            R"(logtype='<0>(a??)??b', contains_wildcard='10')",
            R"(logtype='<0>(a???)?b', contains_wildcard='10')",
            R"(logtype='<0>(a????b)', contains_wildcard='1')",

            R"(logtype='a?<0>(?)??b', contains_wildcard='010')",
            R"(logtype='a?<0>(??)?b', contains_wildcard='010')",
            R"(logtype='a?<0>(???b)', contains_wildcard='01')",
            R"(logtype='a?<0>(?)?<0>(?b)', contains_wildcard='0101')",

            R"(logtype='a??<0>(?)?b', contains_wildcard='010')",
            R"(logtype='a??<0>(??b)', contains_wildcard='01')",

            R"(logtype='a???<0>(?b)', contains_wildcard='01')",

            R"(logtype='<0>(a?)?<0>(?)?b', contains_wildcard='1010')",
            R"(logtype='<0>(a?)?<0>(??b)', contains_wildcard='101')",
            R"(logtype='<0>(a?)??<0>(?b)', contains_wildcard='101')",

            R"(logtype='<0>(a??)?<0>(?b)', contains_wildcard='101')",

            // Double dipping on delimiters
            R"(logtype='<0>(a?)<0>(?)??b', contains_wildcard='110')",
            R"(logtype='<0>(a?)<0>(??)?b', contains_wildcard='110')",
            R"(logtype='<0>(a?)<0>(???b)', contains_wildcard='11')",
            R"(logtype='<0>(a?)<0>(?)?<0>(?b)', contains_wildcard='1101')",
            R"(logtype='<0>(a?)?<0>(?)<0>(?b)', contains_wildcard='1011')",

            R"(logtype='<0>(a??)<0>(?)?b', contains_wildcard='110')",
            R"(logtype='<0>(a??)<0>(??b)', contains_wildcard='11')",

            R"(logtype='<0>(a???)<0>(?b)', contains_wildcard='11')",

            R"(logtype='a?<0>(?)<0>(?)?b', contains_wildcard='0110')",
            R"(logtype='a?<0>(?)<0>(??b)', contains_wildcard='011')",

            R"(logtype='a?<0>(??)<0>(?b)', contains_wildcard='011')",
            R"(logtype='a??<0>(?)<0>(?b)', contains_wildcard='011')",

            R"(logtype='<0>(a?)<0>(?)<0>(?)?b', contains_wildcard='1110')",
            R"(logtype='<0>(a?)<0>(?)<0>(??b)', contains_wildcard='111')",
            R"(logtype='<0>(a?)<0>(??)<0>(?b)', contains_wildcard='111')",
            R"(logtype='<0>(a??)<0>(?)<0>(?b)', contains_wildcard='111')",
            R"(logtype='a?<0>(?)<0>(?)<0>(?b)', contains_wildcard='0111')",

            R"(logtype='<0>(a?)<0>(?)<0>(?)<0>(?b)', contains_wildcard='1111')"
    };

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with an escaped '*' character.
 */
TEST_CASE("escaped_star_query", "[Query]") {
    constexpr string_view cRawQueryString{R"(a\*b)"};
    constexpr string_view cProcessedQueryString{R"(a\*b)"};
    vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations{
            R"(logtype='a\*b', contains_wildcard='0')"
    };

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with an escaped '*' character.
 *
 * NOTE: This has a static-text case as strings "1", "2', and "3" in isolation aren't surrounded by
 * delimiters. These tokens then build up the interpretation "123". Although additional
 * interpretations don't impact correctness, they may impact performance. We can optimize these out,
 * but it'll make the code messy. Instead, we should eventually remove the explicit tracking of
 * static-tokens, in favor of only tracking variable tokens.
 */
TEST_CASE("int_query", "[Query]") {
    constexpr string_view cRawQueryString{"123"};
    constexpr string_view cProcessedQueryString{"123"};
    vector<string> const schema_rules{{R"(int:\d+)"}};
    set<string> const expected_serialized_interpretations{
            R"(logtype='123', contains_wildcard='0')",
            R"(logtype='<0>(123)', contains_wildcard='0')"
    };

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with multiple variable types.
 *
 * This test ensures that each non-wildcard token is assigned to the highest priority variable.
 *
 * NOTE: Similar to the above `int_query` test there are unneeded intepretations due to aggresively
 * generating static-text tokens.
 */
TEST_CASE("non_wildcard_multi_variable_query", "[Query]") {
    constexpr string_view cRawQueryString{"abc123 123"};
    constexpr string_view cProcessedQueryString{"abc123 123"};

    SECTION("int_priority") {
        vector<string> const schema_rules{{R"(int:(\d+))"}, {R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
        set<string> const expected_serialized_interpretations{
                R"(logtype='abc123 123', contains_wildcard='0')",
                R"(logtype='abc123 <0>(123)', contains_wildcard='00')",
                R"(logtype='<1>(abc123) 123', contains_wildcard='00')",
                R"(logtype='<1>(abc123) <0>(123)', contains_wildcard='000')"
        };

        test_query(
                cRawQueryString,
                cProcessedQueryString,
                schema_rules,
                expected_serialized_interpretations
        );
    }

    SECTION("has_number_priority") {
        vector<string> const schema_rules{{R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}, {R"(int:(\d+))"}};
        set<string> const expected_serialized_interpretations{
                R"(logtype='abc123 123', contains_wildcard='0')",
                R"(logtype='abc123 <0>(123)', contains_wildcard='00')",
                R"(logtype='<0>(abc123) 123', contains_wildcard='00')",
                R"(logtype='<0>(abc123) <0>(123)', contains_wildcard='000')"
        };

        test_query(
                cRawQueryString,
                cProcessedQueryString,
                schema_rules,
                expected_serialized_interpretations
        );
    }
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with multiple variable types.
 *
 * This test ensures that each greedy wildcard token is identified as all correct token types.
 *
 * NOTE: Similar to the above `int_query` test there are unneeded intepretations due to aggresively
 * generating static-text tokens. This same issue causes interpretations with redundant wildcards.
 */
TEST_CASE("wildcard_multi_variable_query", "[Query]") {
    constexpr string_view cRawQueryString{"abc123* *123"};
    constexpr string_view cProcessedQueryString{"abc123* *123"};

    vector<string> const schema_rules{{R"(int:(\d+))"}, {R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)"}};
    set<string> const expected_serialized_interpretations{
            R"(logtype='abc123* *123', contains_wildcard='0')",
            R"(logtype='abc123*** *123', contains_wildcard='0')",
            R"(logtype='abc123* ***123', contains_wildcard='0')",
            R"(logtype='abc123*** ***123', contains_wildcard='0')",
            R"(logtype='abc123* **<0>(*123)', contains_wildcard='01')",
            R"(logtype='abc123*** **<0>(*123)', contains_wildcard='01')",
            R"(logtype='abc123* **<1>(*123)', contains_wildcard='01')",
            R"(logtype='abc123*** **<1>(*123)', contains_wildcard='01')",
            R"(logtype='<1>(abc123*)** *123', contains_wildcard='10')",
            R"(logtype='<1>(abc123*)** ***123', contains_wildcard='10')",
            R"(logtype='<1>(abc123*)** **<0>(*123)', contains_wildcard='101')",
            R"(logtype='<1>(abc123*)** **<1>(*123)', contains_wildcard='101')"
    };

    test_query(
            cRawQueryString,
            cProcessedQueryString,
            schema_rules,
            expected_serialized_interpretations
    );
}
