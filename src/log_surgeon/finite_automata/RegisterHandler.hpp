#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP

#include <cstdint>
#include <vector>

#include <log_surgeon/finite_automata/PrefixTree.hpp>

namespace log_surgeon::finite_automata {
/**
 * Represents a register that tracks a sequence of positions where a tag was matched in a lexed
 * string.
 *
 * To improve efficiency, registers are stored in a prefix tree. This class holds only the index
 * of the prefix tree node that represents the current state of the register.
 */
class Register {
public:
    explicit Register(uint32_t const index) : m_index{index} {}

    auto set_index(uint32_t const index) -> void { m_index = index; }

    [[nodiscard]] auto get_index() const -> uint32_t { return m_index; }

private:
    uint32_t m_index;
};

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
    auto add_register(uint32_t const predecessor_index, int32_t const position) -> void {
        auto const index{m_prefix_tree.insert(predecessor_index, position)};
        m_registers.emplace_back(index);
    }

    /**
     * @param register_index The index of the register to set.
     * @param position The position value to set in the register.
     * @throw std::out_of_range if the register index is out of range.
     */
    auto set_register(uint32_t const register_index, int32_t const position) -> void {
        if (m_registers.size() <= register_index) {
            throw std::out_of_range("Register index out of range.");
        }

        auto const tree_index{m_registers[register_index].get_index()};
        m_prefix_tree.set(tree_index, position);
    }

    /**
     * @param dest_register_index The index of the destination register.
     * @param source_register_index The index of the source register.
     * @throw std::out_of_range if the register index is out of range.
     */
    auto copy_register(uint32_t const dest_register_index, uint32_t const source_register_index)
            -> void {
        if (m_registers.size() <= source_register_index
            || m_registers.size() <= dest_register_index)
        {
            throw std::out_of_range("Register index out of range.");
        }

        m_registers[dest_register_index] = m_registers[source_register_index];
    }

    /**
     * @param register_index The index of the register to append to.
     * @param position The position to append to the register's history.
     * @throw std::out_of_range if the register index is out of range.
     */
    auto append_position(uint32_t const register_index, int32_t const position) -> void {
        if (m_registers.size() <= register_index) {
            throw std::out_of_range("Register index out of range.");
        }

        auto const tree_index{m_registers[register_index].get_index()};
        auto const new_index{m_prefix_tree.insert(tree_index, position)};
        m_registers[register_index].set_index(new_index);
    }

    /**
     * @param register_index The index of the register whose positions are retrieved.
     * @return A vector of positions representing the history of the given register.
     * @throw std::out_of_range if the register index is out of range.
     */
    [[nodiscard]] auto get_reversed_positions(uint32_t const register_index
    ) const -> std::vector<int32_t> {
        if (m_registers.size() <= register_index) {
            throw std::out_of_range("Register index out of range.");
        }

        auto const tree_index{m_registers[register_index].get_index()};
        return m_prefix_tree.get_reversed_positions(tree_index);
    }

private:
    PrefixTree m_prefix_tree;
    std::vector<Register> m_registers;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER_HPP
