#ifndef LOG_SURGEON_FINITE_AUTOMATA_CAPTURE
#define LOG_SURGEON_FINITE_AUTOMATA_CAPTURE

#include <string>
#include <utility>

namespace log_surgeon::finite_automata {
class Capture {
public:
    explicit Capture(std::string name) : m_name{std::move(name)} {}

    [[nodiscard]] auto get_name() const -> std::string const& { return m_name; }

private:
    std::string m_name;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_CAPTURE
