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

auto ReaderParser::get_next_event_view(LogEventView& event_view) -> ErrorCode {
    event_view.reset();
    if (ErrorCode err = m_log_parser.read_into_input(m_reader);
        ErrorCode::Success != err && ErrorCode::EndOfFile != err)
    {
        return err;
    }
    while (true) {
        LogParser::ParsingAction parsing_action{LogParser::ParsingAction::None};
        ErrorCode parse_error = m_log_parser.parse(event_view.m_log_output_buffer, parsing_action);
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
    uint32_t start = 0;
    if (false == event_view.m_log_output_buffer->has_timestamp()) {
        start = 1;
    }
    uint32_t first_newline_pos{0};
    for (uint32_t i = start; i < event_view.m_log_output_buffer->pos(); i++) {
        Token* token = &event_view.m_log_output_buffer->get_mutable_token(i);
        event_view.add_token(token->m_type_ids_ptr->at(0), token);
        if (token->m_type_ids_ptr->at(0) == (int)SymbolID::TokenNewlineId && first_newline_pos == 0)
        {
            first_newline_pos = i;
        }
    }
    // To be a multiline log there must be at least one token between the
    // newline token and the last token in the output buffer.
    if (event_view.m_log_output_buffer->has_timestamp() && 0 < first_newline_pos
        && first_newline_pos + 1 < event_view.m_log_output_buffer->pos())
    {
        event_view.set_multiline(true);
    }
    return ErrorCode::Success;
}
}  // namespace log_surgeon
