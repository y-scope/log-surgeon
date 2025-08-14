#include <log_surgeon/wildcard_query_parser/WildcardCharacter.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_wildcard_character `WildcardCharacter` unit tests.
 * @brief Unit tests for `WildcardCharacter` to verify storage and type predicate methods.

 * These unit tests contain the `WildcardCharacter` tag.
 */

using log_surgeon::wildcard_query_parser::WildcardCharacter;

/**
 * @ingroup unit_tests_wildcard_character
 * @brief Tests a `WildcardCharacter` that stores a normal character.
 */
TEST_CASE("normal", "[WildcardCharacter]") {
    WildcardCharacter const wildcard_character{'a', WildcardCharacter::Type::Normal};
    REQUIRE('a' == wildcard_character.value());
    REQUIRE_FALSE(wildcard_character.is_greedy_wildcard());
    REQUIRE_FALSE(wildcard_character.is_non_greedy_wildcard());
    REQUIRE_FALSE(wildcard_character.is_escape());
}

/**
 * @ingroup unit_tests_wildcard_character
 * @brief Tests a `WildcardCharacter` that stores a greedy wildcard.
 */
TEST_CASE("greedy_wildcard", "[WildcardCharacter]") {
    WildcardCharacter const wildcard_character{'*', WildcardCharacter::Type::GreedyWildcard};
    REQUIRE('*' == wildcard_character.value());
    REQUIRE(wildcard_character.is_greedy_wildcard());
    REQUIRE_FALSE(wildcard_character.is_non_greedy_wildcard());
    REQUIRE_FALSE(wildcard_character.is_escape());
}

/**
 * @ingroup unit_tests_wildcard_character
 * @brief Tests a `WildcardCharacter` that stores a non-greedy wildcard.
 */
TEST_CASE("non_greedy_wildcard", "[WildcardCharacter]") {
    WildcardCharacter const wildcard_character{'?', WildcardCharacter::Type::NonGreedyWildcard};
    REQUIRE('?' == wildcard_character.value());
    REQUIRE_FALSE(wildcard_character.is_greedy_wildcard());
    REQUIRE(wildcard_character.is_non_greedy_wildcard());
    REQUIRE_FALSE(wildcard_character.is_escape());
}

/**
 * @ingroup unit_tests_wildcard_character
 * @brief Tests a `WildcardCharacter` that stores an escape.
 */
TEST_CASE("escape", "[WildcardCharacter]") {
    WildcardCharacter const wildcard_character{'\\', WildcardCharacter::Type::Escape};
    REQUIRE('\\' == wildcard_character.value());
    REQUIRE_FALSE(wildcard_character.is_greedy_wildcard());
    REQUIRE_FALSE(wildcard_character.is_non_greedy_wildcard());
    REQUIRE(wildcard_character.is_escape());
}
