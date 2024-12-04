#include "PrefixTree.hpp"

#include <stdexcept>

namespace log_surgeon::finite_automata {
[[nodiscard]] auto PrefixTree::get_reversed_positions(id_t const index
) const -> std::vector<position_t> {
    if (m_nodes.size() <= index) {
        throw std::out_of_range("Prefix tree index out of range.");
    }

    std::vector<position_t> reversed_positions;
    auto current_node{m_nodes[index]};
    while (false == current_node.is_root()) {
        reversed_positions.push_back(current_node.get_position());
        current_node = m_nodes[current_node.get_predecessor_index().value()];
    }
    return reversed_positions;
}
}  // namespace log_surgeon::finite_automata
