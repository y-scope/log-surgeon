#ifndef LOG_SURGEON_LOG_SURGEON_HPP
#define LOG_SURGEON_LOG_SURGEON_HPP

// IWYU pragma: begin_exports
#include "log_surgeon/generated_bindings.hpp"
#include "log_surgeon/rust_compat.hpp"
// IWYU pragma: end_exports

#include <algorithm>
#include <cassert>
#include <cstddef>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace log_surgeon {
using imp::CRange;
using imp::Interpretation;
using imp::Match;
using imp::RuleIdx;
using imp::UncheckedCArray;

class ParsingSpecBuilder;
class ParsingSpec;
class Parser;
class LogEvent;
struct LeafQuery;

// Copy-and-swap: the by-value `operator=` is both copy and move assignment.
// NOLINTNEXTLINE(cppcoreguidelines-special-member-functions)
class ParsingSpecBuilder {
public:
    ParsingSpecBuilder();
    /**
     * Create and initialize a parsing specification builder from a serialized parsing
     * specification.
     *
     * See <https://github.com/y-scope/log-surgeon/blob/log-mechanic/rust/docs/parsing-spec-file.md>
     * for more details.
     * TODO: update link after PR merge.
     *
     * @param definition Definition as a string (file contents, not file path).
     */
    ParsingSpecBuilder(std::string_view definition);

    ~ParsingSpecBuilder();
    /**
     * Copying a moved-from or built (null) builder results in a null builder.
     */
    ParsingSpecBuilder(ParsingSpecBuilder const& other);
    /**
     * Afterwards, `other` is in a well-defined but invalid (null) state, as after `build()`.
     */
    ParsingSpecBuilder(ParsingSpecBuilder&& other) noexcept;
    /**
     * Copy-and-swap: the single assignment operator for both copy and move assignment.
     * `other` is taken by value (copy or move constructed at the call site),
     * so a separate `ParsingSpecBuilder&&` overload would be ambiguous.
     */
    auto operator=(ParsingSpecBuilder other) noexcept -> ParsingSpecBuilder&;

    /**
     * Conventional `swap` function, declared using `friend` for ADL.
     * Used to implement the copy-and-swap idiom.
     *
     * @param first
     * @param second
     */
    friend auto swap(ParsingSpecBuilder& first, ParsingSpecBuilder& second) noexcept -> void;

    /**
     * Construct the parsing specification from the parsing specification builder.
     * Afterwards, this builder is in a well-defined but invalid state.
     */
    auto build() -> ParsingSpec;

    auto set_delimiters(std::string_view delimiters) -> void;

    auto
    add_rule_with_priority(std::string_view name, std::string_view pattern, int32_t priority = 0)
            -> bool;

    auto add_placeholder(std::string_view name, std::string_view pattern) -> bool;

    auto add_encoding(std::string_view name, std::string_view pattern) -> bool;

private:
    imp::ParsingSpecBuilder* m_builder{};
};

/**
 * A reference-counted, immutable parsing specification.
 *
 * This is the main entry point: a spec can create any number of independent `Parser`s, and
 * searches (`search_by_name`, `search_by_log_shapes`) are performed directly on the spec. The
 * underlying Rust `ParsingSpec` is shared via `Arc`; copies of this handle share it.
 */
// Copy-and-swap: the by-value `operator=` is both copy and move assignment.
// NOLINTNEXTLINE(cppcoreguidelines-special-member-functions)
class ParsingSpec {
public:
    ~ParsingSpec();
    /**
     * Copying a moved-from (null) spec results in a null spec.
     */
    ParsingSpec(ParsingSpec const& other);
    /**
     * Afterwards, `other` is in a well-defined but invalid (null) state.
     */
    ParsingSpec(ParsingSpec&& other) noexcept;
    /**
     * Copy-and-swap: the single assignment operator for both copy and move assignment.
     * `other` is taken by value (copy or move constructed at the call site),
     * so a separate `ParsingSpec&&` overload would be ambiguous.
     */
    auto operator=(ParsingSpec other) noexcept -> ParsingSpec&;

    /**
     * Conventional `swap` function, declared using `friend` for ADL.
     * Used to implement the copy-and-swap idiom.
     *
     * @param first
     * @param second
     */
    friend auto swap(ParsingSpec& first, ParsingSpec& second) noexcept -> void;

    /**
     * Create a new, independently mutable `Parser` sharing this spec.
     */
    [[nodiscard]] auto create_parser() const -> Parser;

    /**
     * Computes interpretations for a (non-empty) named query.
     *
     * @param query
     * @param name
     */
    [[nodiscard]] auto search_by_name(std::string_view query, std::string_view name) const
            -> std::vector<std::vector<LeafQuery>>;

    /**
     * Computes interpretations for full-log search.
     * The return value has 3 layers of vectors:
     *
     * ```
     * std::vector< // One-to-one with `log_shapes`.
     *   std::vector< // List of interpretations corresponding to one log shape.
     *     std::vector<LeafQuery> // Each of these is 1 interpretation.
     *   >
     * >
     * ```
     *
     * @param query
     * @param log_shapes
     */
    [[nodiscard]] auto
    search_by_log_shapes(std::string_view query, std::span<CCharArray const> log_shapes) const
            -> std::vector<std::vector<std::vector<LeafQuery>>>;

    /**
     * Get the delimiters for this parsing spec.
     */
    [[nodiscard]] auto get_delimiters() const -> std::string_view;

private:
    friend class ParsingSpecBuilder;

    Arc<imp::ParsingSpec>* m_spec{};

    /**
     * Takes ownership of the given shared spec handle.
     */
    explicit ParsingSpec(Arc<imp::ParsingSpec>* spec) noexcept;

    /**
     * Conversion of FFI compatible Rust type to native C++ type.
     */
    [[nodiscard]] static auto convert_interpretations(
            Vec<Interpretation> const* rust_interpretation
    ) -> std::vector<std::vector<LeafQuery>>;

    /**
     * Conversion of FFI compatible Rust type to native C++ type.
     */
    [[nodiscard]] static auto convert_interpretation(Interpretation const* interpretation)
            -> std::vector<LeafQuery>;
};

// Copy-and-swap: the by-value `operator=` is both copy and move assignment.
// NOLINTNEXTLINE(cppcoreguidelines-special-member-functions)
class Parser {
public:
    ~Parser();
    /**
     * Copying a moved-from (null) parser results in a null parser.
     */
    Parser(Parser const& other);
    /**
     * Afterwards, `other` is in a well-defined but invalid (null) state.
     */
    Parser(Parser&& other) noexcept;
    /**
     * Copy-and-swap: the single assignment operator for both copy and move assignment.
     * `other` is taken by value (copy or move constructed at the call site),
     * so a separate `Parser&&` overload would be ambiguous.
     */
    auto operator=(Parser other) noexcept -> Parser&;

    /**
     * Conventional `swap` function, declared using `friend` for ADL.
     * Used to implement the copy-and-swap idiom.
     *
     * @param first
     * @param second
     */
    friend auto swap(Parser& first, Parser& second) noexcept -> void;

    /**
     * Get the next log event, as a handle.
     * Conceptually, `pos` is the current position/offset to parse the next event;
     * specifically, it's passed as a pointer so log surgeon advances it before returning.
     *
     * The data of the current log event is invalidated after the next call to `next_event()`.
     *
     * @param input A view of the entire input text.
     * @param pos Pointer to an offset value in the text.
     * @return `std::nullopt` iff EOF.
     */
    [[nodiscard]] auto next_event(std::string_view input, size_t* pos) -> std::optional<LogEvent>;

    /**
     * Reset the internal state of the parser;
     * e.g. when refilling a partial buffer and re-parsing the last log event.
     */
    auto reset();

private:
    friend class ParsingSpec;

    imp::Parser* m_parser{};
    imp::LogEvent* m_event{};

    /**
     * Creates a parser (handle) for the given parsing spec.
     *
     * @param spec The spec to parse with; this parser shares ownership of it.
     */
    explicit Parser(Arc<imp::ParsingSpec> const* spec);
};

class LogEvent {
public:
    /**
     * @param event A borrowed `imp::LogEvent const*` (doesn't take ownership).
     */
    LogEvent(imp::LogEvent const* event);

    [[nodiscard]] auto get_all_matches() const -> std::span<Match const> { return m_matches; }

    /**
     * Used to iterate over leaf matches of a log event;
     * done when this function returns `std::nullopt`.
     *
     * @param i Try to get the `i`th match.
     * @return `std::nullopt` iff out of range.
     */
    [[nodiscard]] auto get_leaf_match(size_t i) const -> std::optional<Match>;

    [[nodiscard]] auto get_message() const -> std::string_view;

private:
    imp::LogEvent const* m_event;
    std::span<Match const> m_matches;
    std::span<size_t const> m_leaf_indices;
};

struct LeafQuery {
    std::string name;
    std::string value;
};

inline ParsingSpecBuilder::ParsingSpecBuilder()
        : m_builder{imp::log_surgeon_parsing_spec_builder_new()} {}

inline ParsingSpecBuilder::ParsingSpecBuilder(std::string_view definition) {
    imp::ParsingSpecBuilder* builder{imp::log_surgeon_parsing_spec_builder_from_definition(
            CCharArray::from_string_view(definition)
    )};
    if (nullptr == builder) {
        throw std::invalid_argument("parsing specification definition invalid");
    }
    m_builder = builder;
}

inline ParsingSpecBuilder::~ParsingSpecBuilder() {
    if (nullptr != m_builder) {
        imp::log_surgeon_parsing_spec_builder_drop(m_builder);
    }
}

inline ParsingSpecBuilder::ParsingSpecBuilder(ParsingSpecBuilder const& other)
        // Copy-and-swap idiom: The resource management semantics are implemented here.
        // Don't delegate to the default constructor, which allocates a new builder.
        : m_builder{
                  nullptr == other.m_builder
                          ? nullptr
                          : imp::log_surgeon_parsing_spec_builder_clone(other.m_builder)
          } {}

inline ParsingSpecBuilder::ParsingSpecBuilder(ParsingSpecBuilder&& other) noexcept
        : m_builder{std::exchange(other.m_builder, nullptr)} {}

inline auto ParsingSpecBuilder::operator=(ParsingSpecBuilder other) noexcept
        -> ParsingSpecBuilder& {
    // Copy-and-swap idiom: `other` is copy or move constructed at the call site,
    // so this handles both copy and move assignment (and self-assignment).
    swap(*this, other);
    return *this;
}

inline auto swap(ParsingSpecBuilder& first, ParsingSpecBuilder& second) noexcept -> void {
    using std::swap;

    swap(first.m_builder, second.m_builder);
}

inline auto ParsingSpecBuilder::build() -> ParsingSpec {
    if (nullptr == m_builder) {
        throw std::invalid_argument("builder already constructed");
    }
    ParsingSpec spec{imp::log_surgeon_parsing_spec_builder_build(m_builder)};
    m_builder = nullptr;
    return spec;
}

inline auto ParsingSpecBuilder::set_delimiters(std::string_view delimiters) -> void {
    if (nullptr == m_builder) {
        throw std::invalid_argument("builder already constructed");
    }
    imp::log_surgeon_parsing_spec_builder_set_delimiters(
            m_builder,
            CCharArray::from_string_view(delimiters)
    );
}

inline auto ParsingSpecBuilder::add_rule_with_priority(
        std::string_view name,
        std::string_view pattern,
        int32_t priority
) -> bool {
    if (nullptr == m_builder) {
        throw std::invalid_argument("builder already constructed");
    }
    return imp::log_surgeon_parsing_spec_builder_add_rule_with_priority(
            m_builder,
            priority,
            CCharArray::from_string_view(name),
            CCharArray::from_string_view(pattern)
    );
}

inline auto ParsingSpecBuilder::add_placeholder(std::string_view name, std::string_view pattern)
        -> bool {
    if (nullptr == m_builder) {
        throw std::invalid_argument("builder already constructed");
    }
    return imp::log_surgeon_parsing_spec_builder_add_placeholder(
            m_builder,
            CCharArray::from_string_view(name),
            CCharArray::from_string_view(pattern)
    );
}

inline auto ParsingSpecBuilder::add_encoding(std::string_view name, std::string_view pattern)
        -> bool {
    if (nullptr == m_builder) {
        throw std::invalid_argument("builder already constructed");
    }
    return imp::log_surgeon_parsing_spec_builder_add_encoding(
            m_builder,
            CCharArray::from_string_view(name),
            CCharArray::from_string_view(pattern)
    );
}

inline ParsingSpec::ParsingSpec(Arc<imp::ParsingSpec>* spec) noexcept : m_spec(spec) {}

inline ParsingSpec::~ParsingSpec() {
    if (nullptr != m_spec) {
        imp::log_surgeon_parsing_spec_drop(m_spec);
    }
}

inline ParsingSpec::ParsingSpec(ParsingSpec const& other)
        // Copy-and-swap idiom: The resource management semantics are implemented here.
        : m_spec{nullptr == other.m_spec ? nullptr
                                         : imp::log_surgeon_parsing_spec_clone(other.m_spec)} {}

inline ParsingSpec::ParsingSpec(ParsingSpec&& other) noexcept
        : m_spec{std::exchange(other.m_spec, nullptr)} {}

inline auto ParsingSpec::operator=(ParsingSpec other) noexcept -> ParsingSpec& {
    // Copy-and-swap idiom: `other` is copy or move constructed at the call site,
    // so this handles both copy and move assignment (and self-assignment).
    swap(*this, other);
    return *this;
}

inline auto swap(ParsingSpec& first, ParsingSpec& second) noexcept -> void {
    using std::swap;

    swap(first.m_spec, second.m_spec);
}

inline auto ParsingSpec::create_parser() const -> Parser {
    return Parser{this->m_spec};
}

inline auto ParsingSpec::search_by_name(std::string_view query, std::string_view name) const
        -> std::vector<std::vector<LeafQuery>> {
    Box<Vec<Interpretation>> rust_interpretations{imp::log_surgeon_search_by_name(
            m_spec,
            CCharArray::from_string_view(query),
            CCharArray::from_string_view(name)
    )};

    std::vector<std::vector<LeafQuery>> interpretations{
            ParsingSpec::convert_interpretations(rust_interpretations)
    };

    imp::log_surgeon_search_interpretations_by_name_drop(rust_interpretations);

    return interpretations;
}

inline auto ParsingSpec::search_by_log_shapes(
        std::string_view query,
        std::span<CCharArray const> log_shapes
) const -> std::vector<std::vector<std::vector<LeafQuery>>> {
    std::vector<std::vector<std::vector<LeafQuery>>> interpretations_by_shapes;
    interpretations_by_shapes.reserve(log_shapes.size());

    Box<Vec<Vec<Interpretation>>> rust_interpretations{imp::log_surgeon_search_by_log_shapes(
            m_spec,
            CCharArray::from_string_view(query),
            CArray<CCharArray>::from_span(log_shapes)
    )};

    size_t i{0};
    while (true) {
        Vec<Interpretation> const* interpretations{
                imp::log_surgeon_search_get_interpretations_for_shape(rust_interpretations, i)
        };
        if (nullptr == interpretations) {
            break;
        }

        interpretations_by_shapes.push_back(ParsingSpec::convert_interpretations(interpretations));

        i++;
    }

    imp::log_surgeon_search_interpretations_by_log_shapes_drop(rust_interpretations);

    return interpretations_by_shapes;
}

inline auto ParsingSpec::convert_interpretations(Vec<Interpretation> const* rust_interpretations)
        -> std::vector<std::vector<LeafQuery>> {
    std::vector<std::vector<LeafQuery>> interpretations;
    size_t i{0};
    while (true) {
        Interpretation const* interpretation{
                imp::log_surgeon_search_get_interpretation(rust_interpretations, i)
        };
        if (nullptr == interpretation) {
            break;
        }

        interpretations.push_back(ParsingSpec::convert_interpretation(interpretation));

        i++;
    }
    return interpretations;
}

inline auto ParsingSpec::convert_interpretation(Interpretation const* interpretation)
        -> std::vector<LeafQuery> {
    std::vector<LeafQuery> leaf_queries;
    size_t i{0};
    while (true) {
        imp::LeafQuery const* query{log_surgeon_search_get_leaf_query(interpretation, i)};
        if (nullptr == query) {
            break;
        }

        std::string_view const name{imp::log_surgeon_search_leaf_query_get_name(query)};
        std::string_view const value{imp::log_surgeon_search_leaf_query_get_value(query)};

        leaf_queries.push_back({
                .name = std::string{name},
                .value = std::string{value},
        });

        i++;
    }
    return leaf_queries;
}

inline auto ParsingSpec::get_delimiters() const -> std::string_view {
    return imp::log_surgeon_parsing_spec_get_delimiters(m_spec);
}

inline Parser::Parser(Arc<imp::ParsingSpec> const* spec) {
    if (nullptr == spec) {
        throw std::invalid_argument("spec must not be null");
    }
    m_parser = imp::log_surgeon_parsing_spec_create_parser(spec);
    m_event = imp::log_surgeon_log_event_new();
}

inline Parser::~Parser() {
    if (nullptr != m_event) {
        imp::log_surgeon_log_event_drop(m_event);
    }
    if (nullptr != m_parser) {
        imp::log_surgeon_parser_drop(m_parser);
    }
}

inline Parser::Parser(Parser const& other)
        // Copy-and-swap idiom: The resource management semantics are implemented here.
        : m_parser{
                  nullptr == other.m_parser ? nullptr
                                            : imp::log_surgeon_parser_clone(other.m_parser)
          },
          m_event{nullptr == other.m_event ? nullptr
                                           : imp::log_surgeon_log_event_clone(other.m_event)} {}

inline Parser::Parser(Parser&& other) noexcept
        : m_parser{std::exchange(other.m_parser, nullptr)},
          m_event{std::exchange(other.m_event, nullptr)} {}

inline auto Parser::operator=(Parser other) noexcept -> Parser& {
    // Copy-and-swap idiom: `other` is copy or move constructed at the call site,
    // so this handles both copy and move assignment (and self-assignment).
    swap(*this, other);
    return *this;
}

inline auto swap(Parser& first, Parser& second) noexcept -> void {
    using std::swap;

    swap(first.m_parser, second.m_parser);
    swap(first.m_event, second.m_event);
}

inline auto Parser::next_event(std::string_view input, size_t* pos) -> std::optional<LogEvent> {
    if (!log_surgeon_parser_next(m_parser, CCharArray::from_string_view(input), pos, m_event)) {
        return std::nullopt;
    }
    return std::make_optional(LogEvent{m_event});
}

inline auto Parser::reset() {
    imp::log_surgeon_parser_reset(m_parser);
}

inline LogEvent::LogEvent(imp::LogEvent const* event) : m_event(event) {
    m_matches = event->all_matches.as_span();
    m_leaf_indices = event->leaf_indices.as_span();
}

inline auto LogEvent::get_leaf_match(size_t i) const -> std::optional<Match> {
    if (i < m_leaf_indices.size()) {
        // `std::span` doesn't have `.at()` until C++26...
        // NOLINTNEXTLINE(cppcoreguidelines-pro-bounds-avoid-unchecked-container-access)
        return std::make_optional(m_matches[m_leaf_indices[i]]);
    }
    return std::nullopt;
}

inline auto LogEvent::get_message() const -> std::string_view {
    return m_event->message;
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LOG_SURGEON_HPP
