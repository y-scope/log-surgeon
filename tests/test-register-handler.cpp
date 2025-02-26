#include <cstddef>
#include <stdexcept>
#include <vector>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/PrefixTree.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>

using log_surgeon::finite_automata::RegisterHandler;
using position_t = log_surgeon::finite_automata::PrefixTree::position_t;

namespace {
/**
 * @param num_registers The number of registers managed by the handler.
 * @return The newly initialized register handler.
 */
[[nodiscard]] auto handler_init(size_t num_registers) -> RegisterHandler;

auto handler_init(size_t const num_registers) -> RegisterHandler {
    RegisterHandler handler;
    if (0 == num_registers) {
        return handler;
    }

    handler.add_register();
    for (size_t i{1}; i < num_registers; ++i) {
        handler.add_register(i);
    }
    return handler;
}
}  // namespace

TEST_CASE("`RegisterHandler` tests", "[RegisterHandler]") {
    constexpr position_t cInitialPos1{5};
    constexpr size_t cNumRegisters{5};
    constexpr size_t cRegId1{0};
    constexpr size_t cRegId2{1};

    SECTION("Initial state is empty") {
        RegisterHandler const empty_handler{handler_init(0)};
        REQUIRE_THROWS_AS(empty_handler.get_reversed_positions(cRegId1), std::out_of_range);
    }

    RegisterHandler handler{handler_init(cNumRegisters)};

    SECTION("Set register position correctly") {
        handler.set_register(cRegId1, cInitialPos1);
        REQUIRE(std::vector<position_t>{cInitialPos1} == handler.get_reversed_positions(cRegId1));
    }

    SECTION("Register relationships are maintained") {
        constexpr position_t cInitialPos2{10};
        constexpr position_t cInitialPos3{15};
        constexpr size_t cRegId3{2};

        handler.set_register(cRegId1, cInitialPos1);
        handler.set_register(cRegId2, cInitialPos2);
        handler.set_register(cRegId3, cInitialPos3);

        auto positions{handler.get_reversed_positions(cRegId3)};
        REQUIRE(std::vector<position_t>{cInitialPos3, cInitialPos2, cInitialPos1}
                == handler.get_reversed_positions(cRegId3));
    }

    SECTION("Copy register index correctly") {
        handler.set_register(cRegId1, cInitialPos1);
        handler.copy_register(cRegId2, cRegId1);
        REQUIRE(std::vector<position_t>{cInitialPos1} == handler.get_reversed_positions(cRegId2));
    }

    SECTION("`append_position` appends position correctly") {
        constexpr position_t cAppendPos{10};

        handler.set_register(cRegId1, cInitialPos1);
        handler.append_position(cRegId1, cAppendPos);
        REQUIRE(std::vector<position_t>{cAppendPos, cInitialPos1}
                == handler.get_reversed_positions(cRegId1));
    }

    SECTION("Throws out of range correctly") {
        constexpr size_t cInvalidRegId{10};

        REQUIRE_THROWS_AS(handler.set_register(cInvalidRegId, cInitialPos1), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(cInvalidRegId, cRegId2), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(cRegId1, cInvalidRegId), std::out_of_range);
        REQUIRE_THROWS_AS(handler.append_position(cInvalidRegId, cInitialPos1), std::out_of_range);
        REQUIRE_THROWS_AS(handler.get_reversed_positions(cInvalidRegId), std::out_of_range);
    }

    SECTION("Handles negative position values correctly") {
        constexpr position_t cNegativePos1{-1};
        constexpr position_t cNegativePos2{-100};

        handler.set_register(cRegId1, cNegativePos1);
        handler.append_position(cRegId1, cInitialPos1);
        handler.append_position(cRegId1, cNegativePos2);
        REQUIRE(std::vector<position_t>{cNegativePos2, cInitialPos1, cNegativePos1}
                == handler.get_reversed_positions(cRegId1));
    }
}
