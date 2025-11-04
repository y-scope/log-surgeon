#ifndef LOG_SURGEON_TOKEN_HPP
#define LOG_SURGEON_TOKEN_HPP

#include <cstddef>
#include <cstdint>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include <log_surgeon/finite_automata/PrefixTree.hpp>
#include <log_surgeon/finite_automata/RegisterHandler.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon {
class Token {
public:
    Token() = default;

    Token(size_t start_pos,
          size_t end_pos,
          char const* buf,
          size_t buf_size,
          size_t line_num,
          std::vector<uint32_t> const* type_ids,
          finite_automata::RegisterHandler reg_handler)
            : m_start_pos{start_pos},
              m_end_pos{end_pos},
              m_buffer{buf, buf_size},
              m_line_num{line_num},
              m_type_ids_ptr{type_ids},
              m_reg_handler{std::move(reg_handler)} {}

    Token(size_t start_pos,
          size_t end_pos,
          char const* buf,
          size_t buf_size,
          size_t line_num,
          std::vector<uint32_t> const* type_ids)
            : Token(start_pos, end_pos, buf, buf_size, line_num, type_ids, {}) {}

    /**
     * Construct and cache a string representation of the token.
     * @return The token's value as a string.
     */
    [[nodiscard]] auto to_string() -> std::string;

    /**
     * In the common case, only construct a string_view of the token's underlying buffer. If the
     * buffer is wrapped, a string is construct and cached before returning a view of it.
     * @return The token's value as a string_view.
     */
    [[nodiscard]] auto to_string_view() -> std::string_view;

    /**
     * @return The first character (as a string) of the token string (which is a
     * delimiter if delimiters are being used)
     */
    [[nodiscard]] auto get_delimiter() const -> std::string;

    /**
     * @return The length of the token string
     */
    [[nodiscard]] auto get_length() const -> size_t;

    [[nodiscard]] auto get_reversed_reg_positions(reg_id_t const reg_id) const
            -> std::vector<finite_automata::PrefixTree::position_t> {
        return m_reg_handler.get_reversed_positions(reg_id);
    }

    /**
     * @return The leading delimiter character and remove it.
     */
    [[nodiscard]] auto release_delimiter() -> char;

    [[nodiscard]] auto get_start_pos() const -> size_t { return m_start_pos; }

    /**
     * Set the token's start position. Clears the cached string as the token has changed.
     */
    auto set_start_pos(size_t pos) -> void;

    [[nodiscard]] auto get_end_pos() const -> size_t { return m_end_pos; }

    /**
     * Set the token's end position. Clears the cached string as the token has changed.
     */
    auto set_end_pos(size_t pos) -> void;

    [[nodiscard]] auto get_buffer_size() const -> size_t { return m_buffer.size(); }

    /**
     * @return The line number the token appears on.
     */
    [[nodiscard]] auto get_line_num() const -> size_t { return m_line_num; }

    [[nodiscard]] auto get_type_ids() const -> std::vector<uint32_t> const* {
        return m_type_ids_ptr;
    }

    auto set_type_ids(std::vector<uint32_t> const* type_ids) -> void { m_type_ids_ptr = type_ids; }

    /**
     * Increment the start position by 1, wrapping if necessary. Clears the cached string as the
     * token has changed.
     * @return The previous start position.
     */
    auto increment_start_pos() -> size_t;

    /**
     * @return The next position after the start position, wrapping if necessary.
     */
    [[nodiscard]] auto get_next_pos() const -> size_t;

private:
    /**
     * @return The cached string, computing it first if it is empty.
     */
    auto get_cached_string() -> std::string const&;

    size_t m_start_pos{0};
    size_t m_end_pos{0};
    std::span<char const> m_buffer;
    size_t m_line_num{0};
    std::vector<uint32_t> const* m_type_ids_ptr{nullptr};
    finite_automata::RegisterHandler m_reg_handler{};
    std::string m_cached_string;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_TOKEN_HPP
