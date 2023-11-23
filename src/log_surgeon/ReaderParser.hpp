#ifndef LOG_SURGEON_READER_PARSER_HPP
#define LOG_SURGEON_READER_PARSER_HPP

#include <optional>
#include <string>

#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/LogParser.hpp>
#include <log_surgeon/Reader.hpp>
#include <log_surgeon/Schema.hpp>

namespace log_surgeon {
/**
 * A parser that parses log events from a log_surgeon::Reader. For a parser that
 * parses from a given buffer, see log_surgeon::BufferParser.
 */
class ReaderParser {
public:
    /**
     * Constructs the parser using the the given schema file.
     * @param schema_file_path
     * @throw std::runtime_error from LALR1Parser, RegexAST, or Lexer
     * describing the failure parsing the schema file or processing the schema
     * AST.
     */
    explicit ReaderParser(std::string const& schema_file_path);

    /**
     * Constructs the parser using the given schema object.
     * @param schema
     * @throw std::runtime_error from LALR1Parser, RegexAST, or Lexer
     * describing the failure processing the schema AST.
     */
    explicit ReaderParser(Schema& schema);

    /**
     * Clears the internal state of the log parser (lexer and input buffer),
     * and sets the reader containing the logs to be parsed. The next call to
     * parse_next_event will begin parsing from scratch. This is an
     * alternative to constructing a new Parser that would require rebuilding
     * the LogParser (generating a new lexer and input buffer). This should be
     * called whenever new input is needed.
     * @param reader
     */
    auto reset_and_set_reader(Reader& reader) -> void;

    /**
     * Attempts to parse the next log event from the internal `Reader`. Users
     * should add their own error handling and tracking logic to Reader::read,
     * in order to retrieve IO errors. The result is stored internally and is
     * only valid if ErrorCode::Success is returned.
     * @return ErrorCode::Success if a log event is successfully parsed as a
     * LogEventView.
     * @return ErrorCode from LogParser::parse.
     * @return ErrorCode from the user defined Reader::read.
     * @throw std::bad_alloc if a log event is large enough to exhaust memory.
     */
    auto parse_next_event() -> ErrorCode;

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
    auto get_variable_id(std::string const& var) const -> std::optional<uint32_t> {
        return m_log_parser.get_symbol_id(var);
    }

    /**
     * @return true when the ReaderParser has completed parsing all of the
     * input from the Reader.
     */
    auto done() const -> bool { return m_done; }

private:
    Reader m_reader;
    LogParser m_log_parser;
    bool m_done{false};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_READER_PARSER_HPP
