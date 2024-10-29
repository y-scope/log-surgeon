#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAG
#define LOG_SURGEON_FINITE_AUTOMATA_TAG

#include <cstdint>
#include <string>
#include <utility>
#include <vector>

namespace log_surgeon::finite_automata {
class Tag {
public:
    explicit Tag(std::string name) : m_name{std::move(name)} {}

    [[nodiscard]] auto get_name() const -> std::string const& { return m_name; }

private:
    std::string const m_name;
    std::vector<uint32_t> m_starts;
    std::vector<uint32_t> m_ends;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAG
