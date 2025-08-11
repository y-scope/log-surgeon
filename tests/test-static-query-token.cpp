#include <vector>

#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>

#include <catch2/catch_test_macros.hpp>

#include "comparison_test_utils.hpp"

/**
 * @defgroup unit_tests_static_query_token `StaticQueryToken` unit tests.
 * @brief Unit tests for `StaticQueryToken` construction, mutation, and comparison.

 * These unit tests contain the `StaticQueryToken` tag.
 */

using log_surgeon::tests::pairwise_comparison_of_strictly_ascending_vector;
using log_surgeon::tests::test_equal;
using log_surgeon::wildcard_query_parser::StaticQueryToken;

/**
 * @ingroup unit_tests_static_query_token
 * @brief Tests `operator<=>` and all derived operators.
 */
TEST_CASE("comparison_operators", "[StaticQueryToken]") {
    std::vector<StaticQueryToken> const ordered_tokens{
            StaticQueryToken{""},
            StaticQueryToken{"abc"},
            StaticQueryToken{"def"}
    };
    StaticQueryToken const token{"ghi"};
    StaticQueryToken const duplicate_token{"ghi"};

    pairwise_comparison_of_strictly_ascending_vector(ordered_tokens);
    test_equal(token, duplicate_token);
}
