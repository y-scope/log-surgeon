#ifndef LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP

#include <cstddef>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <vector>

namespace log_surgeon::finite_automata {
/**
 * Represents a prefix tree to store register data during TDFA simulation. Each node in the tree
 * stores a single position in the lexed string. Each path from the root to an index corresponds to
 * a sequence of positions for an individual tag:
 * - Positive position node: Indicates the tag was matched at the position.
 * - Negative position node: Indicates the tag was unmatched. If a negative node is the entire path,
 *   it indicates the tag was never matched. If the negative tag is along a path containing positive
 *   nodes, it functions as a placeholder. This can be useful for nested capture groups, to maintain
 *   a one-to-one mapping between the contained capture group and the enclosing capture group.
 */
class PrefixTree {
public:
    using id_t = uint32_t;
    using position_t = int32_t;

    static constexpr id_t cRootId{0};

    PrefixTree() : m_nodes{{std::nullopt, -1}} {}

    /**
     * @param parent_node_id Index of the inserted node's parent in the prefix tree.
     * @param position The position in the lexed string.
     * @return The index of the newly inserted node in the tree.
     * @throw std::out_of_range if the parent's index is out of range.
     */
    [[maybe_unused]] auto insert(id_t const parent_node_id, position_t const position) -> id_t {
        if (m_nodes.size() <= parent_node_id) {
            throw std::out_of_range("Predecessor index out of range.");
        }

        m_nodes.emplace_back(parent_node_id, position);
        return m_nodes.size() - 1;
    }

    auto set(id_t const node_id, position_t const position) -> void {
        m_nodes.at(node_id).set_position(position);
    }

    [[nodiscard]] auto size() const -> size_t { return m_nodes.size(); }

    /**
     * @param node_id The index of the node.
     * @return A vector containing positions in order from the given index up to but not including
     * the root node.
     * @throw std::out_of_range if the index is out of range.
     */
    [[nodiscard]] auto get_reversed_positions(id_t node_id) const -> std::vector<position_t>;

private:
    class Node {
    public:
        Node(std::optional<id_t> const parent_node_id, position_t const position)
                : m_parent_node_id{parent_node_id},
                  m_position{position} {}

        [[nodiscard]] auto is_root() const -> bool { return false == m_parent_node_id.has_value(); }

        [[nodiscard]] auto get_parent_node_id() const -> std::optional<id_t> {
            return m_parent_node_id;
        }

        auto set_position(position_t const position) -> void { m_position = position; }

        [[nodiscard]] auto get_position() const -> position_t { return m_position; }

    private:
        std::optional<id_t> m_parent_node_id;
        position_t m_position;
    };

    std::vector<Node> m_nodes;
};

}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_PREFIX_TREE_HPP
