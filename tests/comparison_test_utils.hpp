#ifndef LOG_SURGEON_TESTS_COMPARISON_TEST_UTILS_HPP
#define LOG_SURGEON_TESTS_COMPARISON_TEST_UTILS_HPP

#include <compare>
#include <cstddef>
#include <vector>

#include <catch2/catch_message.hpp>
#include <catch2/catch_test_macros.hpp>

namespace log_surgeon::tests {
template <typename T>
concept StronglyThreeWayComparable = std::three_way_comparable<T, std::strong_ordering>;

/**
 * Tests comparison operators when `lhs` == `rhs`.
 * @param lhs Value on the lhs of the operator.
 * @param rhs Value on the rhs of the operator.
 */
template <StronglyThreeWayComparable T>
auto test_equal(T const& lhs, T const& rhs) -> void;

/**
 * Tests comparison operators when `lhs` > `rhs`.
 * @param lhs Value on the lhs of the operator.
 * @param rhs Value on the rhs of the operator.
 */
template <StronglyThreeWayComparable T>
auto test_greater_than(T const& lhs, T const& rhs) -> void;

/**
 * Tests comparison operators when `lhs` < `rhs`.
 * @param lhs Value on the lhs of the operator.
 * @param rhs Value on the rhs of the operator.
 */
template <StronglyThreeWayComparable T>
auto test_less_than(T const& lhs, T const& rhs) -> void;

/**
 * Tests operators `<=>`, `==`, `!=`, `<`, `<=`, `>`, `>=` for every pair of elements in the vector.
 * @param ordered_vector Vector where elements are ordered to be strictly ascending.
 */
template <StronglyThreeWayComparable T>
auto pairwise_comparison_of_strictly_ascending_vector(std::vector<T> const& ordered_vector) -> void;

template <StronglyThreeWayComparable T>
auto test_equal(T const& lhs, T const& rhs) -> void {
    REQUIRE((lhs <=> rhs) == std::strong_ordering::equal);
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

template <StronglyThreeWayComparable T>
auto test_greater_than(T const& lhs, T const& rhs) -> void {
    REQUIRE((lhs <=> rhs) == std::strong_ordering::greater);
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

template <StronglyThreeWayComparable T>
auto test_less_than(T const& lhs, T const& rhs) -> void {
    REQUIRE((lhs <=> rhs) == std::strong_ordering::less);
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

template <StronglyThreeWayComparable T>
auto pairwise_comparison_of_strictly_ascending_vector(std::vector<T> const& ordered_vector)
        -> void {
    for (size_t i{0}; i < ordered_vector.size(); i++) {
        CAPTURE(i);
        for (size_t j{0}; j < ordered_vector.size(); j++) {
            CAPTURE(j);
            if (i < j) {
                test_less_than(ordered_vector[i], ordered_vector[j]);
            } else if (i == j) {
                test_equal(ordered_vector[i], ordered_vector[j]);
            } else {
                test_greater_than(ordered_vector[i], ordered_vector[j]);
            }
        }
    }
}
}  // namespace log_surgeon::tests

#endif  // LOG_SURGEON_TESTS_COMPARISON_TEST_UTILS_HPP
