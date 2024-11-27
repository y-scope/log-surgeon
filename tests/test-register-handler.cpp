#include <memory>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/RegisterHandler.hpp>

using log_surgeon::finite_automata::Register;
using log_surgeon::finite_automata::RegisterHandler;
using std::make_unique;
using std::unique_ptr;

TEST_CASE("Register operations", "[Register]") {
    SECTION("Register constructor and getter initializes correctly") {
        Register const reg(5);
        REQUIRE(reg.get_index() == 5);
    }

    SECTION("Register sets index correctly") {
        Register reg(5);
        reg.set_index(10);
        REQUIRE(reg.get_index() == 10);
    }

    SECTION("Register handles edge cases correctly") {
        Register reg(-1);
        REQUIRE(reg.get_index() == -1);

        reg.set_index(std::numeric_limits<int32_t>::max());
        REQUIRE(reg.get_index() == std::numeric_limits<int32_t>::max());
    }
}

TEST_CASE("RegisterHandler tests", "[RegisterHandler]") {
    RegisterHandler handler;

    SECTION("Initial state is empty") {
        REQUIRE_THROWS_AS(handler.get_reversed_positions(0), std::out_of_range);
    }

    constexpr uint32_t num_registers = 5;
    for (uint32_t i = 0; i < num_registers; i++) {
        handler.add_register(i, 0);
    }

    SECTION("Set register position correctly") {
        handler.set_register(0, 5);
        REQUIRE(std::vector<int32_t>{5} == handler.get_reversed_positions(0));
    }

    SECTION("Register relationships are maintained") {
        handler.set_register(0, 5);
        handler.set_register(1, 10);
        handler.set_register(2, 15);

        auto positions = handler.get_reversed_positions(2);
        REQUIRE(std::vector<int32_t>{15, 10, 5} == handler.get_reversed_positions(2));
    }

    SECTION("Copy register index correctly") {
        handler.set_register(0, 5);
        handler.copy_register(1, 0);
        REQUIRE(std::vector<int32_t>{{5}} == handler.get_reversed_positions(1));
    }

    SECTION("append_position appends position correctly") {
        handler.set_register(0, 5);
        handler.append_position(0, 7);
        REQUIRE(std::vector<int32_t>{{7, 5}} == handler.get_reversed_positions(0));
    }

    SECTION("Throws out of range correctly") {
        REQUIRE_THROWS_AS(handler.set_register(10, 5), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(10, 1), std::out_of_range);
        REQUIRE_THROWS_AS(handler.copy_register(0, 10), std::out_of_range);
        REQUIRE_THROWS_AS(handler.append_position(10, 5), std::out_of_range);
        REQUIRE_THROWS_AS(handler.get_reversed_positions(10), std::out_of_range);
    }
}
