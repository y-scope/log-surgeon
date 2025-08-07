#ifndef LOG_SURGEON_TESTS_COMPARISON_TEST_UTILS_HPP
#define LOG_SURGEON_TESTS_COMPARISON_TEST_UTILS_HPP

#include <compare>

#include <catch2/catch_test_macros.hpp>

using std::strong_ordering;

namespace log_surgeon::tests {
/**
 * Tests comparison operators when `lhs` == `rhs`.
 * @param lhs Value on the lhs of the operator.
 * @param rhs Value on the rhs of the operator.
 */
template <typename T>
auto test_equal(T const& lhs, T const& rhs) -> void;

/**
 * Tests comparison operators when `lhs` > `rhs`.
 * @param lhs Value on the lhs of the operator.
 * @param rhs Value on the rhs of the operator.
 */
template <typename T>
auto test_greater_than(T const& lhs, T const& rhs) -> void;

/**
 * Tests comparison operators when `lhs` < `rhs`.
 * @param lhs Value on the lhs of the operator.
 * @param rhs Value on the rhs of the operator.
 */
template <typename T>
auto test_less_than(T const& lhs, T const& rhs) -> void;

template <typename T>
auto test_equal(T const& lhs, T const& rhs) -> void {
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

template <typename T>
auto test_greater_than(T const& lhs, T const& rhs) -> void {
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

template <typename T>
auto test_less_than(T const& lhs, T const& rhs) -> void {
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
}  // namespace log_surgeon::tests

#endif  // LOG_SURGEON_TESTS_COMPARISON_TEST_UTILS_HPP
