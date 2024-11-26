#include "PrefixTree.hpp"

#include <cstdint>

namespace log_surgeon::finite_automata {
[[nodiscard]] auto PrefixTree::get_reversed_positions(uint32_t const index
) const -> std::vector<int32_t> {
    if (m_nodes.size() <= index) {
        throw std::out_of_range("Prefix tree index out-of-bounds.");
    }

    std::vector<int32_t> reversed_positions;
    auto current_index = index;
    while(0 < current_index) {
        auto const& current_node = m_nodes[current_index];
        reversed_positions.push_back(current_node.get_position());
        current_index = current_node.get_predecessor_index();
    }
    return reversed_positions;
}
}  // namespace log_surgeon::finite_automata
