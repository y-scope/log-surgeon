#ifndef TOKEN_HPP
#define TOKEN_HPP

// C++ standard libraries
#include <set>
#include <string>
#include <string_view>
#include <vector>

namespace log_surgeon {
class Token {
public:
    Token() = default;

    Token(uint32_t start_pos,
          uint32_t end_pos,
          char const* buffer,
          uint32_t buffer_size,
          uint32_t line,
          std::vector<int> const* type_ids_ptr)
        : m_start_pos(start_pos),
          m_end_pos(end_pos),
          m_buffer(buffer),
          m_buffer_size(buffer_size),
          m_line(line),
          m_type_ids_ptr(type_ids_ptr) {}

    /**
     * Sets a token from parsing
     * @param start_pos
     * @param end_pos
     * @param buffer
     * @param buffer_size
     * @param line
     * @param type_ids_ptr
     */
    auto assign_with_ids_vector(uint32_t start_pos,
                                uint32_t end_pos,
                                char const* buffer,
                                uint32_t buffer_size,
                                uint32_t line,
                                std::vector<int> const* type_ids_ptr) -> void;

    /**
     * Sets a token for search
     * @param start_pos
     * @param end_pos
     * @param buffer
     * @param buffer_size
     * @param line
     * @param type_ids_set
     */
    auto assign_with_ids_set(uint32_t start_pos,
                             uint32_t end_pos,
                             char const* buffer,
                             uint32_t buffer_size,
                             uint32_t line,
                             std::set<int> const& type_ids_set) -> void;

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

    uint32_t m_start_pos{0};
    uint32_t m_end_pos{0};
    char const* m_buffer{nullptr};
    uint32_t m_buffer_size{0};
    uint32_t m_line{0};
    std::vector<int> const* m_type_ids_ptr{nullptr};
    std::string m_wrap_around_string{};
};
} // namespace log_surgeon

#endif // TOKEN_HPP
