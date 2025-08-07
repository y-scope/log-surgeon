#include <vector>

#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>

#include <catch2/catch_message.hpp>
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
    std::vector const ordered_tokens{
            StaticQueryToken{""},
            StaticQueryToken{"abc"},
            StaticQueryToken{"def"}
    };
    StaticQueryToken const token{"ghi"};
    StaticQueryToken const duplicate_token{"ghi"};

    for (size_t i{0}; i < ordered_tokens.size(); i++) {
        CAPTURE(i);
        for (size_t j{0}; j < ordered_tokens.size(); j++) {
            CAPTURE(j);
            if (i < j) {
                test_less_than(ordered_tokens[i], ordered_tokens[j]);
            } else if (i == j) {
                test_equal(ordered_tokens[i], ordered_tokens[j]);
            } else {
                test_greater_than(ordered_tokens[i], ordered_tokens[j]);
            }
        }
    }
    test_equal(token, duplicate_token);
}
