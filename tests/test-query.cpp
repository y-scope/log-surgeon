#include <set>
#include <string>
#include <string_view>
#include <utility>

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

namespace {
/**
 * Creates a query from the given query string and tests that its processed query string and
 * interpretations match the expected values.
 *
 * @param raw_query_string The search query.
 * @param expected_processed_query_string The processed search query.
 * @param expected_serialized_interpretations  The expected set of serialized interpretations.
 */
auto test_query(
        string_view raw_query_string,
        string_view expected_processed_query_string,
        set<string> const& expected_serialized_interpretations
) -> void;

/**
 * Initializes a `ByteLexer` with delimiters "\n\r\[:" and variable
 * "hasNumber:[A-Za-z]*\d+[A-Za-z]*".
 *
 * @return The initialized `ByteLexer`.
 */
auto make_test_lexer() -> ByteLexer;

auto test_query(
        string_view const raw_query_string,
        string_view const expected_processed_query_string,
        set<string> const& expected_serialized_interpretations
) -> void {
    auto const lexer{make_test_lexer()};

    Query const query{string(raw_query_string)};
    REQUIRE(expected_processed_query_string == query.get_processed_query_string());

    auto const interpretations{query.get_all_multi_token_interpretations(lexer)};
    set<string> serialized_interpretations;
    for (auto const& interpretation : interpretations) {
        serialized_interpretations.insert(interpretation.serialize());
    }

    REQUIRE(expected_serialized_interpretations == serialized_interpretations);
}

auto make_test_lexer() -> ByteLexer {
    Schema schema;
    schema.add_delimiters(R"(delimiters: \n\r\[:,)");
    schema.add_variable(R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)", -1);

    ByteLexer lexer;
    lexer.m_symbol_id["hasNumber"] = 0;
    lexer.m_id_symbol[0] = "hasNumber";

    auto const schema_ast = schema.release_schema_ast_ptr();
    REQUIRE(nullptr != schema_ast);
    REQUIRE(1 == schema_ast->m_schema_vars.size());
    REQUIRE(nullptr != schema_ast->m_schema_vars[0]);
    auto& capture_rule_ast = dynamic_cast<SchemaVarAST&>(*schema_ast->m_schema_vars[0]);
    lexer.add_rule(lexer.m_symbol_id["hasNumber"], std::move(capture_rule_ast.m_regex_ptr));

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
    set<string> const expected_serialized_interpretations;

    test_query(cRawQueryString, cProcessedQueryString, expected_serialized_interpretations);
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a greedy wildcard `Query`.
 */
TEST_CASE("greedy_wildcard_query", "[Query]") {
    constexpr string_view cRawQueryString{"*"};
    constexpr string_view cProcessedQueryString{"*"};
    set<string> const expected_serialized_interpretations{"logtype='*', contains_wildcard='0'"};

    test_query(cRawQueryString, cProcessedQueryString, expected_serialized_interpretations);
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with repeated greedy wildcards.
 */
TEST_CASE("repeated_greedy_wildcard_query", "[Query]") {
    constexpr string_view cRawQueryString{"a**b"};
    constexpr string_view cProcessedQueryString{"a*b"};
    set<string> const expected_serialized_interpretations{
            "logtype='a*b', contains_wildcard='0'",
            "logtype='a***b', contains_wildcard='0'",
            "logtype='<0>(a*)**b', contains_wildcard='10'",
            "logtype='<0>(a*)*<0>(*b)', contains_wildcard='101'",
            "logtype='<0>(a*b)', contains_wildcard='1'",
            "logtype='a**<0>(*b)', contains_wildcard='01'"
    };

    test_query(cRawQueryString, cProcessedQueryString, expected_serialized_interpretations);
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with a non-greedy wildcard followed by a greedy wildcard.
 */
TEST_CASE("short_wildcard_sequence_query", "[Query]") {
    constexpr string_view cRawQueryString{"a?*b"};
    constexpr string_view cProcessedQueryString{"a*b"};
    set<string> const expected_serialized_interpretations{
            "logtype='a*b', contains_wildcard='0'",
            "logtype='a***b', contains_wildcard='0'",
            "logtype='<0>(a*)**b', contains_wildcard='10'",
            "logtype='<0>(a*)*<0>(*b)', contains_wildcard='101'",
            "logtype='<0>(a*b)', contains_wildcard='1'",
            "logtype='a**<0>(*b)', contains_wildcard='01'"
    };

    test_query(cRawQueryString, cProcessedQueryString, expected_serialized_interpretations);
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with a long mixed wildcard sequence.
 */
TEST_CASE("long_mixed_wildcard_sequence_query", "[Query]") {
    constexpr string_view cRawQueryString{"a?*?*?*?b"};
    constexpr string_view cProcessedQueryString{"a*b"};
    set<string> const expected_serialized_interpretations{
            "logtype='a*b', contains_wildcard='0'",
            "logtype='a***b', contains_wildcard='0'",
            "logtype='<0>(a*)**b', contains_wildcard='10'",
            "logtype='<0>(a*)*<0>(*b)', contains_wildcard='101'",
            "logtype='<0>(a*b)', contains_wildcard='1'",
            "logtype='a**<0>(*b)', contains_wildcard='01'"
    };

    test_query(cRawQueryString, cProcessedQueryString, expected_serialized_interpretations);
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with a long non-greedy wildcard sequence.
 */
TEST_CASE("long_non_greedy_wildcard_sequence_query", "[Query]") {
    constexpr string_view cRawQueryString{"a????b"};
    constexpr string_view cProcessedQueryString{"a????b"};
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

    test_query(cRawQueryString, cProcessedQueryString, expected_serialized_interpretations);
}

/**
 * @ingroup unit_tests_query
 * @brief Creates and tests a query with an escaped '*' character.
 */
TEST_CASE("escaped_star_query", "[Query]") {
    constexpr string_view cRawQueryString{R"(a\*b)"};
    constexpr string_view cProcessedQueryString{R"(a\*b)"};
    set<string> const expected_serialized_interpretations{
            R"(logtype='a\*b', contains_wildcard='0')"
    };

    test_query(cRawQueryString, cProcessedQueryString, expected_serialized_interpretations);
}
