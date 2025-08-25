#include <set>
#include <string>
#include <string_view>
#include <utility>

#include <log_surgeon/BufferParser.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/wildcard_query_parser/Query.hpp>

#include <catch2/catch_message.hpp>
#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_query `Query` unit tests.
 * @brief Unit tests for `Query` construction, mutation, and comparison.

 * These unit tests contain the `Query` tag.
 */

using std::set;
using std::string;
using std::string_view;

using log_surgeon::BufferParser;
using log_surgeon::Schema;
using log_surgeon::wildcard_query_parser::Query;

namespace {
/**
 * Creates a query from the given query string and tests that its processed query string and
 * interpretations matche the expeced values.
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
 * Initializes a `BufferParser` with delimiters "\n\r\[:" and variable "myVar:userID=(?<uid>123)".
 *
 * @result The initialized `BufferParser`.
 */
auto make_test_buffer() -> BufferParser;

auto test_query(
        string_view raw_query_string,
        string_view expected_processed_query_string,
        set<string> const& expected_serialized_interpretations
) -> void {
    auto const& buffer_parser{make_test_buffer()};
    auto const& lexer{buffer_parser.get_log_parser().m_lexer};

    Query const query{string(raw_query_string)};
    REQUIRE(expected_processed_query_string == query.get_processed_query_string());

    auto const interpretations{query.get_all_multi_token_interpretations(lexer)};
    set<string> serialized_interpretations;
    for (auto const& interpretation : interpretations) {
        serialized_interpretations.insert(interpretation.serialize());
    }

    CAPTURE(expected_serialized_interpretations);
    CAPTURE(serialized_interpretations);
    REQUIRE(expected_serialized_interpretations.size() == serialized_interpretations.size());
    REQUIRE(expected_serialized_interpretations == serialized_interpretations);
}

auto make_test_buffer() -> BufferParser {
    Schema schema;
    schema.add_delimiters(R"(delimiters: \n\r\[:,)");
    schema.add_variable(R"(hasNumber:[A-Za-z]*\d+[A-Za-z]*)", -1);
    return BufferParser(std::move(schema.release_schema_ast_ptr()));
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
