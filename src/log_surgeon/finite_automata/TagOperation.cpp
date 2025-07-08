#include <log_surgeon/finite_automata/TagOperation.hpp>

#include <fmt/core.h>

namespace log_surgeon::finite_automata {
[[nodiscard]] auto TagOperation::serialize() const -> std::string {
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
}  // namespace log_surgeon::finite_automata
