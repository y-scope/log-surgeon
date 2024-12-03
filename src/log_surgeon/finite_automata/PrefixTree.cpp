#include "PrefixTree.hpp"

#include <cstdint>

namespace log_surgeon::finite_automata {
[[nodiscard]] auto PrefixTree::get_reversed_positions(uint32_t const index
) const -> std::vector<int32_t> {
    if (m_nodes.size() <= index) {
        throw std::out_of_range("Prefix tree index out of range.");
    }

    std::vector<int32_t> reversed_positions;
    auto current_node{m_nodes[index]};
    while (current_node.get_predecessor_index().has_value()) {
        reversed_positions.push_back(current_node.get_position());
        current_node = m_nodes[current_node.get_predecessor_index().value()];
    }
    return reversed_positions;
}
}  // namespace log_surgeon::finite_automata
