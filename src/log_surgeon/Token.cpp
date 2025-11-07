#include "Token.hpp"

#include <cstddef>
#include <string>
#include <string_view>

namespace log_surgeon {
auto Token::get_cached_string() -> std::string const& {
    if (m_cached_string.empty()) {
        if (get_start_pos() <= get_end_pos()) {
            auto const token{m_buffer.subspan(get_start_pos(), get_end_pos() - get_start_pos())};
            m_cached_string = std::string{token.begin(), token.end()};
        } else {
            auto const token_start{
                    m_buffer.subspan(get_start_pos(), get_buffer_size() - get_start_pos())
            };
            auto const token_end{m_buffer.subspan(0, get_end_pos())};
            m_cached_string = std::string{token_start.begin(), token_start.end()}
                              + std::string{token_end.begin(), token_end.end()};
        }
    }
    return m_cached_string;
}

auto Token::to_string() -> std::string {
    return {get_cached_string()};
}

auto Token::to_string_view() -> std::string_view {
    if (get_start_pos() <= get_end_pos()) {
        auto const token{m_buffer.subspan(get_start_pos(), get_end_pos() - get_start_pos())};
        return {token.begin(), token.end()};
    }
    return {get_cached_string()};
}

auto Token::get_delimiter() const -> std::string {
    auto const delim{m_buffer.subspan(get_start_pos(), 1)};
    return {delim.begin(), delim.end()};
}

auto Token::get_length() const -> size_t {
    if (get_start_pos() <= get_end_pos()) {
        return get_end_pos() - get_start_pos();
    }
    return get_buffer_size() - get_start_pos() + get_end_pos();
}

auto Token::release_delimiter() -> char {
    auto const delim{m_buffer[get_start_pos()]};
    increment_start_pos();
    return delim;
}

auto Token::set_start_pos(size_t pos) -> void {
    m_cached_string.clear();
    m_start_pos = pos;
}

auto Token::set_end_pos(size_t pos) -> void {
    m_cached_string.clear();
    m_end_pos = pos;
}

auto Token::increment_start_pos() -> size_t {
    auto const old_start_pos{get_start_pos()};
    set_start_pos(get_next_pos());
    return old_start_pos;
}

auto Token::get_next_pos() const -> size_t {
    auto next_pos{get_start_pos() + 1};
    if (next_pos == get_buffer_size()) {
        next_pos = 0;
    }
    return next_pos;
}
}  // namespace log_surgeon
