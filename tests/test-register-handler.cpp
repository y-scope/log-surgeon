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
    handler.add_registers(num_registers);
    return handler;
}
}  // namespace

TEST_CASE("`RegisterHandler` tests", "[RegisterHandler]") {
    constexpr position_t cInitialPos1{5};
    constexpr size_t cNumRegisters{5};
    constexpr size_t cRegId1{0};
    constexpr size_t cRegId2{1};
    constexpr position_t cAppendPos1{5};
    constexpr position_t cAppendPos2{10};
    constexpr position_t cAppendPos3{15};

    RegisterHandler handler{handler_init(cNumRegisters)};

    SECTION("Throws out of range correctly") {
        constexpr size_t cInvalidRegId{10};
        RegisterHandler const empty_handler{handler_init(0)};

        REQUIRE_THROWS_AS(empty_handler.get_reversed_positions(cRegId1), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(cInvalidRegId, cRegId2), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(cRegId1, cInvalidRegId), std::out_of_range);
        REQUIRE_THROWS_AS(handler.append_position(cInvalidRegId, cInitialPos1), std::out_of_range);
        REQUIRE_THROWS_AS(handler.get_reversed_positions(cInvalidRegId), std::out_of_range);
    }

    SECTION("Initial register is empty") {
        auto positions{handler.get_reversed_positions(cRegId1)};
        REQUIRE(handler.get_reversed_positions(cRegId1).empty());

        handler.copy_register(cRegId2, cRegId1);
        REQUIRE(handler.get_reversed_positions(cRegId2).empty());
    }

    SECTION("Append and copy position work correctly") {
        handler.append_position(cRegId1, cAppendPos1);
        handler.append_position(cRegId1, cAppendPos2);
        handler.append_position(cRegId1, cAppendPos3);
        REQUIRE(std::vector<position_t>{cAppendPos3, cAppendPos2, cAppendPos1}
                == handler.get_reversed_positions(cRegId1));

        handler.copy_register(cRegId2, cRegId1);
        REQUIRE(std::vector<position_t>{cAppendPos3, cAppendPos2, cAppendPos1}
                == handler.get_reversed_positions(cRegId2));
    }

    SECTION("Handles negative position values correctly") {
        constexpr size_t cRegId3{2};
        constexpr position_t cNegativePos1{-1};
        constexpr position_t cNegativePos2{-100};

        handler.append_position(cRegId3, cNegativePos1);
        handler.append_position(cRegId3, cNegativePos2);
        REQUIRE(std::vector<position_t>{cNegativePos2, cNegativePos1}
                == handler.get_reversed_positions(cRegId3));
    }
}
