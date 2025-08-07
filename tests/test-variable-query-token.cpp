#include <cstdint>

#include <log_surgeon/wildcard_query_parser/VariableQueryToken.hpp>

#include <catch2/catch_test_macros.hpp>

#include "comparison_test_utils.hpp"

/**
 * @defgroup unit_tests_variable_query_token `VariableQueryToken` unit tests.
 * @brief Unit tests for `VariableQueryToken` construction and comparison.

 * These unit tests contain the `VariableQueryToken` tag.
 */

using log_surgeon::tests::test_equal;
using log_surgeon::tests::test_greater_than;
using log_surgeon::tests::test_less_than;
using log_surgeon::wildcard_query_parser::VariableQueryToken;

/**
 * @ingroup unit_tests_variable_query_token
 * @brief Tests `operator<` and `operator>`.
 */
TEST_CASE("comparison_operators", "[VariableQueryToken]") {
    constexpr uint32_t cEmptyId{0};
    constexpr uint32_t cIntId{2};
    constexpr uint32_t cHasNumId{7};

    VariableQueryToken const empty_token{cEmptyId, "", false};
    VariableQueryToken const token_int_123{cIntId, "123", false};
    VariableQueryToken const token_int_456{cIntId, "456", false};
    VariableQueryToken const token_has_number_123{cHasNumId, "123", false};
    VariableQueryToken const token_has_number_user123_wildcard{cHasNumId, "user123*", true};
    VariableQueryToken const another_token_has_number_user123_wildcard{cHasNumId, "user123*", true};

    // empty_token
    test_equal(empty_token, empty_token);
    test_less_than(empty_token, token_int_123);
    test_less_than(empty_token, token_int_456);
    test_less_than(empty_token, token_has_number_123);
    test_less_than(empty_token, token_has_number_user123_wildcard);

    // token_int_123
    test_greater_than(token_int_123, empty_token);
    test_equal(token_int_123, token_int_123);
    test_less_than(token_int_123, token_int_456);
    test_less_than(token_int_123, token_has_number_123);
    test_less_than(token_int_123, token_has_number_user123_wildcard);

    // token_int_456
    test_greater_than(token_int_456, empty_token);
    test_greater_than(token_int_456, token_int_123);
    test_equal(token_int_456, token_int_456);
    test_less_than(token_int_456, token_has_number_123);
    test_less_than(token_int_456, token_has_number_user123_wildcard);

    // token_has_number_123
    test_greater_than(token_has_number_123, empty_token);
    test_greater_than(token_has_number_123, token_int_123);
    test_greater_than(token_has_number_123, token_int_456);
    test_equal(token_has_number_123, token_has_number_123);
    test_less_than(token_has_number_123, token_has_number_user123_wildcard);

    // token_has_number_user123_wildcard
    test_greater_than(token_has_number_user123_wildcard, empty_token);
    test_greater_than(token_has_number_user123_wildcard, token_int_123);
    test_greater_than(token_has_number_user123_wildcard, token_int_456);
    test_greater_than(token_has_number_user123_wildcard, token_has_number_123);
    test_equal(token_has_number_user123_wildcard, token_has_number_user123_wildcard);
    test_equal(token_has_number_user123_wildcard, another_token_has_number_user123_wildcard);
}
