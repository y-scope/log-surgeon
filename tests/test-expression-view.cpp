#include <log_surgeon/wildcard_query_parser/Expression.hpp>
#include <log_surgeon/wildcard_query_parser/ExpressionView.hpp>

#include <catch2/catch_test_macros.hpp>

/**
 * @defgroup unit_tests_expression_view `ExpressionView` unit tests.
 * @brief Unit tests for `ExpressionView` to ... .

 * These unit tests contain the `ExpressionView` tag.
 */

using log_surgeon::wildcard_query_parser::Expression;
using log_surgeon::wildcard_query_parser::ExpressionView;

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
