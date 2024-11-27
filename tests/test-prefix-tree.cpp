#include <memory>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/PrefixTree.hpp>

using log_surgeon::finite_automata::PrefixTree;

TEST_CASE("Prefix tree operations", "[PrefixTree]") {
    SECTION("Newly constructed tree works correctly") {
        PrefixTree const tree;

        REQUIRE(tree.get_reversed_positions(0).empty());
    }

    SECTION("Adding nodes to the prefix tree works correctly") {
        PrefixTree tree;
        uint32_t index_1 = tree.insert(0, 4);
        REQUIRE(std::vector<int32_t>({4}) == tree.get_reversed_positions(index_1));

        uint32_t index_2 = tree.insert(index_1, 7);
        REQUIRE(std::vector<int32_t>({7, 4}) == tree.get_reversed_positions(index_2));

        uint32_t index_3 = tree.insert(index_2, 9);
        REQUIRE(std::vector<int32_t>({9, 7, 4}) == tree.get_reversed_positions(index_3));
    }

    SECTION("Invalid index access throws correctly") {
        PrefixTree tree;
        REQUIRE_THROWS_AS(tree.get_reversed_positions(1), std::out_of_range);

        tree.insert(0, 4);
        REQUIRE_THROWS_AS(tree.get_reversed_positions(2), std::out_of_range);
        REQUIRE_THROWS_AS(tree.get_reversed_positions(3), std::out_of_range);

        REQUIRE_THROWS_AS(
                tree.get_reversed_positions(std::numeric_limits<uint32_t>::max()),
                std::out_of_range
        );
    }

    SECTION("Set position for a valid index works correctly") {
        PrefixTree tree;

        // Test updates to different nodes
        uint32_t index_1 = tree.insert(0, 4);
        uint32_t index_2 = tree.insert(index_1, 7);
        tree.set(index_1, 10);
        tree.set(index_2, 12);
        REQUIRE(tree.get_reversed_positions(index_1) == std::vector<int32_t>({10}));
        REQUIRE(tree.get_reversed_positions(index_2) == std::vector<int32_t>({12, 10}));

        // Test multiple updates to the same node
        tree.set(index_2, 15);
        tree.set(index_2, 20);
        REQUIRE(tree.get_reversed_positions(index_2) == std::vector<int32_t>({20, 10}));

        // Test that updates don't affect unrelated paths
        uint32_t index_3 = tree.insert(0, 30);
        tree.set(index_3, 25);
        REQUIRE(tree.get_reversed_positions(index_1) == std::vector<int32_t>({10}));
        REQUIRE(tree.get_reversed_positions(index_2) == std::vector<int32_t>({20, 10}));
    }

    SECTION("Set position for an invalid index throws correctly") {
        PrefixTree tree;

        // Test setting position before any insertions
        REQUIRE_THROWS_AS(tree.set(100, 20), std::out_of_range);

        // Test setting position just beyond valid range
        uint32_t index_1 = tree.insert(0, 4);
        REQUIRE_THROWS_AS(tree.set(index_1 + 1, 20), std::out_of_range);
    }
}
