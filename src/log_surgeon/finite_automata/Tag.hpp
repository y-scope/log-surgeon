#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAG
#define LOG_SURGEON_FINITE_AUTOMATA_TAG

#include <string>
#include <string_view>
#include <utility>

namespace log_surgeon::finite_automata {
class Tag {
public:
    explicit Tag(std::string name) : m_name{std::move(name)} {}

    [[nodiscard]] auto get_name() const -> std::string_view { return m_name; }

private:
    std::string m_name;
};

class TagPositions {
public:
    explicit TagPositions(Tag const* tag) : m_tag{tag} {}

private:
    Tag const* m_tag;
    std::vector<uint32_t> start_positions;
    std::vector<uint32_t> end_positions;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAG
