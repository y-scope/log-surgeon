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
 * @brief Tests bound clamping during `ExpressionView`construction.
 *
 * Negative casted values test wrap-around behavior.
 */
TEST_CASE("expression_view_bound_clamping", "[ExpressionView]") {
    string const input{"abcdefg"};
    Expression const expression{input};
    auto constexpr cNegativeValue{-5};
    auto constexpr cLargeValue{1000};
    auto constexpr cMiddlePos{4};

    SECTION("start_after_end") {
        ExpressionView const view{expression, cMiddlePos, cMiddlePos - 1};
        REQUIRE(view.get_search_string().empty());
    }

    SECTION("start_equal_end") {
        ExpressionView const view{expression, cMiddlePos, cMiddlePos};
        REQUIRE(view.get_search_string().empty());
    }

    SECTION("start_beyond_size") {
        ExpressionView const view{expression, cLargeValue, input.size()};
        REQUIRE(view.get_search_string().empty());
    }

    SECTION("end_beyond_size") {
        ExpressionView const view{expression, 0, cLargeValue};
        REQUIRE(input == view.get_search_string());
    }

    SECTION("start_before_zero") {
        ExpressionView const view{expression, static_cast<size_t>(cNegativeValue), input.size()};
        REQUIRE(view.get_search_string().empty());
    }

    SECTION("end_before_zero") {
        ExpressionView const view{expression, 0, static_cast<size_t>(cNegativeValue)};
        REQUIRE(input == view.get_search_string());
    }

    SECTION("start_before_zero_and_end_beyond_size") {
        ExpressionView const view{expression, static_cast<size_t>(cNegativeValue), cLargeValue};
        REQUIRE(view.get_search_string().empty());
    }

    SECTION("start_beyond_size_and_end_before_zero") {
        ExpressionView const view{expression, cLargeValue, static_cast<size_t>(cNegativeValue)};
        REQUIRE(view.get_search_string().empty());
    }
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

/**
 * @ingroup unit_tests_expression_view
 * @brief Tests `ExpressionView`s for flanking greedy wildcards.
 */
TEST_CASE("expression_view_starting_or_ending_with_greedy_wildcard", "[ExpressionView]") {
    SECTION("starts_with_greedy_wildcard") {
        string const input{"*abc"};
        Expression const expression{input};
        ExpressionView const view{expression, 0, input.size()};
        REQUIRE(view.starts_or_ends_with_greedy_wildcard());
    }

    SECTION("ends_with_greedy_wildcard") {
        string const input{"abc*"};
        Expression const expression{input};
        ExpressionView const view{expression, 0, input.size()};
        REQUIRE(view.starts_or_ends_with_greedy_wildcard());
    }

    SECTION("starts_and_ends_with_greedy_wildcard") {
        string const input{"*abc*"};
        Expression const expression{input};
        ExpressionView const view{expression, 0, input.size()};
        REQUIRE(view.starts_or_ends_with_greedy_wildcard());
    }

    SECTION("no_flanking_greedy_wildcard") {
        string const input{"a*b"};
        Expression const expression{input};
        ExpressionView const view{expression, 0, input.size()};
        REQUIRE_FALSE(view.starts_or_ends_with_greedy_wildcard());
    }
}

/**
 * @ingroup unit_tests_expression_view
 * @brief Tests extending `ExpressionView` to include adjacent greedy wildcards.
 */
TEST_CASE("extend_expression_view_to_adjacent_greedy_wildcards", "[ExpressionView]") {
    SECTION("prefix_greedy_wildcard") {
        string const input{"*abc?"};
        string const expected_extended_string{"*abc"};

        Expression const expression{input};
        ExpressionView const view{expression, 1, input.size() - 1};
        auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
        REQUIRE(is_extended);
        REQUIRE(expected_extended_string == extended_view.get_search_string());
    }

    SECTION("suffix_greedy_wildcard") {
        string const input{"?abc*"};
        string const expected_extended_string{"abc*"};

        Expression const expression{input};
        ExpressionView const view{expression, 1, input.size() - 1};
        auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
        REQUIRE(is_extended);
        REQUIRE(expected_extended_string == extended_view.get_search_string());
    }

    SECTION("suffix_and_prefix_greedy_wildcard") {
        string const input{"*a?c*"};
        string const expected_extended_string{"*a?c*"};

        Expression const expression{input};
        ExpressionView const view{expression, 1, input.size() - 1};
        auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
        REQUIRE(is_extended);
        REQUIRE(expected_extended_string == extended_view.get_search_string());
    }

    SECTION("no_extension") {
        string const input{"?a*c?"};
        string const expected_extended_string{"a*c"};

        Expression const expression{input};
        ExpressionView const view{expression, 1, input.size() - 1};
        auto const [is_extended, extended_view]{view.extend_to_adjacent_greedy_wildcards()};
        REQUIRE_FALSE(is_extended);
        REQUIRE(expected_extended_string == extended_view.get_search_string());
    }
}
