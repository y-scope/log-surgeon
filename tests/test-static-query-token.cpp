#include <compare>

#include <log_surgeon/wildcard_query_parser/StaticQueryToken.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_static_query_token `StaticQueryToken` unit tests.
 * @brief Unit tests for `StaticQueryToken` construction, mutation, and comparison.

 * These unit tests contain the `StaticQueryToken` tag.
 */

using log_surgeon::wildcard_query_parser::StaticQueryToken;
using std::strong_ordering;

namespace {
/**
 * Tests comparison operators when `lhs` == `rhs`.
 * @param lhs `StaticQueryToken` on the lhs of the operator.
 * @param rhs `StaticQueryToken` on the rhs of the operator.
 */
auto test_equal(StaticQueryToken const& lhs, StaticQueryToken const& rhs) -> void;

/**
 * Tests comparison operators when `lhs` > `rhs`.
 * @param lhs `StaticQueryToken` on the lhs of the operator.
 * @param rhs `StaticQueryToken` on the rhs of the operator.
 */
auto test_greater_than(StaticQueryToken const& lhs, StaticQueryToken const& rhs) -> void;

/**
 * Tests comparison operators when `lhs` < `rhs`.
 * @param lhs `StaticQueryToken` on the lhs of the operator.
 * @param rhs `StaticQueryToken` on the rhs of the operator.
 */
auto test_less_than(StaticQueryToken const& lhs, StaticQueryToken const& rhs) -> void;

auto test_equal(StaticQueryToken const& lhs, StaticQueryToken const& rhs) -> void {
    REQUIRE((lhs <=> rhs) == strong_ordering::equal);
    REQUIRE(lhs == rhs);
    REQUIRE(lhs <= rhs);
    REQUIRE(lhs >= rhs);
    REQUIRE(rhs == lhs);
    REQUIRE(rhs <= lhs);
    REQUIRE(rhs >= lhs);

    REQUIRE_FALSE(lhs != rhs);
    REQUIRE_FALSE(lhs < rhs);
    REQUIRE_FALSE(lhs > rhs);
    REQUIRE_FALSE(rhs != lhs);
    REQUIRE_FALSE(rhs < lhs);
    REQUIRE_FALSE(rhs > lhs);
}

auto test_greater_than(StaticQueryToken const& lhs, StaticQueryToken const& rhs) -> void {
    REQUIRE((lhs <=> rhs) == strong_ordering::greater);
    REQUIRE(lhs != rhs);
    REQUIRE(lhs >= rhs);
    REQUIRE(lhs > rhs);
    REQUIRE(rhs != lhs);
    REQUIRE(rhs <= lhs);
    REQUIRE(rhs < lhs);

    REQUIRE_FALSE(lhs == rhs);
    REQUIRE_FALSE(lhs <= rhs);
    REQUIRE_FALSE(lhs < rhs);
    REQUIRE_FALSE(rhs == lhs);
    REQUIRE_FALSE(rhs >= lhs);
    REQUIRE_FALSE(rhs > lhs);
}

auto test_less_than(StaticQueryToken const& lhs, StaticQueryToken const& rhs) -> void {
    REQUIRE((lhs <=> rhs) == strong_ordering::less);
    REQUIRE(lhs != rhs);
    REQUIRE(lhs <= rhs);
    REQUIRE(lhs < rhs);
    REQUIRE(rhs != lhs);
    REQUIRE(rhs >= lhs);
    REQUIRE(rhs > lhs);

    REQUIRE_FALSE(lhs == rhs);
    REQUIRE_FALSE(lhs >= rhs);
    REQUIRE_FALSE(lhs > rhs);
    REQUIRE_FALSE(rhs == lhs);
    REQUIRE_FALSE(rhs <= lhs);
    REQUIRE_FALSE(rhs < lhs);
}
}  // namespace

/**
 * @ingroup unit_tests_static_query_token
 * @brief Tests `operator<` and `operator>`.
 */
TEST_CASE("three_way_and_derived_comparisons", "[StaticQueryToken]") {
    StaticQueryToken const empty_token{""};
    StaticQueryToken const token_abc{"abc"};
    StaticQueryToken const token_def{"def"};
    StaticQueryToken const another_token_abc{"abc"};

    // empty_token
    test_equal(empty_token, empty_token);
    test_less_than(empty_token, token_abc);
    test_less_than(empty_token, token_def);

    // token_abc
    test_greater_than(token_abc, empty_token);
    test_equal(token_abc, token_abc);
    test_less_than(token_abc, token_def);
    test_equal(token_abc, another_token_abc);

    // token_def
    test_greater_than(token_def, empty_token);
    test_greater_than(token_def, token_abc);
    test_equal(token_def, token_def);
}
