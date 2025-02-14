#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAGOPERATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_TAGOPERATION_HPP

#include <cstdint>
#include <string>
#include <tuple>

#include <fmt/core.h>

#include <log_surgeon/types.hpp>

namespace log_surgeon::finite_automata {
enum class TagOperationType : uint8_t {
    Set,
    Negate,
    None
};

class TagOperation {
public:
    TagOperation(tag_id_t const tag_id, TagOperationType const type)
            : m_tag_id{tag_id},
              m_type{type} {}

    [[nodiscard]] auto operator<(TagOperation const& rhs) const -> bool {
        return std::tie(m_tag_id, m_type) < std::tie(rhs.m_tag_id, rhs.m_type);
    }

    [[nodiscard]] auto operator==(TagOperation const& rhs) const -> bool {
        return std::tie(m_tag_id, m_type) == std::tie(rhs.m_tag_id, rhs.m_type);
    }

    [[nodiscard]] auto get_tag_id() const -> tag_id_t { return m_tag_id; }

    [[nodiscard]] auto get_type() const -> TagOperationType { return m_type; }

    /**
     * @return A string representation of the tag operation.
     */
    [[nodiscard]] auto serialize() const -> std::string {
        switch (m_type) {
            case TagOperationType::Set:
                return fmt::format("{}{}", m_tag_id, "p");
            case TagOperationType::Negate:
                return fmt::format("{}{}", m_tag_id, "n");
            case TagOperationType::None:
                return "none";
        }
    }

private:
    tag_id_t m_tag_id;
    TagOperationType m_type;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAGOPERATION_HPP
