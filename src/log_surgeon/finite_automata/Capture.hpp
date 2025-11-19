#ifndef LOG_SURGEON_FINITE_AUTOMATA_CAPTURE
#define LOG_SURGEON_FINITE_AUTOMATA_CAPTURE

#include <string>
#include <string_view>
#include <utility>

namespace log_surgeon::finite_automata {
class Capture {
public:
    explicit Capture(std::string name) : m_name{std::move(name)} {}

    auto set_context(std::string rule_name, uint32_t pos) {
        m_rule_name = std::move(rule_name);
        m_pos = pos;
    }

    [[nodiscard]] auto get_name() const -> std::string const& { return m_name; }

private:
    std::string m_name;
    std::string m_rule_name;
    uint32_t m_pos{0};
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_CAPTURE
