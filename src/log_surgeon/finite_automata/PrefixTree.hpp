#ifndef LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE
#define LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE

#include <cstdint>
#include <stdexcept>
#include <unordered_map>
#include <vector>

namespace log_surgeon::finite_automata {

/**
 * A prefix tree node helps a register represent a tag by storing the current position where a tag
 * was matched in the lexxed string, as well as the index of the prefix tree node that stores the
 * previous time the tag was matched.
 *
 * Note: m_position is -1 when a tag is
 * unmatched.
 */
class PrefixTreeNode {
public:
    PrefixTreeNode(uint32_t const predecessor_index, int32_t const position)
            : m_predecessor_index(predecessor_index),
              m_position(position) {}

    [[nodiscard]] auto get_predecessor_index() const -> uint32_t { return m_predecessor_index; }

    [[nodiscard]] auto get_position() const -> int32_t { return m_position; }

private:
    uint32_t m_predecessor_index;
    int32_t m_position;
};

/**
 * A prefix tree structure to store positions associated with registers.
 *
 * PrefixTree stores positions at nodes, and each node can represent a part of a position.
 * Multiple positions can be stored at each index in the tree. The tree allows for the addition of
 * positions and the retrieval of positions by their associated index.
 */
class PrefixTree {
public:
    PrefixTree() : m_nodes{{0, -1}} {}

    /**
     * @return The index of the newly inserted node in the tree.
     */
    uint32_t insert(uint32_t const predecessor_index, int32_t const position) {
        m_nodes.emplace_back(predecessor_index, position);
        return m_nodes.size() - 1;
    }

    /**
     * @param index Representing the leaf node of the register's sub-tree.
     * @return The positions, in reverse order, at which the register places the tag in the
     * lexed string.
     */
    [[nodiscard]] auto get_reversed_positions(uint32_t index) const -> std::vector<int32_t>;

private:
    std::vector<PrefixTreeNode> m_nodes;
};

}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE
