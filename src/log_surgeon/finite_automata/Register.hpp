#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTER
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTER

#include <cstdint>

#include <log_surgeon/finite_automata/Tag.hpp>

namespace log_surgeon::finite_automata {
class Register {
public:
    explicit Register(Tag* tag) : m_tag{tag} {}

    auto add_pos(uint32_t const pos) -> void { positions.push_back(pos); }

    auto update_last_position(uint32_t const pos) -> void { positions.back() = pos; }

    auto negate_last_position() -> void { positions.pop_back(); }

    auto negate_all_positions() -> void { positions.clear(); }

    [[nodiscard]] auto get_tag() const -> Tag* { return m_tag; }

    [[nodiscard]] auto get_last_position() const -> uint32_t { return positions.back(); }

    [[nodiscard]] auto get_all_positions() const -> std::vector<uint32_t> const& {
        return positions;
    }

private:
    Tag* m_tag;
    std::vector<uint32_t> positions;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTER
