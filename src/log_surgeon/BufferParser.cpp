#include "BufferParser.hpp"

#include <string>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/LogEvent.hpp>
#include <log_surgeon/Schema.hpp>

namespace log_surgeon {
BufferParser::BufferParser(Schema& schema) : m_log_parser(schema.get_schema_ast_ptr()) {}

BufferParser::BufferParser(std::string const& schema_file_path)
        : m_log_parser(LogParser(schema_file_path)) {}

auto BufferParser::reset() -> void {
    m_log_parser.reset();
    m_done = false;
}

auto BufferParser::parse_next_event(
        char* buf,
        size_t size,
        size_t& offset,
        bool finished_reading_input
) -> ErrorCode {
    m_log_parser.reset_log_event_view();
    // TODO in order to allow logs/tokens to wrap user buffers this function
    // will need more parameters or the input buffer may need to be exposed to
    // the user
    m_log_parser.set_input_buffer(buf, size, offset, finished_reading_input);
    LogParser::ParsingAction parsing_action{LogParser::ParsingAction::None};
    ErrorCode error_code = m_log_parser.parse_and_generate_metadata(parsing_action);
    if (ErrorCode::Success != error_code) {
        if (0 != m_log_parser.get_log_event_view().m_log_output_buffer->pos()) {
            offset = m_log_parser.get_log_event_view()
                             .m_log_output_buffer->get_token(0)
                             .m_start_pos;
        }
        reset();
        return error_code;
    }
    if (LogParser::ParsingAction::CompressAndFinish == parsing_action) {
        m_done = true;
    }
    offset = m_log_parser.get_input_pos();
    return ErrorCode::Success;
}
}  // namespace log_surgeon
