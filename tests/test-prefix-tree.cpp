#include <memory>

#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/PrefixTree.hpp>

using log_surgeon::finite_automata::PrefixTree;
using id_t = PrefixTree::id_t;
using position_t = PrefixTree::position_t;

TEST_CASE("`PrefixTree` operations", "[PrefixTree]") {
    SECTION("Newly constructed tree works correctly") {
        PrefixTree const tree;

        // A newly constructed tree should return no positions as the root node is ignored
        REQUIRE(tree.get_reversed_positions(0).empty());
    }

    SECTION("Inserting nodes into the prefix tree works correctly") {
        PrefixTree tree;

        // Test basic insertions
        auto const node_id_1{tree.insert(0, 4)};
        auto const node_id_2{tree.insert(node_id_1, 7)};
        auto const node_id_3{tree.insert(node_id_2, 9)};
        REQUIRE(std::vector<position_t>{4} == tree.get_reversed_positions(node_id_1));
        REQUIRE(std::vector<position_t>{7, 4} == tree.get_reversed_positions(node_id_2));
        REQUIRE(std::vector<position_t>{9, 7, 4} == tree.get_reversed_positions(node_id_3));

        // Test insertion with large position values
        auto const node_id_4{tree.insert(0, std::numeric_limits<position_t>::max())};
        REQUIRE(std::numeric_limits<position_t>::max() == tree.get_reversed_positions(node_id_4)[0]
        );

        // Test insertion with negative position values
        auto const node_id_5{tree.insert(0, -1)};
        auto const node_id_6{tree.insert(node_id_5, -100)};
        REQUIRE(std::vector<position_t>{-1} == tree.get_reversed_positions(node_id_5));
        REQUIRE(std::vector<position_t>{-100, -1} == tree.get_reversed_positions(node_id_6));
    }

    SECTION("Invalid index access throws correctly") {
        PrefixTree tree;
        REQUIRE_THROWS_AS(tree.get_reversed_positions(1), std::out_of_range);

        tree.insert(0, 4);
        REQUIRE_THROWS_AS(tree.get_reversed_positions(2), std::out_of_range);
        REQUIRE_THROWS_AS(tree.get_reversed_positions(3), std::out_of_range);

        REQUIRE_THROWS_AS(
                tree.get_reversed_positions(std::numeric_limits<id_t>::max()),
                std::out_of_range
        );
    }

    SECTION("Set position for a valid index works correctly") {
        PrefixTree tree;
        // Test that you can set the root node for sanity, although this value is not used
        tree.set(0, 10);

        // Test updates to different nodes
        auto const node_id_1{tree.insert(0, 4)};
        auto const node_id_2{tree.insert(node_id_1, 7)};
        tree.set(node_id_1, 10);
        tree.set(node_id_2, 12);
        REQUIRE(std::vector<position_t>{10} == tree.get_reversed_positions(node_id_1));
        REQUIRE(std::vector<position_t>{12, 10} == tree.get_reversed_positions(node_id_2));

        // Test multiple updates to the same node
        tree.set(node_id_2, 15);
        tree.set(node_id_2, 20);
        REQUIRE(std::vector<position_t>{20, 10} == tree.get_reversed_positions(node_id_2));

        // Test that updates don't affect unrelated paths
        auto const node_id_3{tree.insert(0, 30)};
        tree.set(node_id_3, 25);
        REQUIRE(std::vector<position_t>{10} == tree.get_reversed_positions(node_id_1));
        REQUIRE(std::vector<position_t>{20, 10} == tree.get_reversed_positions(node_id_2));
    }

    SECTION("Set position for an invalid index throws correctly") {
        PrefixTree tree;

        // Test setting position before any insertions
        REQUIRE_THROWS_AS(tree.set(100, 20), std::out_of_range);

        // Test setting position just beyond valid range
        auto const node_id_1{tree.insert(0, 4)};
        REQUIRE_THROWS_AS(tree.set(node_id_1 + 1, 20), std::out_of_range);
    }
}
