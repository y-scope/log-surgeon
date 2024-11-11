#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/Tag.hpp>

using log_surgeon::finite_automata::Tag;

TEST_CASE("Tag operations", "[Tag]") {
    SECTION("Basic name retrieval works correctly") {
        Tag const tag("uID");
        REQUIRE("uID" == tag.get_name());
    }

    SECTION("Empty tag name is handled correctly") {
        Tag const empty_tag("");
        REQUIRE(empty_tag.get_name().empty());
    }

    SECTION("Special characters in tag names are preserved") {
        Tag const special_tag("user.id-123_@");
        REQUIRE("user.id-123_@" == special_tag.get_name());
    }
}
