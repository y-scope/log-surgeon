#include "Token.hpp"

#include <algorithm>>
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

void Token::append_context_to_logtype(
        std::vector<std::pair<reg_id_t, reg_id_t>> const& reg_id_pairs,
        std::vector<capture_id_t> const& capture_ids,
        std::function<std::string(capture_id_t)> const& tag_formatter,
        std::string& logtype
) const {
    struct VariablePosition {
        uint32_t m_start_pos;
        uint32_t m_end_pos;
        capture_id_t m_capture_id;
    };

    std::vector<VariablePosition> variable_positions;

    for (size_t i{0}; i < reg_id_pairs.size(); ++i) {
        auto const& [start_reg_id, end_reg_id]{reg_id_pairs[i]};
        auto const capture_id{capture_ids[i]};

        auto const reg_start_positions{m_reg_handler.get_reversed_positions(start_reg_id)};
        auto const reg_end_positions{m_reg_handler.get_reversed_positions(end_reg_id)};

        for (size_t j{0}; j < reg_start_positions.size(); ++j) {
            if (reg_start_positions[j] < 0) {
                continue;
            }
            variable_positions.push_back(
                    {static_cast<uint32_t>(reg_start_positions[j]),
                     static_cast<uint32_t>(reg_end_positions[j]),
                     capture_id}
            );
        }
    }

    std::sort(
            variable_positions.begin(),
            variable_positions.end(),
            [](auto const& a, auto const& b) { return a.m_start_pos < b.m_start_pos; }
    );
    variable_positions.push_back({m_end_pos, m_end_pos, 0});

    uint32_t prev{m_start_pos};
    for (auto const& variable_position : variable_positions) {
        if (prev <= variable_position.m_start_pos) {
            logtype.append(m_buffer + prev, variable_position.m_start_pos - prev);
        } else {
            logtype.append(m_buffer + prev, m_buffer + m_buffer_size);
            logtype.append(m_buffer, m_buffer + variable_position.m_start_pos);
        }
        // Skip adding tags for zero-length captures
        if (variable_position.m_start_pos != variable_position.m_end_pos) {
            logtype += tag_formatter(variable_position.m_capture_id);
        }
        prev = variable_position.m_end_pos;
    }
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
