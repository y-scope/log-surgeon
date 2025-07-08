#include "RegisterOperation.hpp"

#include <optional>
#include <string>

#include <fmt/core.h>

namespace log_surgeon::finite_automata {
auto RegisterOperation::serialize() const -> std::optional<std::string> {
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
}  // namespace log_surgeon::finite_automata
