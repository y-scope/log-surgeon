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
/// and a sequence of [`Variable`]s to interpolate.
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
    CCharArray name;
    CCharArray lexeme;
    /// Nonzero for a valid capture.
    uint32_t id;
    uint32_t parent_id;
};

struct CVariable {
    size_t rule;
    CCharArray name;
    CCharArray lexeme;
};


extern "C" {

CCapture log_surgeon_log_event_capture(const LogEvent *log_event, size_t i, size_t j);

Box<LogEvent> log_surgeon_log_event_clone(const LogEvent *value);

void log_surgeon_log_event_drop(Box<LogEvent> value);

bool log_surgeon_log_event_have_header(const LogEvent *log_event);

CCharArray log_surgeon_log_event_log_type(const LogEvent *log_event);

Box<LogEvent> log_surgeon_log_event_new();

CVariable log_surgeon_log_event_variable(const LogEvent *log_event, size_t i);

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
