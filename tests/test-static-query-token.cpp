#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_static_query_token `StaticQueryToken` unit tests.
 * @brief Unit tests for `StaticQueryToken` construction, mutation, and comparison.

 * These unit tests contain the `StaticQueryToken` tag.
 */

using log_surgeon::wildcard_query_parser::StaticQueryToken;

/**
 * @ingroup unit_tests_static_query_token
 * @brief Tests `operator<` and `operator>`.
 */
TEST_CASE("comparison_operators", "[StaticQueryToken]") {
    StaticQueryToken empty_token{""};
    StaticQueryToken token_abc{"abc"};
    StaticQueryToken token_def{"def"};
    StaticQueryToken another_token_abc{"abc"};

    SECTION("less_than_operator") {
        REQUIRE(empty_token < token_abc);
        REQUIRE(empty_token < token_def);
        REQUIRE(token_abc < token_def);
        REQUIRE_FALSE(token_abc < empty_token);
        REQUIRE_FALSE(token_def < empty_token);
        REQUIRE_FALSE(token_def < token_abc);
        // False for same value
        REQUIRE_FALSE(token_abc < another_token_abc);
    }

    SECTION("greater_than_operator") {
        REQUIRE(token_abc > empty_token);
        REQUIRE(token_def > empty_token);
        REQUIRE(token_def > token_abc);
        REQUIRE_FALSE(empty_token > token_abc);
        REQUIRE_FALSE(empty_token > token_def);
        REQUIRE_FALSE(token_abc > token_def);
        // False for same value
        REQUIRE_FALSE(token_abc > another_token_abc);
    }
}
