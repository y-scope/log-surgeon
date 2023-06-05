#include <string>

#include "BufferParser.hpp"
#include "Constants.hpp"
#include "LogEvent.hpp"
#include "Schema.hpp"

namespace log_surgeon {
BufferParser::BufferParser(Schema& schema) : m_log_parser(schema.get_schema_ast_ptr()) {}

BufferParser::BufferParser(std::string const& schema_file_path)
    : m_log_parser(LogParser(schema_file_path)) {}

auto BufferParser::reset() -> void {
    m_log_parser.reset();
    m_done = false;
}

auto BufferParser::get_next_event_view(char* buf,
                                       size_t size,
                                       size_t& offset,
                                       LogEventView& event_view,
                                       bool finished_reading_input) -> ErrorCode {
    event_view.reset();
    // TODO in order to allow logs/tokens to wrap user buffers this function
    // will need more parameters or the input buffer may need to be exposed to
    // the user
    m_log_parser.set_input_buffer(buf, size, offset, finished_reading_input);
    LogParser::ParsingAction parsing_action{LogParser::ParsingAction::None};
    ErrorCode error_code = m_log_parser.parse(event_view.m_log_output_buffer, parsing_action);
    if (ErrorCode::Success != error_code) {
        if (0 != event_view.m_log_output_buffer->pos()) {
            offset = event_view.m_log_output_buffer->get_token(0).m_start_pos;
        }
        reset();
        return error_code;
    }
    if (LogParser::ParsingAction::CompressAndFinish == parsing_action) {
        m_done = true;
    }
    offset = m_log_parser.get_input_pos();

    uint32_t start = 0;
    if (false == event_view.m_log_output_buffer->has_timestamp()) {
        start = 1;
    }
    uint32_t first_newline_pos{0};
    for (uint32_t i = start; i < event_view.m_log_output_buffer->pos(); i++) {
        Token* token = &event_view.m_log_output_buffer->get_mutable_token(i);
        event_view.add_token(token->m_type_ids_ptr->at(0), token);
        if (token->m_type_ids_ptr->at(0) == (int)SymbolID::TokenNewlineId &&
            first_newline_pos == 0) {
            first_newline_pos = i;
        }
    }
    // To be a multiline log there must be at least one token between the
    // newline token and the last token in the output buffer.
    if (event_view.m_log_output_buffer->has_timestamp() && 0 < first_newline_pos &&
        first_newline_pos + 1 < event_view.m_log_output_buffer->pos()) {
        event_view.set_multiline(true);
    }
    return ErrorCode::Success;
}
} // namespace log_surgeon
