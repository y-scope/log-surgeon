#include <memory>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/Register.hpp>
#include <log_surgeon/finite_automata/Tag.hpp>

using log_surgeon::finite_automata::Register;
using log_surgeon::finite_automata::Tag;
using std::make_unique;
using std::unique_ptr;

TEST_CASE("Register operations", "[Register]") {
    SECTION("Basic tag retrieval works correctly") {
        auto const tag = make_unique<Tag>("uID");
        Register const reg(tag.get());
        REQUIRE(tag.get() == reg.get_tag());
    }
}
