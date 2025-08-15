#include <cstddef>
#include <string>

#include <log_surgeon/wildcard_query_parser/Expression.hpp>
#include <log_surgeon/wildcard_query_parser/ExpressionView.hpp>

#include <catch2/catch_message.hpp>
#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_expression_view `ExpressionView` unit tests.
 * @brief Unit tests for `ExpressionView` to ... .

 * These unit tests contain the `ExpressionView` tag.
 */

using log_surgeon::wildcard_query_parser::Expression;
using log_surgeon::wildcard_query_parser::ExpressionView;
using std::string;

/**
 * @ingroup unit_tests_expression_view
 * @brief Tests an empty `ExpressionView`.
 */
TEST_CASE("empty_expression_view", "[ExpressionView]") {
    Expression const expression{""};
    ExpressionView const view{expression, 0, 0};

    REQUIRE(view.is_well_formed());
    REQUIRE(view.get_search_string().empty());
    REQUIRE_FALSE(view.starts_or_ends_with_greedy_wildcard());

    auto const [regex_string, contains_wildcard]{view.generate_regex_string()};
    REQUIRE(regex_string.empty());
    REQUIRE_FALSE(contains_wildcard);

    auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
    REQUIRE_FALSE(is_extended);
    REQUIRE(view.get_search_string() == extended_view.get_search_string());
}

/**
 * @ingroup unit_tests_expression_view
 * @brief Tests an `ExpressionView` that captures the entire `Expression`.
 */
TEST_CASE("full_expression_view", "[ExpressionView]") {
    string const input{"abc"};

    Expression const expression{input};
    ExpressionView const view{expression, 0, input.size()};

    REQUIRE(view.is_well_formed());
    REQUIRE(input == view.get_search_string());
    REQUIRE_FALSE(view.starts_or_ends_with_greedy_wildcard());

    auto const [regex_string, contains_wildcard]{view.generate_regex_string()};
    REQUIRE(input == regex_string);
    REQUIRE_FALSE(contains_wildcard);

    auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
    REQUIRE_FALSE(is_extended);
    REQUIRE(view.get_search_string() == extended_view.get_search_string());
}

/**
 * @ingroup unit_tests_expression_view
 * @brief Tests an `ExpressionView` that captures a subrange of `Expression` with wildcards.
 */
TEST_CASE("wildcard_subrange_expression_view", "[ExpressionView]") {
    string const input{"a*b?c"};

    constexpr size_t cBeginPos{1};
    constexpr size_t cEndPos{4};
    string const expected_search_string{"*b?"};
    string const expected_regex_string{".*b."};

    Expression const expression{input};
    ExpressionView const view{expression, cBeginPos, cEndPos};

    REQUIRE(view.is_well_formed());
    REQUIRE(expected_search_string == view.get_search_string());
    REQUIRE(view.starts_or_ends_with_greedy_wildcard());

    auto const [regex_string, contains_wildcard]{view.generate_regex_string()};
    REQUIRE(expected_regex_string == regex_string);
    REQUIRE(contains_wildcard);

    auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
    REQUIRE_FALSE(is_extended);
    REQUIRE(view.get_search_string() == extended_view.get_search_string());
}

/**
 * @ingroup unit_tests_expression_view
 * @brief Tests an `ExpressionView` that captures a subrange of `Expression` with escaped literals.
 */
TEST_CASE("escape_subrange_expression_view", "[ExpressionView]") {
    string const input{R"(a\*b\?c)"};

    constexpr size_t cBeginPos{1};
    constexpr size_t cEndPos{6};
    string const expected_search_string{R"(\*b\?)"};
    string const expected_regex_string{R"(\*b\?)"};

    Expression const expression{input};
    ExpressionView const view{expression, cBeginPos, cEndPos};

    REQUIRE(view.is_well_formed());
    REQUIRE(expected_search_string == view.get_search_string());
    REQUIRE_FALSE(view.starts_or_ends_with_greedy_wildcard());

    auto const [regex_string, contains_wildcard]{view.generate_regex_string()};
    REQUIRE(expected_regex_string == regex_string);
    REQUIRE_FALSE(contains_wildcard);

    auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
    REQUIRE_FALSE(is_extended);
    REQUIRE(view.get_search_string() == extended_view.get_search_string());
}

/**
 * @ingroup unit_tests_expression_view
 * @brief Tests `ExpressionView`s for well-formedness.
 */
TEST_CASE("well_formed_expression_view", "[ExpressionView]") {
    string const input{R"(a\*b\?c)"};
    constexpr size_t cEscapePos1{1};
    constexpr size_t cEscapePos2{4};

    Expression const expression{input};
    for (size_t start_pos{0}; start_pos < input.size(); ++start_pos) {
        for (size_t end_pos{start_pos + 1}; end_pos <= input.size(); ++end_pos) {
            ExpressionView const view{expression, start_pos, end_pos};
            CAPTURE(start_pos);
            CAPTURE(end_pos);
            if (start_pos == cEscapePos1 + 1 || start_pos == cEscapePos2 + 1) {
                REQUIRE_FALSE(view.is_well_formed());
            } else if (end_pos == cEscapePos1 + 1 || end_pos == cEscapePos2 + 1) {
                REQUIRE_FALSE(view.is_well_formed());
            } else {
                REQUIRE(view.is_well_formed());
            }
        }
    }
}
