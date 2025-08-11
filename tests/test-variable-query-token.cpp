#include <cstdint>
#include <vector>

#include <log_surgeon/wildcard_query_parser/VariableQueryToken.hpp>

#include <catch2/catch_test_macros.hpp>

#include "comparison_test_utils.hpp"

/**
 * @defgroup unit_tests_variable_query_token `VariableQueryToken` unit tests.
 * @brief Unit tests for `VariableQueryToken` construction and comparison.

 * These unit tests contain the `VariableQueryToken` tag.
 */

using log_surgeon::tests::pairwise_comparison_of_strictly_ascending_vector;
using log_surgeon::tests::test_equal;
using log_surgeon::wildcard_query_parser::VariableQueryToken;

/**
 * @ingroup unit_tests_variable_query_token
 * @brief Tests `operator==`, `operator<=>`, and all derived operators.
 */
TEST_CASE("comparison_operators", "[VariableQueryToken]") {
    constexpr uint32_t cEmptyId{0};
    constexpr uint32_t cIntId{2};
    constexpr uint32_t cHasNumId{7};

    std::vector<VariableQueryToken> const ordered_tokens{
            {cEmptyId, "", false},
            {cIntId, "123", false},
            {cIntId, "456", false},
            {cHasNumId, "123", false},
            {cHasNumId, "user123*", true}
    };
    VariableQueryToken const token{cHasNumId, "abc*123", true};
    VariableQueryToken const duplicate_token{cHasNumId, "abc*123", true};

    pairwise_comparison_of_strictly_ascending_vector(ordered_tokens);
    test_equal(token, duplicate_token);
}
