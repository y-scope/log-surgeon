#include <string>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/Tag.hpp>

using log_surgeon::finite_automata::Tag;
using std::string;

TEST_CASE("Tag operations", "[Tag]") {
    SECTION("Basic name retrieval works correctly") {
        Tag const tag{"uID"};
        REQUIRE("uID" == string{tag.get_name()});
    }

    SECTION("Empty tag name is handled correctly") {
        Tag const empty_tag{""};
        REQUIRE(empty_tag.get_name().empty());
    }

    SECTION("Special characters in tag names are preserved") {
        Tag const special_tag{"user.id-123_@"};
        REQUIRE("user.id-123_@" == string{special_tag.get_name()});
    }

    SECTION("Move semantics work correctly") {
        Tag original_tag{"source"};
        Tag moved_tag{std::move(original_tag)};
        REQUIRE("source" == string{moved_tag.get_name()});

        Tag assign_tag{"target"};
        assign_tag = Tag{"new_source"};
        REQUIRE("new_source" == string{assign_tag.get_name()});
    }
}
