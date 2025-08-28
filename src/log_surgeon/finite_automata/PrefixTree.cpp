#include "PrefixTree.hpp"

#include <stdexcept>
#include <vector>

namespace log_surgeon::finite_automata {
auto PrefixTree::get_reversed_positions(id_t const node_id) const -> std::vector<position_t> {
    if (m_nodes.size() <= node_id) { throw std::out_of_range("Prefix tree index out of range."); }

    std::vector<position_t> reversed_positions;
    auto current_node{m_nodes[node_id]};
    while (false == current_node.is_root()) {
        reversed_positions.push_back(current_node.get_position());
        current_node = m_nodes[current_node.get_parent_id_unsafe()];
    }
    return reversed_positions;
}
}  // namespace log_surgeon::finite_automata
