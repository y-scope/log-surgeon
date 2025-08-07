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
 * @brief Tests `operator==`, `operator<=>`, and all derived operators.
 */
TEST_CASE("comparison_operators", "[VariableQueryToken]") {
    constexpr uint32_t cEmptyId{0};
    constexpr uint32_t cIntId{2};
    constexpr uint32_t cHasNumId{7};

    VariableQueryToken const empty_token{cEmptyId, "", false};
    VariableQueryToken const int_123_token{cIntId, "123", false};
    VariableQueryToken const int_456_token{cIntId, "456", false};
    VariableQueryToken const has_number_123_token{cHasNumId, "123", false};
    VariableQueryToken const has_number_user123_wildcard_token{cHasNumId, "user123*", true};
    VariableQueryToken const another_has_number_user123_wildcard_token{cHasNumId, "user123*", true};

    // empty_token
    test_equal(empty_token, empty_token);
    test_less_than(empty_token, int_123_token);
    test_less_than(empty_token, int_456_token);
    test_less_than(empty_token, has_number_123_token);
    test_less_than(empty_token, has_number_user123_wildcard_token);

    // int_123_token
    test_greater_than(int_123_token, empty_token);
    test_equal(int_123_token, int_123_token);
    test_less_than(int_123_token, int_456_token);
    test_less_than(int_123_token, has_number_123_token);
    test_less_than(int_123_token, has_number_user123_wildcard_token);

    // int_456_token
    test_greater_than(int_456_token, empty_token);
    test_greater_than(int_456_token, int_123_token);
    test_equal(int_456_token, int_456_token);
    test_less_than(int_456_token, has_number_123_token);
    test_less_than(int_456_token, has_number_user123_wildcard_token);

    // has_number_123_token
    test_greater_than(has_number_123_token, empty_token);
    test_greater_than(has_number_123_token, int_123_token);
    test_greater_than(has_number_123_token, int_456_token);
    test_equal(has_number_123_token, has_number_123_token);
    test_less_than(has_number_123_token, has_number_user123_wildcard_token);

    // has_number_user123_wildcard_token
    test_greater_than(has_number_user123_wildcard_token, empty_token);
    test_greater_than(has_number_user123_wildcard_token, int_123_token);
    test_greater_than(has_number_user123_wildcard_token, int_456_token);
    test_greater_than(has_number_user123_wildcard_token, has_number_123_token);
    test_equal(has_number_user123_wildcard_token, has_number_user123_wildcard_token);
    test_equal(has_number_user123_wildcard_token, another_has_number_user123_wildcard_token);
}
