#include "Token.hpp"

#include <string>
#include <string_view>

namespace log_surgeon {

auto Token::to_string() -> std::string {
    if (m_start_pos <= m_end_pos) {
        return {m_buffer + m_start_pos, m_buffer + m_end_pos};
    }
    if (m_wrap_around_string.empty()) {
        m_wrap_around_string = std::string{m_buffer + m_start_pos, m_buffer + m_buffer_size}
                               + std::string{m_buffer, m_buffer + m_end_pos};
    }
    return {m_wrap_around_string};
}

auto Token::to_string_view() -> std::string_view {
    if (m_start_pos <= m_end_pos) {
        return {m_buffer + m_start_pos, m_end_pos - m_start_pos};
    }
    if (m_wrap_around_string.empty()) {
        m_wrap_around_string = std::string{m_buffer + m_start_pos, m_buffer + m_buffer_size}
                               + std::string{m_buffer, m_buffer + m_end_pos};
    }
    return {m_wrap_around_string};
}

auto Token::get_char(uint8_t i) const -> char {
    if (m_start_pos + i < m_buffer_size) {
        return m_buffer[m_start_pos + i];
    }
    return m_buffer[i - (m_buffer_size - m_start_pos)];
}

auto Token::get_delimiter() const -> std::string {
    return {m_buffer + m_start_pos, m_buffer + m_start_pos + 1};
}

auto Token::get_length() const -> uint32_t {
    if (m_start_pos <= m_end_pos) {
        return m_end_pos - m_start_pos;
    }
    return m_buffer_size - m_start_pos + m_end_pos;
}
}  // namespace log_surgeon
