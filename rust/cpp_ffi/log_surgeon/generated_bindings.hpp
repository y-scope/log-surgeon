// C++ declarations for C-ABI functions implemented in Rust.

// NOLINTBEGIN


#ifndef LOG_SURGEON_GENERATED_BINDINGS_HPP
#define LOG_SURGEON_GENERATED_BINDINGS_HPP

#include <cstddef>
#include <cstdint>
#include "rust_compat.hpp"
namespace log_surgeon {
// https://github.com/mozilla/cbindgen/issues/43
struct Capture;
}


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
struct Schema;

struct SchemaBuilder;

struct SearchResult;

template<typename T = void>
struct Vec;

struct RuleIdx {
    /// The original priority level given by the user.
    int32_t priority;
    /// Insertion order of the rule at the given priority level.
    uint16_t position;
    /// Index in the schema.
    uint16_t index;
};

/// Only reason we don't use [`std::ops::Range`] is because it isn't `Copy`
/// (by questionable design reasons).
struct CaptureRange {
    size_t start;
    size_t end;
};

template<typename T>
struct SpookyCArray {
    const T *pointer;
    size_t length;
    // Custom
    [[nodiscard]] auto as_cpp_view() const noexcept -> std::string_view
    requires std::is_same_v<T, char>
    {
        return {this->pointer, this->length};
    }
};

struct CaptureFfiPointers {
    const Capture *parent;
    SpookyCArray<char> lexeme;
    SpookyCArray<char> variable_name;
    SpookyCArray<char> capture_name;
};

struct Capture {
    RuleIdx rule_idx;
    /// Capture ID, statically assigned left-to-right based on the regex pattern;
    /// e.g. the pattern `(?<start>[a-z]+(?<rest>\.[a-z]+)*)|(?<start>[0-9]+)` has three capture IDs.
    /// When this variable/pattern is actually matched,
    /// there may be multiple instances of capture ID 2 (corresponding to `"rest"`).
    /// The capture ID also differentiates between different capture groups given the same name,
    /// e.g. the two instances of `"start"` in the pattern.
    uint32_t capture_id;
    uint32_t parent_id;
    /// `usize::MAX` if none.
    size_t parent_index;
    /// Offset of the capture in the log message.
    CaptureRange range;
    bool is_leaf;
    /// DANGEROUS fields for FFI.
    /// But it's not dangerous if you don't look at it.
    CaptureFfiPointers ffi_pointers;
};


extern "C" {

const Capture *log_surgeon_log_event_all_captures(const LogEvent *log_event, size_t *len);

Box<LogEvent> log_surgeon_log_event_clone(const LogEvent *value);

void log_surgeon_log_event_drop(Box<LogEvent> value);

const size_t *log_surgeon_log_event_leaf_capture_indices(const LogEvent *log_event, size_t *len);

CCharArray log_surgeon_log_event_log_type(const LogEvent *log_event);

Box<LogEvent> log_surgeon_log_event_new();

Box<Parser> log_surgeon_parser_clone(const Parser *value);

void log_surgeon_parser_drop(Box<Parser> value);

Box<Parser> log_surgeon_parser_new(Box<Schema> schema);

bool log_surgeon_parser_next(Parser *parser, CCharArray input, size_t *pos, LogEvent *out);

void log_surgeon_regex_error_drop(Box<RegexError> value);

Option<Box<RegexError>> log_surgeon_schema_builder_add_rule_with_priority(SchemaBuilder *builder,
                                                                          int32_t priority,
                                                                          CCharArray name,
                                                                          CCharArray pattern);

Box<Schema> log_surgeon_schema_builder_build(Box<SchemaBuilder> builder);

Box<SchemaBuilder> log_surgeon_schema_builder_new();

void log_surgeon_schema_builder_set_delimiters(SchemaBuilder *builder, CCharArray delimiters);

Option<Box<Schema>> log_surgeon_schema_from_definition(CCharArray definition);

Box<SearchResult> log_surgeon_search_by_named_type(const Schema *schema,
                                                   CCharArray name,
                                                   CCharArray value);

void log_surgeon_search_interpretations_drop(Box<Box<Vec<Interpretation>>> value);

CCharArray log_surgeon_search_query_interpretation_as_string(const Vec<Interpretation> *interpretations,
                                                             size_t i,
                                                             size_t *len);

Box<Vec<Interpretation>> log_surgeon_search_query_interpretations(const Parser *parser,
                                                                  CCharArray input);

void log_surgeon_search_result_drop(Box<SearchResult> value);

const Capture *log_surgeon_search_result_get_leaf_captures(const SearchResult *search_result,
                                                           size_t *len);

}  // extern "C"

}  // namespace log_surgeon

#endif  // LOG_SURGEON_GENERATED_BINDINGS_HPP

// NOLINTEND
