#ifndef LOG_SURGEON_UNIQUEIDGENERATOR_HPP
#define LOG_SURGEON_UNIQUEIDGENERATOR_HPP

#include <cstdint>

namespace log_surgeon {
class UniqueIdGenerator {
public:
    [[nodiscard]] auto generate_id() -> uint32_t { return m_current_id++; }

    [[nodiscard]] auto get_num_ids() const -> uint32_t { return m_current_id; }

private:
    uint32_t m_current_id{0};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_UNIQUEIDGENERATOR_HPP
