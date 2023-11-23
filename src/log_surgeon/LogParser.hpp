#ifndef LOG_SURGEON_LOG_PARSER_HPP
#define LOG_SURGEON_LOG_PARSER_HPP

#include <cassert>
#include <iostream>
#include <memory>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/LALR1Parser.hpp>
#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/LogParserOutputBuffer.hpp>
#include <log_surgeon/Parser.hpp>
#include <log_surgeon/ParserInputBuffer.hpp>
#include <log_surgeon/SchemaParser.hpp>

namespace log_surgeon {
// TODO: Compare c-array vs. vectors (its underlying array) for buffers
class LogParser
        : public Parser<finite_automata::RegexNFAByteState, finite_automata::RegexDFAByteState> {
public:
    enum class ParsingAction {
        None,
        Compress,
        CompressAndFinish
    };

    /**
     * Constructs the parser using the given schema file.
     * @param schema_file_path
     * @throw std::runtime_error from LALR1Parser, RegexAST, or Lexer
     * describing the failure parsing the schema file or processing the schema
     * AST.
     */
    explicit LogParser(std::string const& schema_file_path);

    /**
     * Constructs the parser using the given schema object.
     * @param schema
     * @throw std::runtime_error from LALR1Parser, RegexAST, or Lexer
     * describing the failure processing the schema AST.
     */
    explicit LogParser(log_surgeon::SchemaAST const* schema_ast);

    /**
     * Returns the parser to its initial state, clearing any existing
     * parsed/lexed state.
     */
    auto reset() -> void;

    /**
     * Parses and generates metadata if parse was successful.
     * @param parsing_action Returns the action for CLP to take by reference.
     * @return ErrorCode::Success if successfully parsed to the start of a new
     * log event.
     * @return ErrorCode from LogParser::parse.
     */
    auto parse_and_generate_metadata(ParsingAction& parsing_action) -> ErrorCode;

    // TODO protect against invalid id (use optional)
    /**
     * @param id The integer ID of the symbol from the schema.
     * @return the name of the variable type / symbol from the schema using its
     * integer ID.
     */
    auto get_id_symbol(uint32_t id) const -> std::string { return m_lexer.m_id_symbol.at(id); }

    /**
     * @param symbol name of the variable type from the schema.
     * @return the integer ID corresponding to the symbol name on a successful
     * lookup.
     * @return nullopt if symbol was not found.
     */
    auto get_symbol_id(std::string const& symbol) const -> std::optional<uint32_t>;

    /**
     * Manually sets up the underlying input buffer. The ParserInputBuffer will
     * no longer use the currently set underlying storage and instead use what
     * is passed in.
     * @param storage A pointer to the new buffer to use as the input buffer.
     * @param size The size of the buffer pointed to by storage.
     * @param pos The current position into the buffer (storage) to set.
     * @param finished_reading_input True if there is no more input left to
     * process, so the parser will finish consuming the entire buffer without
     * requesting more input data.
     */
    auto set_input_buffer(char* storage, uint32_t size, uint32_t pos, bool finished_reading_input)
            -> void {
        m_input_buffer.set_storage(storage, size, pos, finished_reading_input);
    }

    /**
     * @return the current position inside the input buffer.
     */
    auto get_input_pos() -> uint32_t { return m_input_buffer.storage().pos(); }

    /**
     * Reads into the input buffer if only consumed data will be overwritten.
     * @param reader to use for IO.
     * @return ErrorCode::Success on successful read or if a read is unsafe.
     * @return ErrorCode forwarded from ParserInputBuffer::read_if_safe.
     */
    auto read_into_input(Reader& reader) -> ErrorCode {
        return m_input_buffer.read_if_safe(reader);
    }

    /**
     * Grows the capacity of the input buffer if it is not large enough to store
     * the contents of an entire LogEvent.
     */
    auto increase_capacity() -> void { m_lexer.increase_buffer_capacity(m_input_buffer); }

    /**
     * Resets the log event view to prepare for the next parse
     */
    auto reset_log_event_view() -> void { m_log_event_view->reset(); }

    /**
     * @return the log event view based on the last parse
     */
    auto get_log_event_view() const -> LogEventView const& { return *m_log_event_view; }

private:
    /**
     * Parses the input buffer until a complete log event has been parsed and
     * its tokens are stored into m_log_event_view.
     * @param parsing_action Returns the action for CLP to take by reference.
     * @return ErrorCode::Success if successfully parsed to the start of a new
     * log event.
     * @return ErrorCode from LogParser::get_next_symbol.
     */
    auto parse(ParsingAction& parsing_action) -> ErrorCode;

    /**
     * Generates metadata for last parsed log event indicating occurrences of
     * each variable and if the log event is multiline
     */
    auto generate_log_event_view_metadata() -> void;

    /**
     * Requests the next token from the lexer.
     * @param token is populated with the next token found by the parser.
     * @return ErrorCode::Success
     * @return ErrorCode::BufferOutOfBounds if end of input reached before
     * lexing a token.
     */
    auto get_next_symbol(Token& token) -> ErrorCode;

    /**
     * Adds delimiters (originally from the schema AST) to the log parser.
     * @param delimiters The AST object containing the delimiters to add.
     */
    auto add_delimiters(std::unique_ptr<ParserAST> const& delimiters) -> void;

    /**
     * Adds log parsing and lexing rules from the schema AST.
     * Delimiters are added to the start of regex patterns if delimiters are
     * specified in the schema AST.
     * @param schema_ast The AST from which parsing and lexing rules are
     * generated.
     */
    auto add_rules(SchemaAST const* schema_ast) -> void;

    // TODO: move ownership of the buffer to the lexer
    ParserInputBuffer m_input_buffer;
    bool m_has_start_of_log{false};
    Token m_start_of_log_message{};
    std::unique_ptr<LogEventView> m_log_event_view{nullptr};
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LOG_PARSER_HPP
