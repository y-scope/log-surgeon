#ifndef LOG_SURGEON_BUFFER_PARSER_HPP
#define LOG_SURGEON_BUFFER_PARSER_HPP

#include <optional>
#include <string>

#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/LogParser.hpp>
#include <log_surgeon/Schema.hpp>

namespace log_surgeon {
/**
 * A parser that parses log events from a buffer. The caller is responsible for
 * filling the buffer and handling log events that may be split across multiple
 * buffer instances (see get_next_view for details). For a parser that handles
 * this internally when reading from a file, see log_surgeon::ReaderParser.
 */
class BufferParser {
public:
    /**
     * Constructs the parser using the given schema file.
     * @param schema_file_path
     * @throw std::runtime_error from LALR1Parser, RegexAST, or Lexer
     * describing the failure parsing the schema file or processing the schema
     * AST.
     */
    explicit BufferParser(std::string const& schema_file_path);

    /**
     * Constructs the parser using the given schema object.
     * @param schema
     * @throw std::runtime_error from LALR1Parser, RegexAST, or Lexer
     * describing the failure processing the schema AST.
     */
    explicit BufferParser(Schema& schema);

    /**
     * Clears the internal state of the log parser (lexer and input buffer) so
     * that the next call to parse_next_event will begin parsing from
     * scratch. This is an alternative to constructing a new Parser that would
     * require rebuilding the LogParser (generating a new lexer and input
     * buffer). This should be called whenever you mutate the input buffer, but
     * is already called internally if get_next_log_view returns
     * ErrorCode::BufferOutOfBounds.
     */
    auto reset() -> void;

    /**
     * Attempts to parse the next log event from buf[offset:size]. The
     * bytes between offset and size may contain a partial log event. It is the
     * user's responsibility to preserve these bytes when mutating the buffer
     * to contain more of the log event before the next call of
     * get_next_log_view. The result is stored internally and is only valid if
     * ErrorCode::Success is returned.
     * @param buf The byte buffer containing raw log events to be parsed.
     * @param size The size of the buffer.
     * @param offset The starting position in the buffer of the current log
     * event to be parsed. Updated to be the starting position of the next
     * unparsed log event. If no log event is parsed it remains unchanged.
     * @param finished_reading_input Indicates if the end of the buffer is the
     * end of input and therefore the end of the final log event.
     * @return ErrorCode::Success if a log event is successfully parsed as a
     * LogEventView.
     * @return ErrorCode::BufferOutOfBounds if the end of the log event is not
     * found after scanning the entire buffer. In this case, `reset` is called
     * internally before this method returns.
     * @return ErrorCode from LogParser::parse.
     */
    auto
    parse_next_event(char* buf, size_t size, size_t& offset, bool finished_reading_input = false)
            -> ErrorCode;

    /**
     * @return The underlying LogParser.
     */
    auto get_log_parser() const -> LogParser const& { return m_log_parser; }

    /**
     * @param var The name of the variable as provided in the schema file or
     * when building the LogParser's Schema object.
     * @return nullopt If var is not found in the schema.
     * @return The integer ID of the variable.
     */
    auto get_variable_id(std::string const& var) -> std::optional<uint32_t> {
        return m_log_parser.get_symbol_id(var);
    }

    /**
     * @return true when the BufferParser has completed parsing all of the
     * provided input. This can only occur if finished_reading_input was set to
     * true in parse_next_event. Otherwise, the BufferParser will always
     * assume more input can be read.
     */
    auto done() const -> bool { return m_done; }

private:
    LogParser m_log_parser;
    bool m_done{false};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_BUFFER_PARSER_HPP
