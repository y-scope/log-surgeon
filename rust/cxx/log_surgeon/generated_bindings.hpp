// C++ declarations for C-ABI functions implemented in Rust.

// NOLINTBEGIN


#ifndef LOG_SURGEON_GENERATED_BINDINGS_HPP
#define LOG_SURGEON_GENERATED_BINDINGS_HPP

#include <cstddef>
#include <cstdint>
#include "rust_compat.hpp"
namespace log_surgeon {
// https://github.com/mozilla/cbindgen/issues/43
struct Match;
}


namespace log_surgeon {

struct Interpretation;

struct LogEvent;

/// Newtype wrapper around a `usize` index.
struct NfaIdx;

struct Parser;

/// A `ParsingSpec` is conceptually a list of rules and a set of delimiter characters.
///
/// [`Rule`]s may be added with a specific integer priority;
/// larger integer value means higher priority.
/// Within a priority level, rules are prioritized by insertion order.
///
struct ParsingSpec;

struct ParsingSpecBuilder;

struct SearchResult;

struct InternalSubQuery;

template<typename T = void>
struct Vec;

/// Index in the parsing spec, offset by/starting at 1.
using RuleIdx = uint16_t;

/// Can't use `std::range::Range` because it's not `#[repr(C)]`.
template<typename Idx>
struct CRange {
    Idx start;
    Idx end;
};

/// A pointer-length pair with unchecked/untied lifetime.
template<typename T>
struct UncheckedCArray {
    const T *pointer;
    size_t length;
    // Custom
    [[nodiscard]] auto as_cpp_view() const noexcept -> std::string_view
    requires std::is_same_v<T, char>
    {
        return {this->pointer, this->length};
    }
};

struct MatchFfiPointers {
    const Match *parent;
    UncheckedCArray<char> lexeme;
    UncheckedCArray<char> root_rule_name;
    /// Name of _this_ (root or sub-) rule.
    UncheckedCArray<char> rule_name;
    /// Fully-qualified name, including the root rule and all nested regex capture expressions.
    UncheckedCArray<char> fully_qualified_name;
};

/// `Match`es are exposed to FFI, so they need to be `#[repr(C)]`.
struct Match {
    RuleIdx rule_idx;
    /// SubRule ID, local to the containing rule/variable/regex pattern;
    /// `None`/`0` for a root rule,
    /// See [`SubRule`](crate::parsing_spec::SubRule).
    uint16_t sub_rule_id;
    /// Parent SubRule ID, if any;
    /// `None` for both a root rule and a top-level capture in a regex pattern.
    uint16_t parent_id;
    /// Index of the parent in the full list of matches (including variables/root rules).
    /// For a variable, the parent index equals its own index.
    size_t parent_index;
    /// Relative to the start of the log message.
    CRange<size_t> range;
    bool is_leaf;
    uint16_t encoding_idx;
    /// DANGEROUS fields for FFI.
    /// But it's not dangerous if you don't look at it.
    MatchFfiPointers ffi_pointers;
};




extern "C" {

void log_surgeon_enable_tracing();

const Match *log_surgeon_log_event_all_matches(const LogEvent *log_event, size_t *len);

Box<LogEvent> log_surgeon_log_event_clone(const LogEvent *value);

void log_surgeon_log_event_drop(Box<LogEvent> value);

const size_t *log_surgeon_log_event_leaf_match_indices(const LogEvent *log_event, size_t *len);

Box<LogEvent> log_surgeon_log_event_new();

Box<Parser> log_surgeon_parser_clone(const Parser *value);

void log_surgeon_parser_drop(Box<Parser> value);

Box<Parser> log_surgeon_parser_new(Box<ParsingSpec> parsing_spec);

bool log_surgeon_parser_next(Parser *parser, CCharArray input, size_t *pos, LogEvent *out);

bool log_surgeon_parsing_spec_add_encoding(ParsingSpecBuilder *builder,
                                           CCharArray name,
                                           CCharArray pattern);

bool log_surgeon_parsing_spec_builder_add_rule_with_priority(ParsingSpecBuilder *builder,
                                                             int32_t priority,
                                                             CCharArray name,
                                                             CCharArray pattern);

Box<ParsingSpec> log_surgeon_parsing_spec_builder_build(Box<ParsingSpecBuilder> builder);

Option<Box<ParsingSpecBuilder>> log_surgeon_parsing_spec_builder_from_definition(CCharArray definition);

Box<ParsingSpecBuilder> log_surgeon_parsing_spec_builder_new();

void log_surgeon_parsing_spec_builder_set_delimiters(ParsingSpecBuilder *builder,
                                                     CCharArray delimiters);

Option<Box<ParsingSpec>> log_surgeon_parsing_spec_from_definition(CCharArray definition);

CCharArray log_surgeon_parsing_spec_get_encoding(const Parser *parser,
                                                 size_t encoding_idx,
                                                 size_t i);

const Interpretation *log_surgeon_search_get_interpretation(const Vec<Interpretation> *interpretations,
                                                            size_t i);

const InternalSubQuery *log_surgeon_search_get_sub_query(const Interpretation *interpretation,
                                                         size_t i);

void log_surgeon_search_interpretations_drop(Box<Vec<Interpretation>> value);

Box<Vec<Interpretation>> log_surgeon_search_query_interpretations(const Parser *parser,
                                                                  CCharArray input,
                                                                  CCharArray name);

void log_surgeon_search_result_drop(Box<SearchResult> value);

const Match *log_surgeon_search_result_get_leaf_matches(const SearchResult *search_result,
                                                        size_t *len);

CCharArray log_surgeon_search_sub_query_get_qualified_name(const InternalSubQuery *sub_query);

CCharArray log_surgeon_search_sub_query_get_value(const InternalSubQuery *sub_query);

}  // extern "C"

}  // namespace log_surgeon

#endif  // LOG_SURGEON_GENERATED_BINDINGS_HPP

// NOLINTEND
