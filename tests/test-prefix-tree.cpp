#include <memory>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/PrefixTree.hpp>

using log_surgeon::finite_automata::PrefixTree;

TEST_CASE("Prefix tree operations", "[PrefixTree]") {
    SECTION("Newly constructed tree works correctly") {
        PrefixTree const tree;

        // A newly constructed tree should return no positions as the root node is ignored
        REQUIRE(tree.get_reversed_positions(0).empty());
    }

    SECTION("Inserting nodes into the prefix tree works correctly") {
        PrefixTree tree;

        // Test basic insertions
        uint32_t index_1 = tree.insert(0, 4);
        uint32_t index_2 = tree.insert(index_1, 7);
        uint32_t index_3 = tree.insert(index_2, 9);
        REQUIRE(std::vector<int32_t>{4} == tree.get_reversed_positions(index_1));
        REQUIRE(std::vector<int32_t>{7, 4} == tree.get_reversed_positions(index_2));
        REQUIRE(std::vector<int32_t>{9, 7, 4} == tree.get_reversed_positions(index_3));

        // Test insertion with large position values
        uint32_t index_4 = tree.insert(0, std::numeric_limits<int32_t>::max());
        REQUIRE(std::numeric_limits<int32_t>::max() == tree.get_reversed_positions(index_4)[0]);

        // Test insertion with negative position values
        uint32_t index_5 = tree.insert(0, -1);
        uint32_t index_6 = tree.insert(index_5, -100);
        REQUIRE(std::vector<int32_t>{-1} == tree.get_reversed_positions(index_5));
        REQUIRE(std::vector<int32_t>{-1, -100} == tree.get_reversed_positions(index_6));
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
        // Test that you can set the root node for sanity, although this value is not used
        tree.set(0, 10);

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
