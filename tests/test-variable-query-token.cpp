#include <cstdint>

#include <log_surgeon/wildcard_query_parser/VariableQueryToken.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_variable_query_token `VariableQueryToken` unit tests.
 * @brief Unit tests for `VariableQueryToken` construction and comparison.

 * These unit tests contain the `VariableQueryToken` tag.
 */

using log_surgeon::wildcard_query_parser::VariableQueryToken;

/**
 * @ingroup unit_tests_variable_query_token
 * @brief Tests `operator<` and `operator>`.
 */
TEST_CASE("comparison_operators", "[VariableQueryToken]") {
    constexpr uint32_t cEmptyId{0};
    constexpr uint32_t cIntId{2};
    constexpr uint32_t cHasNumberId{7};

    VariableQueryToken empty_token{cEmptyId, "", false};
    VariableQueryToken token_int_123{cIntId, "123", false};
    VariableQueryToken token_int_456{cIntId, "456", false};
    VariableQueryToken token_has_number_123{cHasNumberId, "123", false};
    VariableQueryToken token_has_number_user123_wildcard{cHasNumberId, "user123*", true};
    VariableQueryToken another_token_int_123{cIntId, "123", false};

    SECTION("less_than_operator") {
        // empty token
        REQUIRE(empty_token < token_int_123);
        REQUIRE(empty_token < token_int_456);
        REQUIRE(empty_token < token_has_number_123);
        REQUIRE(empty_token < token_has_number_user123_wildcard);
        REQUIRE_FALSE(token_int_123 < empty_token);
        REQUIRE_FALSE(token_int_456 < empty_token);
        REQUIRE_FALSE(token_has_number_123 < empty_token);
        REQUIRE_FALSE(token_has_number_user123_wildcard < empty_token);

        // token_int_123
        REQUIRE(token_int_123 < token_int_456);
        REQUIRE(token_int_123 < token_has_number_123);
        REQUIRE(token_int_123 < token_has_number_user123_wildcard);
        REQUIRE_FALSE(token_int_456 < token_int_123);
        REQUIRE_FALSE(token_has_number_123 < token_int_123);
        REQUIRE_FALSE(token_has_number_user123_wildcard < token_int_123);

        // token_int_456
        REQUIRE(token_int_456 < token_has_number_123);
        REQUIRE(token_int_456 < token_has_number_user123_wildcard);
        REQUIRE_FALSE(token_has_number_123 < token_int_456);
        REQUIRE_FALSE(token_has_number_user123_wildcard < token_int_456);

        // token_has_number_123
        REQUIRE(token_has_number_123 < token_has_number_user123_wildcard);
        REQUIRE_FALSE(token_has_number_user123_wildcard < token_has_number_123);

        // False for same value
        REQUIRE_FALSE(token_int_123 < another_token_int_123);
    }

    SECTION("greater_than_operator") {
        // empty token
        REQUIRE(token_int_123 > empty_token);
        REQUIRE(token_int_456 > empty_token);
        REQUIRE(token_has_number_123 > empty_token);
        REQUIRE(token_has_number_user123_wildcard > empty_token);
        REQUIRE_FALSE(empty_token > token_int_123);
        REQUIRE_FALSE(empty_token > token_int_456);
        REQUIRE_FALSE(empty_token > token_has_number_123);
        REQUIRE_FALSE(empty_token > token_has_number_user123_wildcard);

        // token_int_123
        REQUIRE(token_int_456 > token_int_123);
        REQUIRE(token_has_number_123 > token_int_123);
        REQUIRE(token_has_number_user123_wildcard > token_int_123);
        REQUIRE_FALSE(token_int_123 > token_int_456);
        REQUIRE_FALSE(token_int_123 > token_has_number_123);
        REQUIRE_FALSE(token_int_123 > token_has_number_user123_wildcard);

        // token_int_456
        REQUIRE(token_has_number_123 > token_int_456);
        REQUIRE(token_has_number_user123_wildcard > token_int_456);
        REQUIRE_FALSE(token_int_456 > token_has_number_123);
        REQUIRE_FALSE(token_int_456 > token_has_number_user123_wildcard);

        // token_has_number_123
        REQUIRE(token_has_number_user123_wildcard > token_has_number_123);
        REQUIRE_FALSE(token_has_number_123 > token_has_number_user123_wildcard);

        // False for same value
        REQUIRE_FALSE(token_int_123 > another_token_int_123);
    }
}
