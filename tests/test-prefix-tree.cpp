#include <limits>
#include <stdexcept>
#include <vector>

#include <log_surgeon/finite_automata/PrefixTree.hpp>

#include <catch2/catch_test_macros.hpp>

using log_surgeon::finite_automata::PrefixTree;
using id_t = PrefixTree::id_t;
using position_t = PrefixTree::position_t;

TEST_CASE("`PrefixTree` operations", "[PrefixTree]") {
    constexpr auto cRootId{PrefixTree::cRootId};
    constexpr position_t cInitialPos1{4};
    constexpr position_t cSetPos1{10};

    SECTION("Newly constructed tree works correctly") {
        PrefixTree const tree;

        // A newly constructed tree should return no positions as the root node is ignored
        REQUIRE(tree.get_reversed_positions(cRootId).empty());
    }

    SECTION("Inserting nodes into the prefix tree works correctly") {
        constexpr position_t cInitialPos2{7};
        constexpr position_t cInitialPos3{9};
        constexpr position_t cMaxPos{std::numeric_limits<position_t>::max()};
        constexpr position_t cNegativePos1{-1};
        constexpr position_t cNegativePos2{-100};
        constexpr position_t cTreeSize1{4};
        constexpr position_t cTreeSize2{8};

        PrefixTree tree;

        // Test basic insertions
        auto const node_id_1{tree.insert(cRootId, cInitialPos1)};
        auto const node_id_2{tree.insert(node_id_1, cInitialPos2)};
        auto const node_id_3{tree.insert(node_id_2, cInitialPos3)};
        REQUIRE(std::vector<position_t>{cInitialPos1} == tree.get_reversed_positions(node_id_1));
        REQUIRE(std::vector<position_t>{cInitialPos2, cInitialPos1}
                == tree.get_reversed_positions(node_id_2));
        REQUIRE(std::vector<position_t>{cInitialPos3, cInitialPos2, cInitialPos1}
                == tree.get_reversed_positions(node_id_3));
        REQUIRE(cTreeSize1 == tree.size());

        // Test insertion with large position values
        auto const node_id_4{tree.insert(cRootId, cMaxPos)};
        REQUIRE(cMaxPos == tree.get_reversed_positions(node_id_4)[0]);

        // Test insertion with negative position values
        auto const node_id_5{tree.insert(cRootId, cNegativePos1)};
        auto const node_id_6{tree.insert(node_id_5, cInitialPos1)};
        auto const node_id_7{tree.insert(node_id_6, cNegativePos2)};
        REQUIRE(std::vector<position_t>{cNegativePos1} == tree.get_reversed_positions(node_id_5));
        REQUIRE(std::vector<position_t>{cInitialPos1, cNegativePos1}
                == tree.get_reversed_positions(node_id_6));
        REQUIRE(std::vector<position_t>{cNegativePos2, cInitialPos1, cNegativePos1}
                == tree.get_reversed_positions(node_id_7));
        REQUIRE(cTreeSize2 == tree.size());
    }

    SECTION("Invalid index access throws correctly") {
        PrefixTree tree;
        REQUIRE_THROWS_AS(tree.get_reversed_positions(tree.size()), std::out_of_range);

        tree.insert(cRootId, cInitialPos1);
        REQUIRE_THROWS_AS(tree.get_reversed_positions(tree.size()), std::out_of_range);

        REQUIRE_THROWS_AS(
                tree.get_reversed_positions(std::numeric_limits<id_t>::max()),
                std::out_of_range
        );
    }

    SECTION("Set position for a valid index works correctly") {
        constexpr position_t cSetPos2{12};
        constexpr position_t cSetPos3{15};
        constexpr position_t cSetPos4{20};

        PrefixTree tree;
        // Test that you can set the root node for sanity, although this value is not used
        tree.set(cRootId, cSetPos1);

        // Test updates to different nodes
        auto const node_id_1{tree.insert(cRootId, cInitialPos1)};
        auto const node_id_2{tree.insert(node_id_1, cInitialPos1)};
        tree.set(node_id_1, cSetPos1);
        tree.set(node_id_2, cSetPos2);
        REQUIRE(std::vector<position_t>{cSetPos1} == tree.get_reversed_positions(node_id_1));
        REQUIRE(std::vector<position_t>{cSetPos2, cSetPos1}
                == tree.get_reversed_positions(node_id_2));

        // Test multiple updates to the same node
        tree.set(node_id_2, cSetPos3);
        tree.set(node_id_2, cSetPos4);
        REQUIRE(std::vector<position_t>{cSetPos4, cSetPos1}
                == tree.get_reversed_positions(node_id_2));

        // Test that updates don't affect unrelated paths
        auto const node_id_3{tree.insert(cRootId, cSetPos2)};
        tree.set(node_id_3, cSetPos3);
        REQUIRE(std::vector<position_t>{cSetPos1} == tree.get_reversed_positions(node_id_1));
        REQUIRE(std::vector<position_t>{cSetPos4, cSetPos1}
                == tree.get_reversed_positions(node_id_2));
    }

    SECTION("Set position for an invalid index throws correctly") {
        constexpr id_t cInvalidNodeId{100};

        PrefixTree tree;

        // Test setting position before any insertions
        REQUIRE_THROWS_AS(tree.set(cInvalidNodeId, cSetPos1), std::out_of_range);

        // Test setting position just beyond valid range
        auto const node_id_1{tree.insert(cRootId, cInitialPos1)};
        REQUIRE_THROWS_AS(tree.set(node_id_1 + 1, cSetPos1), std::out_of_range);
    }
}
