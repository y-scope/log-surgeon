#ifndef LOG_SURGEON_LOG_PARSER_OUTPUT_BUFFER_HPP
#define LOG_SURGEON_LOG_PARSER_OUTPUT_BUFFER_HPP

#include <optional>

#include <log_surgeon/Buffer.hpp>
#include <log_surgeon/Token.hpp>

namespace log_surgeon {
/**
 * A buffer containing the tokenized output of the log parser. The first
 * token contains the timestamp (if there is no timestamp the first token is
 * unused). For performance (runtime latency) it defaults to a static
 * buffer and when more tokens are needed to be stored than the current
 * capacity, it switches to a dynamic buffer. Each time the capacity is
 * exceeded (i.e. advance_to_next_token causes the buffer pos to pass the
 * end of the buffer), the tokens are moved into a new dynamic buffer
 * with twice the size of the current buffer and is added to the list of
 * dynamic buffers.
 */
class LogParserOutputBuffer {
public:
    /**
     * Advances the position of the buffer so that it is at the next token.
     */
    auto advance_to_next_token() -> void;

    auto reset() -> void {
        m_has_header = false;
        m_timestamp = std::nullopt;
        m_has_delimiters = false;
        m_storage.reset();
    }

    auto set_has_header(bool has_header) -> void { m_has_header = has_header; }

    [[nodiscard]] auto has_header() const -> bool { return m_has_header; }

    auto set_timestamp(std::optional<std::string_view> const& timestamp) -> void {
        m_timestamp = timestamp;
    }

    [[nodiscard]] auto get_timestamp() const -> std::optional<std::string> const& {
        return m_timestamp;
    }

    auto set_has_delimiters(bool has_delimiters) -> void { m_has_delimiters = has_delimiters; }

    [[nodiscard]] auto has_delimiters() const -> bool { return m_has_delimiters; }

    auto set_token(uint32_t pos, Token& value) -> void { m_storage.set_value(pos, value); }

    [[nodiscard]] auto get_token(uint32_t pos) const -> Token const& {
        return m_storage.get_value(pos);
    }

    [[nodiscard]] auto get_mutable_token(uint32_t pos) const -> Token& {
        return m_storage.get_mutable_value(pos);
    }

    auto set_curr_token(Token& value) -> void { m_storage.set_curr_value(value); }

    [[nodiscard]] auto get_curr_token() const -> Token const& { return m_storage.get_curr_value(); }

    auto set_pos(uint32_t pos) -> void { m_storage.set_pos(pos); }

    [[nodiscard]] auto pos() const -> uint32_t { return m_storage.pos(); }

    [[nodiscard]] auto size() const -> uint32_t { return m_storage.size(); }

private:
    bool m_has_header{false};
    std::optional<std::string> m_timestamp;
    bool m_has_delimiters{false};
    // contains the static and dynamic Token buffers
    Buffer<Token> m_storage{};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LOG_PARSER_OUTPUT_BUFFER_HPP
