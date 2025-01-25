#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP

#include <cstdint>
#include <string>

#include <fmt/core.h>

#include <log_surgeon/finite_automata/Register.hpp>

namespace log_surgeon::finite_automata {
enum class RegisterOperationType : uint8_t {
    Copy,
    Set,
    Negate
};

class RegisterOperation {
public:
    RegisterOperation(register_id_t const reg_id, RegisterOperationType const type)
            : m_reg_id{reg_id},
              m_type{type} {}

    [[nodiscard]] auto get_reg_id() const -> register_id_t { return m_reg_id; }

    [[nodiscard]] auto get_type() const -> RegisterOperationType { return m_type; }

    /**
     * @return A string representation of the register opertion.
     */
    [[nodiscard]] auto serialize() const -> std::string {
        switch (m_type) {
            case RegisterOperationType::Copy:
                return fmt::format("{}{}", m_reg_id, "c");
            case RegisterOperationType::Set:
                return fmt::format("{}{}", m_reg_id, "p");
            case RegisterOperationType::Negate:
                return fmt::format("{}{}", m_reg_id, "n");
            default:
                return fmt::format("{}{}", m_reg_id, "?");
        }
    }

private:
    register_id_t m_reg_id;
    RegisterOperationType m_type;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP
