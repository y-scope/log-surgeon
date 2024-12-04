#ifndef LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP

#include <cstdint>
#include <optional>
#include <stdexcept>
#include <vector>

namespace log_surgeon::finite_automata {
/**
 * Represents a prefix tree to store register data during TDFA simulation. Each path from the root
 * to an index corresponds to a sequence of positions for an individual tag:
 * - Positive position node: Indicates the tag was matched at the position.
 * - Negative position node: Indicates the tag was unmatched. If a negative node is the entire path,
 * it indicates the tag was never matched. If the negative tag is along a path containing positive
 * nodes, it functions as a placeholder. This can be useful for nested capture groups, to maintain a
 * one-to-one mapping between the contained capture group and the enclosing capture group.
 */
class PrefixTree {
public:
    using id_t = uint32_t;
    using position_t = int32_t;

private:
    /**
     * Represents a prefix tree node. A node stores a potential value for a TDFA register.
     *
     * A node stores the current position at which a tag was matched, as well as the index of the
     * prefix tree node corresponding to the previous match of the same tag.
     *
     * Note: A value of m_position < 0 indicates that the tag is currently unmatched in the lexed
     * string.
     */
    class Node {
    public:
        Node(std::optional<id_t> const predecessor_index, position_t const position)
                : m_predecessor_index{predecessor_index},
                  m_position{position} {}

        [[nodiscard]] auto is_root() const -> bool {
            return false == m_predecessor_index.has_value();
        }

        [[nodiscard]] auto get_predecessor_index() const -> std::optional<id_t> {
            return m_predecessor_index;
        }

        auto set_position(position_t const position) -> void { m_position = position; }

        [[nodiscard]] auto get_position() const -> position_t { return m_position; }

    private:
        std::optional<id_t> m_predecessor_index;
        position_t m_position;
    };

public:
    PrefixTree() : m_nodes{{std::nullopt, -1}} {}

    /**
     * @param predecessor_index Index of the inserted node's predecessor in the prefix tree.
     * @param position The position in the lexed string.
     * @return The index of the newly inserted node in the tree.
     * @throw std::out_of_range if the predecessor index is out of range.
     */
    auto insert(id_t const predecessor_index, position_t const position) -> id_t {
        if (m_nodes.size() <= predecessor_index) {
            throw std::out_of_range("Predecessor index out of range.");
        }

        m_nodes.emplace_back(predecessor_index, position);
        return m_nodes.size() - 1;
    }

    auto set(id_t const index, position_t const position) -> void {
        m_nodes.at(index).set_position(position);
    }

    /**
     * Retrieves a vector of positions in reverse order by traversing from the given index to the
     * root.
     * @param index The index of the node to start the traversal from.
     * @return A vector containing positions in reverse order from the given index to root.
     * @throw std::out_of_range if the index is out of range.
     */
    [[nodiscard]] auto get_reversed_positions(id_t index) const -> std::vector<position_t>;

private:
    std::vector<Node> m_nodes;
};

}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP
