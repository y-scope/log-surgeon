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
 * @brief Tests `operator<=>` and all derived operators.
 */
TEST_CASE("comparison_operators", "[StaticQueryToken]") {
    StaticQueryToken const empty_token{""};
    StaticQueryToken const abc_token{"abc"};
    StaticQueryToken const def_token{"def"};
    StaticQueryToken const another_def_token{"def"};

    // empty_token
    test_equal(empty_token, empty_token);
    test_less_than(empty_token, abc_token);
    test_less_than(empty_token, def_token);

    // abc_token
    test_greater_than(abc_token, empty_token);
    test_equal(abc_token, abc_token);
    test_less_than(abc_token, def_token);

    // def_token
    test_greater_than(def_token, empty_token);
    test_greater_than(def_token, abc_token);
    test_equal(def_token, def_token);
    test_equal(def_token, another_def_token);
}
