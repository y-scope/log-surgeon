#include <cstdint>
#include <string_view>
#include <vector>

#include <log_surgeon/wildcard_query_parser/QueryInterpretation.hpp>

#include <catch2/catch_test_macros.hpp>

#include "comparison_test_utils.hpp"

/**
 * @defgroup unit_tests_query_interpretation `QueryInterpretation` unit tests.
 * @brief Unit tests for `QueryInterpretation` construction, mutation, and comparison.

 * These unit tests contain the `QueryInterpretation` tag.
 */

using log_surgeon::tests::pairwise_comparison_of_strictly_ascending_vector;
using log_surgeon::tests::test_equal;
using log_surgeon::wildcard_query_parser::QueryInterpretation;
using std::string_view;

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates an empty `QueryInterpretation` and tests serialization.
 */
TEST_CASE("empty_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='', contains_wildcard=''"};

    QueryInterpretation const query_interpretation;
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with only static-text and tests serialization.
 */
TEST_CASE("static_text_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='Static text', contains_wildcard='0'"};

    QueryInterpretation const query_interpretation{"Static text"};
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with a variable and tests serialization.
 */
TEST_CASE("variable_query_interpretation", "[QueryInterpretation]") {
    constexpr uint32_t cHasNumberId{7};
    constexpr string_view cExpectedSerialization{"logtype='<7>(var123)', contains_wildcard='0'"};

    QueryInterpretation const query_interpretation{cHasNumberId, "var123", false};
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with a wildcard variable and tests serialization.
 */
TEST_CASE("wildcard_variable_query_interpretation", "[QueryInterpretation]") {
    constexpr uint32_t cFloatId{1};
    constexpr string_view cExpectedSerialization{"logtype='<1>(123.123*)', contains_wildcard='1'"};

    QueryInterpretation const query_interpretation{cFloatId, "123.123*", true};
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends empty static-text to a `QueryInterpretation` and tests serialization.
 */
TEST_CASE("append_empty_static_text", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='', contains_wildcard=''"};

    QueryInterpretation query_interpretation;
    query_interpretation.append_static_token("");
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends an empty variable to a `QueryInterpretation` and tests serialization.
 */
TEST_CASE("append_empty_variable", "[QueryInterpretation]") {
    constexpr uint32_t cEmptyId{0};
    constexpr string_view cExpectedSerialization{"logtype='<0>()', contains_wildcard='0'"};

    QueryInterpretation query_interpretation;
    query_interpretation.append_variable_token(cEmptyId, "", false);
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends an empty `QueryInterpretation` to another and tests serialization.
 */
TEST_CASE("append_empty_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='hello', contains_wildcard='0'"};

    QueryInterpretation query_interpretation{"hello"};
    QueryInterpretation const empty_query_interpretation;
    query_interpretation.append_query_interpretation(empty_query_interpretation);
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends a sequence of static and variable tokens and tests serialization.
 */
TEST_CASE("append_tokens", "[QueryInterpretation]") {
    constexpr uint32_t cFloatId{1};
    constexpr uint32_t cIntId{2};
    constexpr string_view cExpectedSerialization{
            "logtype='start <2>(*123*) middle <1>(12.3) end', contains_wildcard='01000'"
    };

    QueryInterpretation query_interpretation;
    query_interpretation.append_static_token("start ");
    query_interpretation.append_variable_token(cIntId, "*123*", true);
    query_interpretation.append_static_token(" middle ");
    query_interpretation.append_variable_token(cFloatId, "12.3", false);
    query_interpretation.append_static_token(" end");
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Tests whether adjacent static-text tokens are merged for canonicalization.
 */
TEST_CASE("append_canonicalization", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='ab', contains_wildcard='0'"};

    QueryInterpretation query_interpretation;
    query_interpretation.append_static_token("a");
    query_interpretation.append_static_token("b");
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends a `QueryInterpretation` to another and tests serialization and canonicalization.
 */
TEST_CASE("append_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='foobar', contains_wildcard='0'"};

    QueryInterpretation prefix{"foo"};
    QueryInterpretation const suffix{"bar"};
    prefix.append_query_interpretation(suffix);
    REQUIRE(prefix.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Tests `operator==`, `operator<=>`, and all derived operators.
 */
TEST_CASE("comparison_operators", "[QueryInterpretation]") {
    constexpr uint32_t cIntId{2};
    constexpr uint32_t cHasNumId{7};

    std::vector<QueryInterpretation> ordered_interpretations;
    // a
    ordered_interpretations.emplace_back("a");
    // a<int>(123)
    ordered_interpretations.emplace_back("a");
    ordered_interpretations.back().append_variable_token(cIntId, "123", false);
    // b
    ordered_interpretations.emplace_back("b");
    // <int>(123)
    ordered_interpretations.emplace_back(cIntId, "123", false);
    // <int>(123)a
    ordered_interpretations.emplace_back(cIntId, "123", false);
    ordered_interpretations.back().append_static_token("a");
    // <int>(123*)
    ordered_interpretations.emplace_back(cIntId, "123*", true);
    // <int>(1234)
    ordered_interpretations.emplace_back(cIntId, "1234", false);
    // <int>(456)
    ordered_interpretations.emplace_back(cIntId, "456", false);
    // <hasNumber>(123)
    ordered_interpretations.emplace_back(cHasNumId, "123", false);

    // <hasNumber>(abc*123)
    QueryInterpretation const interpretation{cHasNumId, "abc*123", true};
    QueryInterpretation const duplicate_interpretation{cHasNumId, "abc*123", true};

    pairwise_comparison_of_strictly_ascending_vector(ordered_interpretations);
    test_equal(interpretation, duplicate_interpretation);
}
