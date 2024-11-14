#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/Tag.hpp>

using log_surgeon::finite_automata::Tag;

    TEST_CASE("Test Tag class", "[Tag]") {
        Tag const tag("uID");
        REQUIRE("uID" == tag.get_name());
    }
