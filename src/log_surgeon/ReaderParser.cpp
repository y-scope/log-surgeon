#include "ReaderParser.hpp"

#include <string>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/Schema.hpp>

namespace log_surgeon {
ReaderParser::ReaderParser(Schema& schema) : m_log_parser(schema.get_schema_ast_ptr()) {}

ReaderParser::ReaderParser(std::string const& schema_file_path) : m_log_parser(schema_file_path) {}

auto ReaderParser::reset_and_set_reader(Reader& reader) -> void {
    m_done = false;
    m_log_parser.reset();
    m_reader = reader;
}

auto ReaderParser::parse_next_event() -> ErrorCode {
    m_log_parser.reset_log_event_view();
    if (ErrorCode err = m_log_parser.read_into_input(m_reader);
        ErrorCode::Success != err && ErrorCode::EndOfFile != err)
    {
        return err;
    }
    while (true) {
        LogParser::ParsingAction parsing_action{LogParser::ParsingAction::None};
        ErrorCode parse_error = m_log_parser.parse_and_generate_metadata(parsing_action);
        if (ErrorCode::Success == parse_error) {
            if (LogParser::ParsingAction::CompressAndFinish == parsing_action) {
                m_done = true;
            }
            break;
        }
        if (ErrorCode::BufferOutOfBounds == parse_error) {
            m_log_parser.increase_capacity();
            if (ErrorCode err = m_log_parser.read_into_input(m_reader);
                ErrorCode::Success != err && ErrorCode::EndOfFile != err)
            {
                return err;
            }
        } else {
            return parse_error;
        }
    }
    return ErrorCode::Success;
}
}  // namespace log_surgeon
