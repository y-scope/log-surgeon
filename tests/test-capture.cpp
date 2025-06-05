#include <utility>

#include <log_surgeon/finite_automata/Capture.hpp>

#include <catch2/catch_test_macros.hpp>

using log_surgeon::finite_automata::Capture;

TEST_CASE("Capture operations", "[Capture]") {
    SECTION("Basic name retrieval works correctly") {
        Capture const capture{"uID"};
        REQUIRE("uID" == capture.get_name());
    }

    SECTION("Empty capture name is handled correctly") {
        Capture const empty_capture{""};
        REQUIRE(empty_capture.get_name().empty());
    }

    SECTION("Special characters in capture names are preserved") {
        Capture const special_capture{"user.id-123_@"};
        REQUIRE("user.id-123_@" == special_capture.get_name());
    }

    SECTION("Copy constructor works correctly") {
        Capture assign_capture{"target"};
        assign_capture = Capture{"new_source"};
        REQUIRE("new_source" == assign_capture.get_name());
    }

    SECTION("Move constructor works correctly") {
        Capture original_capture{"source"};
        Capture const moved_capture{std::move(original_capture)};
        REQUIRE("source" == moved_capture.get_name());
    }
}
