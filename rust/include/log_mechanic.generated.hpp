#include <cstdint>
#include "log_mechanic.compat.hpp"


namespace log_mechanic {

struct LogEvent;

struct Parser;

struct RegexError;

struct Schema;

struct CCapture {
    CCharArray name;
    CCharArray lexeme;
    /// Nonzero for a valid capture.
    uint32_t id;
    uint32_t parent_id;
    bool is_leaf;
};

struct CVariable {
    size_t rule;
    CCharArray name;
    CCharArray lexeme;
};


extern "C" {

CCapture logmech_log_event_capture(const LogEvent *log_event, size_t i, size_t j);

Box<LogEvent> logmech_log_event_clone(const LogEvent *value);

void logmech_log_event_drop(Box<LogEvent> value);

bool logmech_log_event_have_header(const LogEvent *log_event);

CCharArray logmech_log_event_log_type(const LogEvent *log_event);

Box<LogEvent> logmech_log_event_new();

CVariable logmech_log_event_variable(const LogEvent *log_event, size_t i);

Box<Parser> logmech_parser_clone(const Parser *value);

void logmech_parser_drop(Box<Parser> value);

Box<Parser> logmech_parser_new(const Schema *schema);

bool logmech_parser_next(Parser *parser, CCharArray input, size_t *pos, LogEvent *out);

void logmech_regex_error_drop(Box<RegexError> value);

Option<Box<RegexError>> logmech_schema_add_rule(Schema *schema,
                                                CCharArray name,
                                                CCharArray pattern);

void logmech_schema_drop(Box<Schema> value);

Box<Schema> logmech_schema_new();

void logmech_schema_set_delimiters(Schema *schema, CCharArray delimiters);

}  // extern "C"

}  // namespace log_mechanic
