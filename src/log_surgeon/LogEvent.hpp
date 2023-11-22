#ifndef LOG_SURGEON_LOG_EVENT_HPP
#define LOG_SURGEON_LOG_EVENT_HPP

#include <memory>
#include <string>
#include <vector>

#include <log_surgeon/LogParserOutputBuffer.hpp>
#include <log_surgeon/Token.hpp>

namespace log_surgeon {
class LogParser;
class LogEvent;

/**
 * A class that represents a parsed log event. Contains ways to access parsed
 * variables and information from the original raw log event. All returned
 * `string_view`s point into the original source buffer containing the raw log
 * event. Thus, the components of a LogEventView are weak references to the
 * original buffer, and will become invalid if they exceed the lifetime of the
 * original buffer, or if the original buffer is mutated.
 */
class LogEventView {
public:
    /**
     * Constructs an empty LogEventView
     * @param log_parser The LogParser whose input buffer the view will
     * reference
     */
    explicit LogEventView(LogParser const& log_parser);

    /**
     * Copies the tokens representing a log event from the source buffer. This
     * allows the returned LogEvent to own all its tokens.
     * Equivalent to LogEvent::LogEvent(LogEventView const& src).
     * @return The LogEvent object made from this LogEventView.
     */
    [[nodiscard]] auto deep_copy() const -> LogEvent;

    /**
     * Reverts the LogEventView to its initial empty state by clearing all of
     * the Token references to its LogParser's input buffer.
     */
    auto reset() -> void;

    /**
     * NOTE: Currently, the returned Token(s) cannot be const as calling
     * Token::to_string or Token::to_string_view may mutate Token (to handle the
     * case where a token is wraps from the end to the beginning of a buffer).
     * @param var_id
     * @return The tokens corresponding to var_id
     */
    [[nodiscard]] auto get_variables(size_t var_id) const -> std::vector<Token*> const& {
        return m_log_var_occurrences[var_id];
    }

    /**
     * @return The LogParser whose input buffer this LogEventView references
     */
    [[nodiscard]] auto get_log_parser() const -> LogParser const& { return m_log_parser; }

    /**
     * @return The LogParserOutputBuffer containing the tokens that make up the
     * LogEventView.
     */
    [[nodiscard]] auto get_log_output_buffer() const
            -> std::unique_ptr<LogParserOutputBuffer> const& {
        return m_log_output_buffer;
    }

    /**
     * @return the Token corresponding to the LogEvent's timestamp.
     */
    [[nodiscard]] auto get_timestamp() const -> Token*;

    /**
     * @param multiline Whether the log event contains multiple lines.
     */
    auto set_multiline(bool multiline) -> void { m_multiline = multiline; }

    /**
     * @return Whether the log event spans multiple lines. A log event contains
     * multiple lines if it contains any character after a new line character.
     */
    [[nodiscard]] auto is_multiline() const -> bool { return m_multiline; }

    /**
     * Reconstructs the raw log event represented by the LogEventView by
     * iterating the event's tokens and copying the contents of each into a
     * string (similar to deep_copy).
     * @return The reconstructed raw log event.
     */
    [[nodiscard]] auto to_string() const -> std::string;

    /**
     * Constructs a user friendly/readable representation of the log event's
     * logtype. A logtype is essentially the static text of a log event with the
     * variable components replaced with their name. Therefore, two separate log
     * events from the same logging source code may have the same logtype.
     * @return The logtype of the log.
     */
    auto get_logtype() const -> std::string;

    /**
     * Adds a Token to the array of tokens of a particular token type.
     * @param token_type_id The ID of the variable/token type that token_ptr
     * will be added to.
     * @param token_ptr The token to add to the array of tokens with ID
     * token_type_id.
     */
    auto add_token(uint32_t token_type_id, Token* token_ptr) -> void {
        // TODO a Token knows all of its types through m_type_ids_ptr, so it
        // should be possible to remove token_type_id, or improve the use of
        // this function
        m_log_var_occurrences[token_type_id].push_back(token_ptr);
    }

    // TODO: have LogParser own the output buffer as a LogEventView is already
    // tied to a single log parser
    std::unique_ptr<LogParserOutputBuffer> m_log_output_buffer;

private:
    bool m_multiline{false};
    LogParser const& m_log_parser;
    std::vector<std::vector<Token*>> m_log_var_occurrences{};
};

/**
 * Contains all of the data necessary to store the log event. Essentially, this
 * has a copy of the source buffers' contents originally used by the parser and
 * tokens that point to the copied buffer rather than the original source
 * buffers.
 */
class LogEvent : public LogEventView {
public:
    /**
     * Constructs a LogEvent by copying the tokens representing a log event from
     * the source buffer. This allows the LogEvent to own all its tokens.
     * Equivalent to LogViewEvent::deep_copy().
     */
    LogEvent(LogEventView const& src);

private:
    std::vector<char> m_buffer;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LOG_EVENT_HPP
