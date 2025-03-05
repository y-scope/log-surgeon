#ifndef LOG_SURGEON_TOKEN_HPP
#define LOG_SURGEON_TOKEN_HPP

#include <string>
#include <string_view>
#include <vector>

#include <log_surgeon/finite_automata/PrefixTree.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon {
class Token {
public:
    auto set_reg_handler(finite_automata::RegisterHandler reg_handler) -> void {
        m_reg_handler = std::move(reg_handler);
    }

    /**
     * @return The token's value as a string
     */
    [[nodiscard]] auto to_string() -> std::string;
    /**
     * @return A string view of the token's value
     */
    [[nodiscard]] auto to_string_view() -> std::string_view;

    /**
     * @return The first character (as a string) of the token string (which is a
     * delimiter if delimiters are being used)
     */
    [[nodiscard]] auto get_delimiter() const -> std::string;

    /**
     * @param i
     * @return The ith character of the token string
     */
    [[nodiscard]] auto get_char(uint8_t i) const -> char;

    /**
     * @return The length of the token string
     */
    [[nodiscard]] auto get_length() const -> uint32_t;

    [[nodiscard]] auto get_reg_positions(reg_id_t const reg_id
    ) const -> std::vector<finite_automata::PrefixTree::position_t> {
        return m_reg_handler.get_reversed_positions(reg_id);
    }

    uint32_t m_start_pos{0};
    uint32_t m_end_pos{0};
    char const* m_buffer{nullptr};
    uint32_t m_buffer_size{0};
    uint32_t m_line{0};
    std::vector<uint32_t> const* m_type_ids_ptr{nullptr};
    finite_automata::RegisterHandler m_reg_handler{};
    std::string m_wrap_around_string{};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_TOKEN_HPP
