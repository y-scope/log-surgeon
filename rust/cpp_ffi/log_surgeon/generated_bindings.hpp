// C++ declarations for C-ABI functions implemented in Rust.

// NOLINTBEGIN


#ifndef LOG_SURGEON_GENERATED_BINDINGS_HPP
#define LOG_SURGEON_GENERATED_BINDINGS_HPP

#include <cstddef>
#include <cstdint>
#include "rust_compat.hpp"


namespace log_surgeon {

struct Interpretation;

/// A `LogEvent` has a template [`LogType`](crate::log_type::LogType).
/// and a sequence of [`Capture`]s to interpolate.
struct LogEvent;

struct Parser;

struct RegexError;

/// A `Schema` is conceptually a list of rules and a set of delimiter characters.
///
/// [`Rule`]s may be added with a specific integer priority;
/// larger integer value means higher priority.
/// Within a priority level, rules are prioritized by insertion order.
///
/// Before automata construction, rules are "flattened", ordered by priority (highest first).
/// A special `0`th rule internally represents a "newline" token.
/// Rules are "ID"ed by their index in this flattened priority list.
///
struct Schema;

template<typename T = void>
struct Vec;

struct CCapture {
    size_t rule_id;
    /// `None`/zero when it is an implicit capture of the entire variable pattern.
    uint32_t capture_id;
    uint32_t parent_id;
    /// Offset relative to start of log event message.
    size_t start;
    /// Offset relative to start of log event message.
    size_t end;
    CCharArray variable_name;
    CCharArray capture_name;
    CCharArray lexeme;
};


extern "C" {

Box<LogEvent> log_surgeon_log_event_clone(const LogEvent *value);

void log_surgeon_log_event_drop(Box<LogEvent> value);

CCapture log_surgeon_log_event_get_capture(const LogEvent *log_event,
                                           size_t i,
                                           const Parser *parser);

bool log_surgeon_log_event_get_variable_window(const LogEvent *log_event,
                                               size_t i,
                                               size_t *start,
                                               size_t *end);

CCharArray log_surgeon_log_event_log_type(const LogEvent *log_event);

Box<LogEvent> log_surgeon_log_event_new();

Box<Parser> log_surgeon_parser_clone(const Parser *value);

void log_surgeon_parser_drop(Box<Parser> value);

Box<Parser> log_surgeon_parser_new(const Schema *schema);

bool log_surgeon_parser_next(Parser *parser, CCharArray input, size_t *pos, LogEvent *out);

void log_surgeon_regex_error_drop(Box<RegexError> value);

Option<Box<RegexError>> log_surgeon_schema_add_rule_with_priority(Schema *schema,
                                                                  int32_t priority,
                                                                  CCharArray name,
                                                                  CCharArray pattern);

void log_surgeon_schema_drop(Box<Schema> value);

Box<Schema> log_surgeon_schema_new();

void log_surgeon_schema_set_delimiters(Schema *schema, CCharArray delimiters);

void log_surgeon_search_interpretations_drop(Box<Box<Vec<Interpretation>>> value);

CCharArray log_surgeon_search_query_interpretation_as_string(const Vec<Interpretation> *interpretations,
                                                             size_t i,
                                                             size_t *len);

Box<Vec<Interpretation>> log_surgeon_search_query_interpretations(const Parser *parser,
                                                                  CCharArray input);

}  // extern "C"

}  // namespace log_surgeon

#endif  // LOG_SURGEON_GENERATED_BINDINGS_HPP

// NOLINTEND
