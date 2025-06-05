#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP

#include <cstdint>
#include <optional>
#include <string>

#include <log_surgeon/types.hpp>

#include <fmt/core.h>

namespace log_surgeon::finite_automata {
/**
 * Represents a register operation:
 * - A register ID specifying which register the operation applies to.
 * - An operation type: `Copy`, `Set`, or `Negate`.
 * - An optional source register ID for copy operations.
 */
class RegisterOperation {
public:
    enum class Type : uint8_t {
        Copy,
        Set,
        Negate
    };

    bool operator==(RegisterOperation const& rhs) const {
        return m_reg_id == rhs.m_reg_id && m_type == rhs.m_type
               && m_copy_reg_id == rhs.m_copy_reg_id;
    }

    static auto create_set_operation(reg_id_t const reg_id) -> RegisterOperation {
        return {reg_id, Type::Set};
    }

    static auto create_negate_operation(reg_id_t const reg_id) -> RegisterOperation {
        return {reg_id, Type::Negate};
    }

    static auto create_copy_operation(reg_id_t const dest_reg_id, reg_id_t const src_reg_id)
            -> RegisterOperation {
        return {dest_reg_id, src_reg_id};
    }

    auto set_reg_id(reg_id_t const reg_id) -> void { m_reg_id = reg_id; }

    [[nodiscard]] auto get_reg_id() const -> reg_id_t { return m_reg_id; }

    [[nodiscard]] auto get_type() const -> Type { return m_type; }

    [[nodiscard]] auto get_copy_reg_id() const -> std::optional<reg_id_t> { return m_copy_reg_id; }

    /**
     * Serializes the register operation into a string representation.
     *
     * @return A formatted string encoding the operation.
     * @return `std::nullopt` if:
     * - the operation type is invalid.
     * - no source register is specified for a copy operation.
     */
    [[nodiscard]] auto serialize() const -> std::optional<std::string> {
        switch (m_type) {
            case Type::Copy:
                if (false == m_copy_reg_id.has_value()) {
                    return std::nullopt;
                }
                return fmt::format("{}{}{}", m_reg_id, "c", m_copy_reg_id.value());
            case Type::Set:
                return fmt::format("{}{}", m_reg_id, "p");
            case Type::Negate:
                return fmt::format("{}{}", m_reg_id, "n");
            default:
                return std::nullopt;
        }
    }

private:
    RegisterOperation(reg_id_t const reg_id, Type const type) : m_reg_id{reg_id}, m_type{type} {}

    RegisterOperation(reg_id_t const reg_id, reg_id_t const copy_reg_id)
            : m_reg_id{reg_id},
              m_type{Type::Copy},
              m_copy_reg_id{copy_reg_id} {}

    reg_id_t m_reg_id;
    Type m_type;
    std::optional<reg_id_t> m_copy_reg_id{std::nullopt};
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTEROPERATION_HPP
