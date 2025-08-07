#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>

#include <catch2/catch_test_macros.hpp>

#include "comparison_test_utils.hpp"

/**
 * @defgroup unit_tests_static_query_token `StaticQueryToken` unit tests.
 * @brief Unit tests for `StaticQueryToken` construction, mutation, and comparison.

 * These unit tests contain the `StaticQueryToken` tag.
 */

using log_surgeon::tests::test_equal;
using log_surgeon::tests::test_greater_than;
using log_surgeon::tests::test_less_than;
using log_surgeon::wildcard_query_parser::StaticQueryToken;

/**
 * @ingroup unit_tests_static_query_token
 * @brief Tests `operator<` and `operator>`.
 */
TEST_CASE("three_way_and_derived_comparisons", "[StaticQueryToken]") {
    StaticQueryToken const empty_token{""};
    StaticQueryToken const token_abc{"abc"};
    StaticQueryToken const token_def{"def"};
    StaticQueryToken const another_token_def{"def"};

    // empty_token
    test_equal(empty_token, empty_token);
    test_less_than(empty_token, token_abc);
    test_less_than(empty_token, token_def);

    // token_abc
    test_greater_than(token_abc, empty_token);
    test_equal(token_abc, token_abc);
    test_less_than(token_abc, token_def);

    // token_def
    test_greater_than(token_def, empty_token);
    test_greater_than(token_def, token_abc);
    test_equal(token_def, token_def);
    test_equal(token_def, another_token_def);
}
