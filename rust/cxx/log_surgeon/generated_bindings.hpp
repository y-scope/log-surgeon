// C++ declarations for C-ABI functions implemented in Rust.

// NOLINTBEGIN

#ifndef LOG_SURGEON_GENERATED_BINDINGS_HPP
    #define LOG_SURGEON_GENERATED_BINDINGS_HPP

    #include "rust_compat.hpp"

    #include <cstddef>
    #include <cstdint>

namespace log_surgeon::imp {
// https://github.com/mozilla/cbindgen/issues/43
struct Match;
}  // namespace log_surgeon::imp

namespace log_surgeon::imp {
struct DfsIndex;

struct Interpretation;

struct LeafQuery;

/// Newtype wrapper around a `usize` index.
struct NfaIdx;

/// A parser is almost stateless aside from:
///
/// - An owned copy of the substring of the input text
///   for the "current" (most recently returned) log event.
///   - This greatly simplifies lifetime management, especially in the presence of FFI,
///     since the parser doesn't own a whole copy of the input text.
///   - Most calls to [`Parser::next_event`] won't require allocation;
///     the memory is bounded by the max log event size.
/// - Whether a separating `header` has been seen (header mode vs newline mode).
/// - The lexer's pending match, i.e. the header of the next log event.
/// - (Optional/implementation detail) [`TdfaExecution`] cache.
///
struct Parser;

/// A `ParsingSpec` is conceptually a list of rules and a set of delimiter characters.
///
/// [`Rule`]s may be added with a specific integer priority;
/// larger integer value means higher priority.
/// Within a priority level, rules are prioritized by insertion order.
///
struct ParsingSpec;

struct ParsingSpecBuilder;

/// Index in the parsing specification, offset by/starting at 1.
/// `Option<RuleIdx>` is ABI equivalent to `u16` (for FFI);
/// `None`/`0` represents static text fragments.
using RuleIdx = uint16_t;

/// Can't use `std::range::Range` because it's not `#[repr(C)]`.
template <typename Idx>
struct CRange {
    Idx start;
    Idx end;
};

/// A pointer-length pair with unchecked/untied lifetime.
template <typename T>
struct UncheckedCArray {
    T const* pointer;
    size_t length;

    // Custom
    [[nodiscard]] auto as_cpp_view() const noexcept -> std::string_view
    requires std::is_same_v<T, char>
    {
        return {this->pointer, this->length};
    }
};

struct MatchFfiPointers {
    Match const* parent;
    UncheckedCArray<char> lexeme;
    /// Name of _this_ (root or sub-) rule.
    UncheckedCArray<char> rule_name;
    /// Fully-qualified name, including the root rule and all nested regex capture expressions.
    UncheckedCArray<char> fully_qualified_name;
};

/// `Match`es are exposed to FFI, so they need to be `#[repr(C)]`.
struct Match {
    RuleIdx rule_idx;
    /// Capture ID, local to the containing rule/variable/regex pattern;
    /// `None`/`0` for a root rule,
    /// See [`RuleInfo`](crate::parsing_spec::RuleInfo).
    uint16_t sub_rule_id;
    /// Index of the parent in the full list of matches (including variables/root rules).
    /// For a variable, the parent index equals its own index.
    size_t parent_index;
    /// Relative to the start of the log message.
    CRange<size_t> range;
    bool is_leaf;
    /// This should be `Option<EncodingIdx>`,
    /// but cbindgen doesn't translate it to `uint16_t` properly.
    ///
    /// See:
    /// - https://github.com/mozilla/cbindgen/issues/690
    /// - https://github.com/mozilla/cbindgen/issues/326
    /// - https://github.com/mozilla/cbindgen/pull/1029
    uint16_t encoding_idx;
    /// DANGEROUS fields exposed for FFI.
    /// But it's not dangerous if you don't look at it (in Rust).
    /// Safe Rust code should refer to the fields above and the corresponding [`LogEvent`]
    /// as necessary.
    ///
    /// Note: [`LogEvent`] can borrow from [`crate::parser::Parser`] since it's an "external" value,
    /// but the [`Match`]es of a `LogEvent` live in a `Vec` inside `Parser`,
    /// so they can't safely reference the `Parser` (be self-referential).
    MatchFfiPointers ffi_pointers;

    // Custom
    [[nodiscard]] auto is_root() const noexcept -> bool { return this->sub_rule_id == 0; }

    [[nodiscard]] auto get_parent() const noexcept -> Match const* {
        return this->ffi_pointers.parent;
    }

    [[nodiscard]] auto get_lexeme() const noexcept -> std::string_view {
        return this->ffi_pointers.lexeme.as_cpp_view();
    }

    [[nodiscard]] auto get_rule_name() const noexcept -> std::string_view {
        return this->ffi_pointers.rule_name.as_cpp_view();
    }

    [[nodiscard]] auto get_fully_qualified_name() const noexcept -> std::string_view {
        return this->ffi_pointers.fully_qualified_name.as_cpp_view();
    }
};

struct LogEvent {
    /// Strictly speaking, this field is redundant;
    /// however, the spec is needed to get info about the rules,
    /// and is included here for convenience.
    ///
    /// In particular, since [`Parser::next_event`](crate::parser::Parser::next_event)
    /// returns a `LogEvent` that `mut` (exclusively) borrows from the parser
    /// (see [`Parser`](crate::parser::Parser) for more details),
    /// the caller can't access the parser's spec and the event at the same time.
    /// So, `Parser::next_event` passes a reference to the spec through the returned `LogEvent`.
    ParsingSpec const* spec;
    CCharArray message;
    /// Matches are sorted:
    ///
    /// 1. left-to-right (lexicographically with respect to the input),
    /// 2. top-down; parent rules first (lexicographically with respect to the regex pattern).
    ///
    /// In particular, root rule matches always come before their sub-rule matches.
    CArray<Match> all_matches;
    CArray<size_t> root_indices;
    CArray<size_t> leaf_indices;
};

extern "C" {

    /// Enable tracing debugging logs; see [`README.md#Debugging`].
    void log_surgeon_enable_tracing();

    /// Get the matches of a log event.
    Match const* log_surgeon_log_event_all_matches(LogEvent const* log_event, size_t* len);

    Box<LogEvent> log_surgeon_log_event_clone(LogEvent const* value);

    void log_surgeon_log_event_drop(Box<LogEvent> value);

    /// Get the match indices of a log event.
    size_t const* log_surgeon_log_event_leaf_match_indices(LogEvent const* log_event, size_t* len);

    /// Create a (boxed) [`LogEvent`], for cached/reused return value for `log_surgeon_parser_next`.
    Box<LogEvent> log_surgeon_log_event_new();

    Box<Parser> log_surgeon_parser_clone(Parser const* value);

    void log_surgeon_parser_drop(Box<Parser> value);

    /// See [`Parser::next_event`].
    bool log_surgeon_parser_next(Parser* parser, CCharArray input, size_t* pos, LogEvent* out);

    /// See [`Parser::reset`].
    void log_surgeon_parser_reset(Parser* parser);

    /// See [`ParsingSpecBuilder::add_encoding`].
    bool log_surgeon_parsing_spec_builder_add_encoding(
            ParsingSpecBuilder* builder,
            CCharArray name,
            CCharArray pattern
    );

    /// See [`ParsingSpecBuilder::add_placeholder`].
    bool log_surgeon_parsing_spec_builder_add_placeholder(
            ParsingSpecBuilder* builder,
            CCharArray name,
            CCharArray pattern
    );

    /// See [`ParsingSpecBuilder::add_rule_with_priority`].
    bool log_surgeon_parsing_spec_builder_add_rule_with_priority(
            ParsingSpecBuilder* builder,
            int32_t priority,
            CCharArray name,
            CCharArray pattern
    );

    /// Consume the (boxed) [`ParsingSpecBuilder`] to construct a shared [`ParsingSpec`].
    ///
    /// The spec is returned behind an [`Arc`] so it can outlive any single [`Parser`] and back
    /// several of them (see [`log_surgeon_parsing_spec_create_parser`]),
    /// and/or be searched directly.
    Box<Arc<ParsingSpec>> log_surgeon_parsing_spec_builder_build(Box<ParsingSpecBuilder> builder);

    Box<ParsingSpecBuilder> log_surgeon_parsing_spec_builder_clone(ParsingSpecBuilder const* value);

    void log_surgeon_parsing_spec_builder_drop(Box<ParsingSpecBuilder> value);

    /// See [`ParsingSpecBuilder::from_parsing_spec_definition`].
    Option<Box<ParsingSpecBuilder>> log_surgeon_parsing_spec_builder_from_definition(
            CCharArray definition
    );

    /// Create a new [`ParsingSpecBuilder`].
    Box<ParsingSpecBuilder> log_surgeon_parsing_spec_builder_new();

    /// See [`ParsingSpecBuilder::set_delimiters`].
    void log_surgeon_parsing_spec_builder_set_delimiters(
            ParsingSpecBuilder* builder,
            CCharArray delimiters
    );

    Box<Arc<ParsingSpec>> log_surgeon_parsing_spec_clone(Arc<ParsingSpec> const* value);

    /// Create an owned [`Parser`] sharing the given [`ParsingSpec`].
    Box<Parser> log_surgeon_parsing_spec_create_parser(Arc<ParsingSpec> const* spec);

    void log_surgeon_parsing_spec_drop(Box<Arc<ParsingSpec>> value);

    CCharArray log_surgeon_parsing_spec_get_delimiters(Arc<ParsingSpec> const* spec);

    Box<Vec<Vec<Interpretation>>> log_surgeon_search_by_log_shapes(
            Arc<ParsingSpec> const* spec,
            CCharArray input,
            CArray<CCharArray> log_shapes
    );

    Box<Vec<Interpretation>>
    log_surgeon_search_by_name(Arc<ParsingSpec> const* spec, CCharArray input, CCharArray name);

    Interpretation const*
    log_surgeon_search_get_interpretation(Vec<Interpretation> const* interpretations, size_t i);

    Vec<Interpretation> const* log_surgeon_search_get_interpretations_for_shape(
            Vec<Vec<Interpretation>> const* interpretations,
            size_t i
    );

    LeafQuery const*
    log_surgeon_search_get_leaf_query(Interpretation const* interpretation, size_t i);

    void log_surgeon_search_interpretations_by_log_shapes_drop(Box<Vec<Vec<Interpretation>>> value);

    void log_surgeon_search_interpretations_by_name_drop(Box<Vec<Interpretation>> value);

    CCharArray log_surgeon_search_leaf_query_get_name(LeafQuery const* leaf_query);

    CCharArray log_surgeon_search_leaf_query_get_value(LeafQuery const* leaf_query);

}  // extern "C"
}  // namespace log_surgeon::imp

#endif  // LOG_SURGEON_GENERATED_BINDINGS_HPP

// NOLINTEND
