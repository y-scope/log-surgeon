#ifndef LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP

#include <cstdint>
#include <stdexcept>
#include <vector>

namespace log_surgeon::finite_automata {
/**
 * Represents a prefix tree node used by a register to track tag matches in a lexed string.
 * This node stores the current position at which a tag was matched, as well as the index of the
 * prefix tree node corresponding to the previous match of the same tag.
 *
 * Note: A value of m_position < 0 indicates that the tag is currently unmatched in the lexed
 * string.
 */
class PrefixTreeNode {
public:
    PrefixTreeNode(uint32_t const predecessor_index, int32_t const position)
            : m_predecessor_index{predecessor_index},
              m_position{position} {}

    [[nodiscard]] auto get_predecessor_index() const -> uint32_t { return m_predecessor_index; }

    auto set_position(int32_t const position) -> void { m_position = position; }

    [[nodiscard]] auto get_position() const -> int32_t { return m_position; }

private:
    uint32_t m_predecessor_index;
    int32_t m_position;
};

/**
 * Represents a prefix tree that stores all data needed by registers.
 *
 * Each path from the root to an index represents a sequence of matched tag positions.
 */
class PrefixTree {
public:
    PrefixTree() : m_nodes{{0, -1}} {}

    /**
     * @param predecessor_index Index of the inserted node's predecessor in the prefix tree.
     * @param position The position in the lexed string.
     * @return The index of the newly inserted node in the tree.
     * @throw std::out_of_range if the predecessor index is out of range.
     */
    auto insert(uint32_t const predecessor_index, int32_t const position) -> uint32_t {
        if (m_nodes.size() <= predecessor_index) {
            throw std::out_of_range("Predecessor index out of range.");
        }

        m_nodes.emplace_back(predecessor_index, position);
        return m_nodes.size() - 1;
    }

    /**
     * @param index Index of the node to update.
     * @param position New position value to set for the node.
     * @throw std::out_of_range if prefix tree index is out of range.
     */
    auto set(uint32_t const index, int32_t const position) -> void {
        if (m_nodes.size() <= index) {
            throw std::out_of_range("Prefix tree index out of range.");
        }

        m_nodes[index].set_position(position);
    }

    /**
     * Retrieves a vector of positions in reverse order by traversing from the given index to the
     * root.
     * @param index The index of the node to start the traversal from.
     * @return A vector containing positions in reverse order from the given index to root.
     * @throw std::out_of_range if the index is out of range.
     */
    [[nodiscard]] auto get_reversed_positions(uint32_t index) const -> std::vector<int32_t>;

private:
    std::vector<PrefixTreeNode> m_nodes;
};

}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP
