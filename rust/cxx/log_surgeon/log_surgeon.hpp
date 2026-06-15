#ifndef LOG_SURGEON_LOG_SURGEON_HPP
#define LOG_SURGEON_LOG_SURGEON_HPP

#include "log_surgeon/generated_bindings.hpp"
#include "log_surgeon/rust_compat.hpp"

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
class ParserHandle;
class EventHandle;
struct SubQuery;

class ParserHandle {
public:
    /**
     * Creates a parser (handle) for the given parsing spec.
     *
     * @param spec An owned`ParsingSpec*` (takes ownership).
     */
    ParserHandle(ParsingSpec* spec) : ParserHandle{} {
        if (nullptr == spec) {
            throw std::invalid_argument("spec must not be null");
        }
        m_parser = log_surgeon_parser_new(spec);
        m_event = log_surgeon_log_event_new();

        m_encodings.emplace_back();
        while (true) {
            std::vector<std::string_view> encodings;
            while (true) {
                std::string_view const name{log_surgeon_parsing_spec_get_encoding(
                        m_parser,
                        m_encodings.size(),
                        encodings.size()
                )};

                if (name.empty()) {
                    break;
                }

                encodings.push_back(name);
            }

            if (encodings.empty()) {
                break;
            }

            m_encodings.push_back(std::move(encodings));
        }
    }

    ~ParserHandle() {
        if (nullptr != m_event) {
            log_surgeon_log_event_drop(m_event);
        }
        if (nullptr != m_parser) {
            log_surgeon_parser_drop(m_parser);
        }
    }

    ParserHandle(ParserHandle const& other) : ParserHandle{} {
        // Copy-and swap idiom: The first "centerpiece";
        // the "semantics" of this type's resource management must be
        // bona fide implemented here.
        m_parser = log_surgeon_parser_clone(other.m_parser);
        m_event = log_surgeon_log_event_clone(other.m_event);
    }

    ParserHandle(ParserHandle&& other) noexcept : ParserHandle{} {
        // Copy-and-swap idiom: The move constructor is handled by the same
        // `swap` mechanism used to safely implement copy assignment.
        swap(*this, other);
    }

    auto operator=(ParserHandle other) noexcept -> ParserHandle& {
        // Copy-and-swap idiom: It is important that `other` is taken by value.
        // This would handle both copy and move assignment;
        // when called with an rvalue reference,
        // the compiler would use the move constructor to create `other`,
        // which we then swap with.
        // Supposedly, that allows for better optimization opportunities too.
        swap(*this, other);
        return *this;
    }

    auto operator=(ParserHandle&& other) noexcept -> ParserHandle& {
        // Copy-and-swap idiom: Duplicate of copy assignment;
        // lints aren't smart enough to realize that this would be covered as above.
        swap(*this, other);
        return *this;
    }

    /**
     * Conventional `swap` function, declared using `friend` for ADL.
     * Also the second critical piece for the copy-and-swap idiom.
     *
     * @param first
     * @param second
     */
    friend void swap(ParserHandle& first, ParserHandle& second) noexcept {
        using std::swap;

        swap(first.m_parser, second.m_parser);
        swap(first.m_event, second.m_event);
    }

    /**
     * Get the next log event, as a handle.
     * Updates `pos` before returning.
     *
     * @param input A view of the entire input text.
     * @param pos A pointer to the current position in the text.
     * @return `std::nullopt` iff EOF.
     */
    [[nodiscard]] auto next_event(std::string_view input, size_t* pos)
            -> std::optional<EventHandle>;

    /**
     * Computes interpretations for a query.
     *
     * @param name
     * @param query
     */
    [[nodiscard]] auto query_interpretations(std::string_view name, std::string_view query)
            -> std::vector<std::vector<SubQuery>>;

    /**
     * `encoding_idx == 0` corresponds to the empty set (no possible encodings).
     */
    [[nodiscard]] auto get_encoding(size_t encoding_idx) const
            -> std::vector<std::string_view> const&;

private:
    /**
     * Last piece of copy-and-swap;
     * private since we only want this for copy-and-swap.
     */
    ParserHandle() noexcept = default;

    Parser* m_parser{};
    LogEvent* m_event{};

    std::vector<std::vector<std::string_view>> m_encodings;
};

class EventHandle {
public:
    /**
     * @param event A borrowed `Event const*` (doesn't take ownership).
     * @param parser A borrowed `Parser const*` (doesn't take ownership).
     */
    EventHandle(LogEvent const* event);

    [[nodiscard]] auto get_all_matches() const -> std::span<Match const> { return m_matches; }

    /**
     * Used to iterate over leaf matches of a log event;
     * done when this function returns `std::nullopt`.
     *
     * @param i Try to get the `i`th match.
     * @return `std::nullopt` iff out of range.
     */
    [[nodiscard]] auto get_leaf_match(size_t i) const -> std::optional<Match>;

private:
    std::span<Match const> m_matches;
    std::span<size_t const> m_leaf_indices;
};

struct SubQuery {
    std::string qualified_name;
    std::string value;
};

inline auto ParserHandle::next_event(std::string_view input, size_t* pos)
        -> std::optional<EventHandle> {
    if (!log_surgeon_parser_next(m_parser, CCharArray::from_string_view(input), pos, m_event)) {
        return std::nullopt;
    }
    return std::make_optional(EventHandle{m_event});
}

inline auto ParserHandle::query_interpretations(std::string_view name, std::string_view query)
        -> std::vector<std::vector<SubQuery>> {
    std::vector<std::vector<SubQuery>> interpretations;

    Box<Vec<Interpretation>> rust_interpretations{log_surgeon_search_query_interpretations(
            m_parser,
            CCharArray::from_string_view(query),
            CCharArray::from_string_view(name)
    )};

    size_t i{0};
    while (true) {
        Interpretation const* interpretation{
                log_surgeon_search_get_interpretation(rust_interpretations, i)
        };
        if (nullptr == interpretation) {
            break;
        }

        std::vector<SubQuery> sub_queries;
        size_t j{0};
        while (true) {
            InternalSubQuery const* sub_query{log_surgeon_search_get_sub_query(interpretation, j)};
            if (nullptr == sub_query) {
                break;
            }

            std::string_view const qualified_name{
                    log_surgeon_search_sub_query_get_qualified_name(sub_query)
            };
            std::string_view const value{log_surgeon_search_sub_query_get_value(sub_query)};

            sub_queries.push_back({
                    .qualified_name = std::string{qualified_name},
                    .value = std::string{value},
            });

            j++;
        }
        interpretations.push_back(std::move(sub_queries));

        i++;
    }

    log_surgeon_search_interpretations_drop(rust_interpretations);

    return interpretations;
}

inline auto ParserHandle::get_encoding(size_t encoding_idx) const
        -> std::vector<std::string_view> const& {
    return m_encodings.at(encoding_idx);
}

inline EventHandle::EventHandle(LogEvent const* event) {
    size_t len{0};
    Match const* matches{log_surgeon_log_event_all_matches(event, &len)};
    m_matches = {matches, len};
    size_t const* leaf_indices{log_surgeon_log_event_leaf_match_indices(event, &len)};
    m_leaf_indices = {leaf_indices, len};
}

inline auto EventHandle::get_leaf_match(size_t i) const -> std::optional<Match> {
    if (i < m_leaf_indices.size()) {
        // `std::span` doesn't have `.at()` until C++26...
        // NOLINTNEXTLINE(cppcoreguidelines-pro-bounds-avoid-unchecked-container-access)
        return std::make_optional(m_matches[m_leaf_indices[i]]);
    }
    return std::nullopt;
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LOG_SURGEON_HPP
