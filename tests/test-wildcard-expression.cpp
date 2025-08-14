#include <string>

#include <log_surgeon/wildcard_query_parser/WildcardExpression.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_wildcard_expression `WildcardExpression` unit tests.
 * @brief Unit tests for `WildcardExpression` to verify storage and type predicate methods.

 * These unit tests contain the `WildcardExpression` tag.
 */

using log_surgeon::wildcard_query_parser::WildcardExpression;
using std::string;

/**
 * @ingroup unit_tests_wildcard_expression
 * @brief Tests an empty `WildcardExpression`.
 */
TEST_CASE("empty_wildcard_expression", "[WildcardExpression]") {
    WildcardExpression const expression{""};
    REQUIRE(expression.get_search_string().empty());
    REQUIRE(expression.get_chars().empty());
}

/**
 * @ingroup unit_tests_wildcard_expression
 * @brief Tests a `WildcardExpression` with only normal characters.
 */
TEST_CASE("normal_character_wildcard_expression", "[WildcardExpression]") {
    string const input{"abc"};

    WildcardExpression const expression{input};
    REQUIRE(input == expression.get_search_string());

    auto const& expression_chars{expression.get_chars()};
    REQUIRE(input.size() == expression_chars.size());

    for (size_t i{0}; i < expression_chars.size(); i++) {
        auto const& expression_char{expression_chars[i]};
        REQUIRE(input[i] == expression_char.value());
        REQUIRE_FALSE(expression_char.is_greedy_wildcard());
        REQUIRE_FALSE(expression_char.is_non_greedy_wildcard());
        REQUIRE_FALSE(expression_char.is_escape());
    }
}

/**
 * @ingroup unit_tests_wildcard_expression
 * @brief Tests a `WildcardExpression` with mixed normal and wildcard characters.
 */
TEST_CASE("normal_and_wildcard_character_wildcard_expression", "[WildcardExpression]") {
    string const input{"a*b?c"};

    WildcardExpression const expression{input};
    REQUIRE(input == expression.get_search_string());

    auto const& expression_chars{expression.get_chars()};
    REQUIRE(input.size() == expression_chars.size());

    for (size_t i{0}; i < expression_chars.size(); i++) {
        auto const& expression_char{expression_chars[i]};
        REQUIRE(input[i] == expression_char.value());
        if (0 == i || 2 == i || 4 == i) {
            REQUIRE_FALSE(expression_char.is_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_non_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_escape());
        } else if (1 == i) {
            REQUIRE(expression_char.is_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_non_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_escape());
        } else {
            REQUIRE_FALSE(expression_char.is_greedy_wildcard());
            REQUIRE(expression_char.is_non_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_escape());
        }
    }
}

/**
 * @ingroup unit_tests_wildcard_expression
 * @brief Tests a `WildcardExpression` with mixed normal and escape characters.
 */
TEST_CASE("normal_and_escape_character_wildcard_expression", "[WildcardExpression]") {
    string const input{R"(a\*b\?c\\)"};
    constexpr size_t cFirstEscapePos{1};
    constexpr size_t cSecondEscapePos{4};
    constexpr size_t cLastEscapePos{7};

    WildcardExpression const expression{input};
    REQUIRE(input == expression.get_search_string());

    auto const& expression_chars{expression.get_chars()};
    REQUIRE(input.size() == expression_chars.size());

    for (size_t i{0}; i < expression_chars.size(); i++) {
        auto const& expression_char{expression_chars[i]};
        REQUIRE(input[i] == expression_char.value());
        if (cFirstEscapePos == i || cSecondEscapePos == i || cLastEscapePos == i) {
            REQUIRE_FALSE(expression_char.is_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_non_greedy_wildcard());
            REQUIRE(expression_char.is_escape());
        } else {
            REQUIRE_FALSE(expression_char.is_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_non_greedy_wildcard());
            REQUIRE_FALSE(expression_char.is_escape());
        }
    }
}
