#include <cstddef>
#include <cstdint>
#include <string_view>

#include <log_surgeon/wildcard_query_parser/QueryInterpretation.hpp>

#include <catch2/catch_test_macros.hpp>

#include "comparison_test_utils.hpp"

/**
 * @defgroup unit_tests_query_interpretation `QueryInterpretation` unit tests.
 * @brief Unit tests for `QueryInterpretation` construction, mutation, and comparison.

 * These unit tests contain the `QueryInterpretation` tag.
 */

using log_surgeon::wildcard_query_parser::QueryInterpretation;
using std::string_view;

using log_surgeon::tests::test_equal;
using log_surgeon::tests::test_greater_than;
using log_surgeon::tests::test_less_than;
using log_surgeon::wildcard_query_parser::VariableQueryToken;

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates an empty `QueryInterpretation` and tests serialization.
 */
TEST_CASE("empty_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='', has_wildcard=''"};

    QueryInterpretation const query_interpretation;
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with only static-text and tests serialization.
 */
TEST_CASE("static_text_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='Static text', has_wildcard='0'"};

    QueryInterpretation const query_interpretation{"Static text"};
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with a variable and tests serialization.
 */
TEST_CASE("variable_query_interpretation", "[QueryInterpretation]") {
    constexpr uint32_t cHasNumberId{7};
    constexpr string_view cExpectedSerialization{"logtype='<7>(var123)', has_wildcard='0'"};

    QueryInterpretation const query_interpretation{cHasNumberId, "var123", false};
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with a wildcard variable and tests serialization.
 */
TEST_CASE("wildcard_variable_query_interpretation", "[QueryInterpretation]") {
    constexpr uint32_t cFloatId{1};
    constexpr string_view cExpectedSerialization{"logtype='<1>(123.123*)', has_wildcard='1'"};

    QueryInterpretation const query_interpretation{cFloatId, "123.123*", true};
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends empty static-text to a `QueryInterpretation` and tests serialization.
 */
TEST_CASE("append_empty_static_text", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='', has_wildcard=''"};

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
    constexpr string_view cExpectedSerialization{"logtype='<0>()', has_wildcard='0'"};

    QueryInterpretation query_interpretation;
    query_interpretation.append_variable_token(cEmptyId, "", false);
    REQUIRE(query_interpretation.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends an empty `QueryInterpretation` to another and tests serialization.
 */
TEST_CASE("append_empty_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='hello', has_wildcard='0'"};

    QueryInterpretation query_interpretation{"hello"};
    QueryInterpretation empty_query_interpretation;
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
            "logtype='start <2>(*123*) middle <1>(12.3) end', has_wildcard='01000'"
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
    constexpr string_view cExpectedSerialization{"logtype='ab', has_wildcard='0'"};

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
    constexpr string_view cExpectedSerialization{"logtype='foobar', has_wildcard='0'"};

    QueryInterpretation prefix{"foo"};
    QueryInterpretation suffix{"bar"};
    prefix.append_query_interpretation(suffix);
    REQUIRE(prefix.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Tests `operator==`, `operator<=>`, and all derived operators.
 */
TEST_CASE("comparison_operators", "[QueryInterpretation]") {
    constexpr uint32_t cIntId{2};
    constexpr uint32_t cHasNumberId{7};

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
    // <cHasNumberId>(123)
    ordered_interpretations.emplace_back(cHasNumberId, "123", false);

    // <cHasNumberId>(abc*123)
    QueryInterpretation const interpretation(cHasNumberId, "abc*123", true);
    QueryInterpretation const duplicate_interpretation(cHasNumberId, "abc*123", true);

    for (size_t i{0}; i < ordered_interpretations.size(); i++) {
        CAPTURE(i);
        for (size_t j{0}; j < ordered_interpretations.size(); j++) {
            CAPTURE(j);
            if (i < j) {
                test_less_than(ordered_interpretations[i], ordered_interpretations[j]);
            } else if (i == j) {
                test_equal(ordered_interpretations[i], ordered_interpretations[j]);
            } else {
                test_greater_than(ordered_interpretations[i], ordered_interpretations[j]);
            }
        }
    }
    test_equal(interpretation, duplicate_interpretation);
}
