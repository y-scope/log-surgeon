#include <cstdint>
#include <string_view>

#include <log_surgeon/query_parser/QueryInterpretation.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_query_interpretation `QueryInterpretation` unit tests.
 * @brief Unit tests for `QueryInterpretation` construction, mutation, and comparison.

 * These unit tests contain the `QueryInterpretation` tag.
 */

using log_surgeon::query_parser::QueryInterpretation;
using std::string_view;

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates an empty `QueryInterpretation` and tests serialization.
 */
TEST_CASE("empty_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='', has_wildcard=''"};

    QueryInterpretation const qi;
    REQUIRE(qi.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with only static-text and tests serialization.
 */
TEST_CASE("static_text_query_interpretation", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='Static text', has_wildcard='0'"};

    QueryInterpretation const qi{"Static text"};
    REQUIRE(qi.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with a variable and tests serialization.
 */
TEST_CASE("variable_query_interpretation", "[QueryInterpretation]") {
    constexpr uint32_t cHasNumberId{7};
    constexpr string_view cExpectedSerialization{"logtype='<7>(var123)', has_wildcard='0'"};

    QueryInterpretation const qi{cHasNumberId, "var123", false};
    REQUIRE(qi.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Creates a `QueryInterpretation` with a wildcard variable and tests serialization.
 */
TEST_CASE("wildcard_variable_query_interpretation", "[QueryInterpretation]") {
    constexpr uint32_t cFloatId{1};
    constexpr string_view cExpectedSerialization{"logtype='<1>(123.123*)', has_wildcard='1'"};

    QueryInterpretation const qi{cFloatId, "123.123*", true};
    REQUIRE(qi.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends empty static-text to a `QueryInterpretation` and tests serialization.
 */
TEST_CASE("append_empty_static_text", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='', has_wildcard=''"};

    QueryInterpretation qi;
    qi.append_static_token("");
    REQUIRE(qi.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Appends an empty variable to a `QueryInterpretation` and tests serialization.
 */
TEST_CASE("append_empty_variable", "[QueryInterpretation]") {
    constexpr uint32_t cEmptyId{0};
    constexpr string_view cExpectedSerialization{"logtype='<0>()', has_wildcard='0'"};

    QueryInterpretation qi;
    qi.append_variable_token(cEmptyId, "", false);
    REQUIRE(qi.serialize() == cExpectedSerialization);
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

    QueryInterpretation qi;
    qi.append_static_token("start ");
    qi.append_variable_token(cIntId, "*123*", true);
    qi.append_static_token(" middle ");
    qi.append_variable_token(cFloatId, "12.3", false);
    qi.append_static_token(" end");
    REQUIRE(qi.serialize() == cExpectedSerialization);
}

/**
 * @ingroup unit_tests_query_interpretation
 * @brief Tests whether adjacent static-text tokens are merged for canonicalization.
 */
TEST_CASE("append_canonicalization", "[QueryInterpretation]") {
    constexpr string_view cExpectedSerialization{"logtype='ab', has_wildcard='0'"};

    QueryInterpretation qi;
    qi.append_static_token("a");
    qi.append_static_token("b");
    REQUIRE(qi.serialize() == cExpectedSerialization);
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
 * @brief Tests `operator<` with various token types and orders.
 */
TEST_CASE("less_than_operator", "[QueryInterpretation]") {
    constexpr uint32_t cFloatId{1};
    constexpr uint32_t cIntId{2};
    constexpr uint32_t cHasNumberId{7};

    QueryInterpretation qi1;
    QueryInterpretation qi2;

    SECTION("different_length_logtype") {
        qi1.append_static_token("a");
        qi2.append_static_token("a");
        qi2.append_variable_token(cFloatId, "1.1", false);

        REQUIRE(qi1 < qi2);
        REQUIRE_FALSE(qi2 < qi1);
    }

    SECTION("different_static_content") {
        qi1.append_static_token("a");
        qi2.append_static_token("b");

        REQUIRE(qi1 < qi2);
        REQUIRE_FALSE(qi2 < qi1);
    }

    SECTION("different_var_types") {
        qi1.append_variable_token(cIntId, "123", false);
        qi2.append_variable_token(cHasNumberId, "123", false);

        REQUIRE(qi1 < qi2);
        REQUIRE_FALSE(qi2 < qi1);
    }

    SECTION("different_var_values") {
        qi1.append_variable_token(cIntId, "123", false);
        qi2.append_variable_token(cIntId, "456", false);

        REQUIRE(qi1 < qi2);
        REQUIRE_FALSE(qi2 < qi1);
    }

    SECTION("token_order") {
        qi1.append_static_token("hello");
        qi1.append_variable_token(cIntId, "123", false);
        qi2.append_variable_token(cIntId, "123", false);
        qi2.append_static_token("hello");

        // `StaticQueryToken` is a lower index in the variant so is considered less than
        // `VariableQueryToken`.
        REQUIRE(qi1 < qi2);
        REQUIRE_FALSE(qi2 < qi1);
    }

    SECTION("identical_tokens") {
        qi1.append_variable_token(cIntId, "123", false);
        qi2.append_variable_token(cIntId, "123", false);

        REQUIRE_FALSE(qi1 < qi2);
        REQUIRE_FALSE(qi2 < qi1);
    }
}
