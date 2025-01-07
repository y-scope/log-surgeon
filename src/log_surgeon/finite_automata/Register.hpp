#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTER
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTER

#include <cstdint>

#include <log_surgeon/finite_automata/Tag.hpp>
enum class RegisterOperation {
    Assign,
    Append,
    Clear,
    None,
    Pop,
};

namespace log_surgeon::finite_automata {
class Register {
public:
    explicit Register(Tag* tag, bool const is_start) : m_tag{tag}, m_is_start(is_start) {}

    auto add_pos(uint32_t const pos) -> void { m_positions.push_back(pos); }

    auto update_last_position(uint32_t const pos) -> void { m_positions.back() = pos; }

    auto negate_last_position() -> void { m_positions.pop_back(); }

    auto negate_all_positions() -> void { m_positions.clear(); }

    [[nodiscard]] auto get_tag() const -> Tag* { return m_tag; }

    [[nodiscard]] auto is_start() const -> bool { return m_is_start; }

    [[nodiscard]] auto get_last_position() const -> uint32_t { return m_positions.back(); }

    [[nodiscard]] auto get_all_positions() const -> std::vector<uint32_t> const& {
        return m_positions;
    }

private:
    Tag* m_tag;
    bool m_is_start;
    std::vector<uint32_t> m_positions;
};

class RegisterOperator {
public:
    RegisterOperator(Register* reg, RegisterOperation const operation)
            : m_register(reg),
              m_operation(operation) {}

    [[nodiscard]] auto get_register() const -> Register* { return m_register; }

    [[nodiscard]] auto is_start() const -> bool { return m_register->is_start(); }

private:
    Register* m_register;
    RegisterOperation m_operation;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTER
