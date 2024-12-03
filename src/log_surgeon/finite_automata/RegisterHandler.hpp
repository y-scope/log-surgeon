#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP

#include <cstdint>
#include <vector>

#include <log_surgeon/finite_automata/PrefixTree.hpp>

namespace log_surgeon::finite_automata {
/**
 * The register handler maintains a prefix tree that is sufficient to represent all registers.
 * The register handler also contains a vector of registers, and performs the set, copy, and append
 * operations for these registers.
 *
 * Note: for efficiency these registers may be re-used, but are not required to be re-initialized.
 * It is the responsibility of the DFA to set the register value when needed.
 */
class RegisterHandler {
public:
    auto add_register(
            PrefixTree::id_t const prefix_tree_parent_node_id,
            PrefixTree::position_t const position
    ) -> void {
        auto const prefix_tree_node_id{m_prefix_tree.insert(prefix_tree_parent_node_id, position)};
        m_registers.emplace_back(prefix_tree_node_id);
    }

    auto set_register(uint32_t const reg_id, PrefixTree::position_t const position) -> void {
        m_prefix_tree.set(m_registers.at(reg_id), position);
    }

    auto copy_register(uint32_t const dest_reg_id, uint32_t const source_reg_id) -> void {
        m_registers.at(dest_reg_id) = m_registers.at(source_reg_id);
    }

    auto append_position(uint32_t const reg_id, PrefixTree::position_t const position) -> void {
        auto& reg{m_registers.at(reg_id)};
        reg = m_prefix_tree.insert(reg, position);
    }

    [[nodiscard]] auto get_reversed_positions(uint32_t const reg_id
    ) const -> std::vector<PrefixTree::position_t> {
        return m_prefix_tree.get_reversed_positions(m_registers.at(reg_id));
    }

private:
    PrefixTree m_prefix_tree;
    std::vector<PrefixTree::id_t> m_registers;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP
