#include <cstddef>
#include <stdexcept>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/PrefixTree.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>

using log_surgeon::finite_automata::RegisterHandler;
using position_t = log_surgeon::finite_automata::PrefixTree::position_t;

constexpr position_t cInitialPos{0};
constexpr position_t cNegativePos1{-1};
constexpr position_t cNegativePos2{-100};
constexpr position_t cSetPos1{5};
constexpr position_t cSetPos2{10};
constexpr position_t cSetPos3{15};
constexpr size_t cNumRegisters{5};
constexpr size_t cRegId1{0};
constexpr size_t cRegId2{1};
constexpr size_t cRegId3{2};
constexpr size_t cInvalidRegId{10};

namespace {
auto add_register_to_handler(RegisterHandler& handler) -> void {
    for (size_t i{0}; i < cNumRegisters; ++i) {
        handler.add_register(i, cInitialPos);
    }
}
}  // namespace

TEST_CASE("`RegisterHandler` tests", "[RegisterHandler]") {
    RegisterHandler handler;

    SECTION("Initial state is empty") {
        REQUIRE_THROWS_AS(handler.get_reversed_positions(cRegId1), std::out_of_range);
    }

    add_register_to_handler(handler);

    SECTION("Set register position correctly") {
        handler.set_register(cRegId1, cSetPos1);
        REQUIRE(std::vector<position_t>{cSetPos1} == handler.get_reversed_positions(cRegId1));
    }

    SECTION("Register relationships are maintained") {
        handler.set_register(cRegId1, cSetPos1);
        handler.set_register(cRegId2, cSetPos2);
        handler.set_register(cRegId3, cSetPos3);

        auto positions{handler.get_reversed_positions(cRegId3)};
        REQUIRE(std::vector<position_t>{cSetPos3, cSetPos2, cSetPos1}
                == handler.get_reversed_positions(cRegId3));
    }

    SECTION("Copy register index correctly") {
        handler.set_register(cRegId1, cSetPos1);
        handler.copy_register(cRegId2, cRegId1);
        REQUIRE(std::vector<position_t>{cSetPos1} == handler.get_reversed_positions(cRegId2));
    }

    SECTION("`append_position` appends position correctly") {
        handler.set_register(cRegId1, cSetPos1);
        handler.append_position(cRegId1, cSetPos2);
        REQUIRE(std::vector<position_t>{cSetPos2, cSetPos1}
                == handler.get_reversed_positions(cRegId1));
    }

    SECTION("Throws out of range correctly") {
        REQUIRE_THROWS_AS(handler.set_register(cInvalidRegId, cSetPos1), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(cInvalidRegId, cRegId2), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(cRegId1, cInvalidRegId), std::out_of_range);
        REQUIRE_THROWS_AS(handler.append_position(cInvalidRegId, cSetPos1), std::out_of_range);
        REQUIRE_THROWS_AS(handler.get_reversed_positions(cInvalidRegId), std::out_of_range);
    }

    SECTION("Handles negative position values correctly") {
        handler.set_register(cRegId1, cNegativePos1);
        handler.append_position(cRegId1, cSetPos1);
        handler.append_position(cRegId1, cNegativePos2);
        REQUIRE(std::vector<position_t>{cNegativePos2, cSetPos1, cNegativePos1}
                == handler.get_reversed_positions(cRegId1));
    }
}
