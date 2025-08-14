#include <log_surgeon/wildcard_query_parser/ExpressionCharacter.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_expression_character `ExpressionCharacter` unit tests.
 * @brief Unit tests for `ExpressionCharacter` to verify storage and type predicate methods.

 * These unit tests contain the `ExpressionCharacter` tag.
 */

using log_surgeon::wildcard_query_parser::ExpressionCharacter;

/**
 * @ingroup unit_tests_expression_character
 * @brief Tests a `ExpressionCharacter` that stores a normal character.
 */
TEST_CASE("normal_expression_character", "[ExpressionCharacter]") {
    ExpressionCharacter const expression_character{'a', ExpressionCharacter::Type::Normal};
    REQUIRE('a' == expression_character.value());
    REQUIRE_FALSE(expression_character.is_greedy_wildcard());
    REQUIRE_FALSE(expression_character.is_non_greedy_wildcard());
    REQUIRE_FALSE(expression_character.is_escape());
}

/**
 * @ingroup unit_tests_expression_character
 * @brief Tests a `ExpressionCharacter` that stores a greedy wildcard.
 */
TEST_CASE("greedy_wildcard_expression_character", "[ExpressionCharacter]") {
    ExpressionCharacter const expression_character{'*', ExpressionCharacter::Type::GreedyWildcard};
    REQUIRE('*' == expression_character.value());
    REQUIRE(expression_character.is_greedy_wildcard());
    REQUIRE_FALSE(expression_character.is_non_greedy_wildcard());
    REQUIRE_FALSE(expression_character.is_escape());
}

/**
 * @ingroup unit_tests_expression_character
 * @brief Tests a `ExpressionCharacter` that stores a non-greedy wildcard.
 */
TEST_CASE("non_greedy_wildcard_expression_character", "[ExpressionCharacter]") {
    ExpressionCharacter const expression_character{
            '?',
            ExpressionCharacter::Type::NonGreedyWildcard
    };
    REQUIRE('?' == expression_character.value());
    REQUIRE_FALSE(expression_character.is_greedy_wildcard());
    REQUIRE(expression_character.is_non_greedy_wildcard());
    REQUIRE_FALSE(expression_character.is_escape());
}

/**
 * @ingroup unit_tests_expression_character
 * @brief Tests a `ExpressionCharacter` that stores an escape.
 */
TEST_CASE("escape_expression_character", "[ExpressionCharacter]") {
    ExpressionCharacter const expression_character{'\\', ExpressionCharacter::Type::Escape};
    REQUIRE('\\' == expression_character.value());
    REQUIRE_FALSE(expression_character.is_greedy_wildcard());
    REQUIRE_FALSE(expression_character.is_non_greedy_wildcard());
    REQUIRE(expression_character.is_escape());
}
