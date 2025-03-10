#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP

#include <cstdint>
#include <vector>

#include <log_surgeon/finite_automata/PrefixTree.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon::finite_automata {
/**
 * The register handler maintains a prefix tree that is sufficient to represent all registers.
 * The register handler also contains a vector of registers, and performs the set, copy, and append
 * operations for these registers.
 *
 * NOTE: For efficiency, registers are not initialized when lexing a new string; instead, it is the
 * DFA's responsibility to set the register values when needed.
 */
class RegisterHandler {
public:
    auto add_registers(uint32_t const num_reg_to_add) -> std::vector<uint32_t> {
        std::vector<uint32_t> added_registers;
        for (uint32_t i{0}; i < num_reg_to_add; ++i) {
            added_registers.emplace_back(add_register());
        }
        return added_registers;
    }

    auto add_register() -> reg_id_t {
        auto const prefix_tree_node_id{
                m_prefix_tree.insert(PrefixTree::cRootId, PrefixTree::cDefaultPos)
        };
        m_registers.emplace_back(prefix_tree_node_id);
        return m_registers.size() - 1;
    }

    auto add_register(PrefixTree::id_t const prefix_tree_parent_node_id) -> reg_id_t {
        auto const prefix_tree_node_id{
                m_prefix_tree.insert(prefix_tree_parent_node_id, PrefixTree::cDefaultPos)
        };
        m_registers.emplace_back(prefix_tree_node_id);
        return m_registers.size() - 1;
    }

    auto set_register(reg_id_t const reg_id, PrefixTree::position_t const position) -> void {
        m_prefix_tree.set(m_registers.at(reg_id), position);
    }

    auto copy_register(reg_id_t const dest_reg_id, reg_id_t const source_reg_id) -> void {
        m_registers.at(dest_reg_id) = m_registers.at(source_reg_id);
    }

    auto append_position(reg_id_t const reg_id, PrefixTree::position_t const position) -> void {
        auto const node_id{m_registers.at(reg_id)};
        m_registers.at(reg_id) = m_prefix_tree.insert(node_id, position);
    }

    [[nodiscard]] auto get_reversed_positions(reg_id_t const reg_id
    ) const -> std::vector<PrefixTree::position_t> {
        return m_prefix_tree.get_reversed_positions(m_registers.at(reg_id));
    }

private:
    PrefixTree m_prefix_tree;
    std::vector<PrefixTree::id_t> m_registers;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP
