#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER

#include <cstdint>
#include <unordered_map>
#include <vector>

#include <log_surgeon/finite_automata/PrefixTree.hpp>

namespace log_surgeon::finite_automata {
/**
 * A register stores an index in the prefix tree. The index node fully represents the register's
 * history.
 *
 * Note: history refers to the previous tag locations. E.g., given the tagged regex "aaa(1\d2)+",
 * after parsing input string "aaa123", a register representing tag 1 would contain the history
 * {3,4,5}.
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
    void add_register(uint32_t const predecessor_index, int32_t const position) {
        auto const index = prefix_tree.insert(predecessor_index, position);
        m_registers.emplace_back(index);
    }

    /**
     * @param register_index
     * @param position
     * @throw std::out_of_range if the register index is out of range
     */
    void set_register(uint32_t const register_index, int32_t const position) {
        if (m_registers.size() <= register_index) {
            throw std::out_of_range("Register index out of range");
        }

        auto const tree_index = m_registers[register_index].get_index();
        prefix_tree.set(tree_index, position);
    }

    /**
     * @param dest_register_index
     * @param source_register_index
     * @throw std::out_of_range if the register index is out of range
     */
    void copy_register(uint32_t const dest_register_index, uint32_t const source_register_index) {
        if (m_registers.size() <= source_register_index
            || m_registers.size() <= dest_register_index)
        {
            throw std::out_of_range("Register index out of range");
        }

        m_registers[dest_register_index] = m_registers[source_register_index];
    }

    /**
     * @param register_index
     * @param position
     * @throw std::out_of_range if the register index is out of range
     */
    void append_position(uint32_t register_index, int32_t position) {
        if (register_index >= m_registers.size()) {
            throw std::out_of_range("Register index out of range");
        }

        uint32_t const tree_index = m_registers[register_index].get_index();
        auto const new_index = prefix_tree.insert(tree_index, position);
        m_registers[register_index].set_index(new_index);
    }

    /**
     * @param register_index
     * @return Vector of positions representing the history of the given register.
     * @throw std::out_of_range if the register index is out of range

     */
    [[nodiscard]] auto get_reversed_positions(uint32_t const register_index
    ) const -> std::vector<int32_t> {
        if (register_index >= m_registers.size()) {
            throw std::out_of_range("Register index out of range");
        }

        uint32_t const tree_index = m_registers[register_index].get_index();
        return prefix_tree.get_reversed_positions(tree_index);
    }

private:
    PrefixTree prefix_tree;
    std::vector<Register> m_registers;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTER_HANDLER
