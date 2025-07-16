#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAGOPERATION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_TAGOPERATION_HPP

#include <cstdint>
#include <string>
#include <tuple>

#include <log_surgeon/types.hpp>

#include <fmt/core.h>

namespace log_surgeon::finite_automata {
enum class TagOperationType : uint8_t {
    Set,
    Negate
};

class TagOperation {
public:
    TagOperation(tag_id_t const tag_id, TagOperationType const type, bool const multi_valued)
            : m_tag_id{tag_id},
              m_type{type},
              m_multi_valued(multi_valued) {}

    [[nodiscard]] auto operator<(TagOperation const& rhs) const -> bool {
        return std::tie(m_tag_id, m_type) < std::tie(rhs.m_tag_id, rhs.m_type);
    }

    [[nodiscard]] auto operator==(TagOperation const& rhs) const -> bool {
        return std::tie(m_tag_id, m_type) == std::tie(rhs.m_tag_id, rhs.m_type);
    }

    [[nodiscard]] auto get_tag_id() const -> tag_id_t { return m_tag_id; }

    [[nodiscard]] auto get_type() const -> TagOperationType { return m_type; }

    [[nodiscard]] auto is_multi_valued() const -> bool { return m_multi_valued; }

    /**
     * @return A string representation of the tag operation.
     */
    [[nodiscard]] auto serialize() const -> std::string {
        char type_char = '?';
        switch (m_type) {
            case TagOperationType::Set:
                type_char = 'p';
                break;
            case TagOperationType::Negate:
                type_char = 'n';
                break;
        }
        return fmt::format("{}{}{}", m_tag_id, type_char, m_multi_valued ? "+" : "");
    }

private:
    tag_id_t m_tag_id;
    TagOperationType m_type;
    bool m_multi_valued;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAGOPERATION_HPP
