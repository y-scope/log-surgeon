#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP

#include <cstdint>
#include <optional>
#include <string>

#include <fmt/core.h>

#include <log_surgeon/types.hpp>

namespace log_surgeon::finite_automata {
enum class RegisterOperationType : uint8_t {
    Copy,
    Set,
    Negate
};

class RegisterOperation {
public:
    RegisterOperation(reg_id_t const reg_id, RegisterOperationType const type)
            : m_reg_id{reg_id},
              m_type{type} {}

    RegisterOperation(reg_id_t const reg_id, reg_id_t const copy_reg_id)
            : m_reg_id{reg_id},
              m_type{RegisterOperationType::Copy},
              m_copy_reg_id{copy_reg_id} {}

    auto set_reg_id(reg_id_t const reg_id) -> void { m_reg_id = reg_id; }

    [[nodiscard]] auto get_reg_id() const -> reg_id_t { return m_reg_id; }

    [[nodiscard]] auto get_type() const -> RegisterOperationType { return m_type; }

    /**
     * @return A string representation of the register opertion.
     */
    [[nodiscard]] auto serialize() const -> std::optional<std::string> {
        switch (m_type) {
            case RegisterOperationType::Copy:
                if (false == m_copy_reg_id.has_value()) {
                    return std::nullopt;
                }
                return fmt::format("{}{}{}", m_reg_id, "c", m_copy_reg_id.value());
            case RegisterOperationType::Set:
                return fmt::format("{}{}", m_reg_id, "p");
            case RegisterOperationType::Negate:
                return fmt::format("{}{}", m_reg_id, "n");
            default:
                return fmt::format("{}{}", m_reg_id, "?");
        }
    }

private:
    reg_id_t m_reg_id;
    RegisterOperationType m_type;
    std::optional<reg_id_t> m_copy_reg_id{std::nullopt};
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP
