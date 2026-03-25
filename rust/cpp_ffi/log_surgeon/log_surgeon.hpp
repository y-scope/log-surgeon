#ifndef LOG_SURGEON_LOG_SURGEON_HPP
#define LOG_SURGEON_LOG_SURGEON_HPP

#include "log_surgeon/generated_bindings.hpp"

#include <algorithm>
#include <cassert>
#include <cstddef>
#include <optional>
#include <string_view>
#include <utility>

namespace log_surgeon {
class ParserHandle;
class EventHandle;

class ParserHandle {
public:
    /**
     * Creates a parser (handle) for the given schema.
     *
     * @param schema A borrowed `Schema const*` (doesn't take ownership).
     */
    ParserHandle(Schema* schema) : ParserHandle{} {
        m_parser = log_surgeon_parser_new(schema);
        m_event = log_surgeon_log_event_new();
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

private:
    /**
     * Last piece of copy-and-swap;
     * private since we only want this for copy-and-swap.
     */
    ParserHandle() noexcept = default;

    Parser* m_parser;
    LogEvent* m_event;
};

class EventHandle {
public:
    /**
     * @param event A borrowed `Event const*` (doesn't take ownership).
     */
    EventHandle(LogEvent const* event, Parser const* parser) : m_event(event), m_parser(parser) {}

    [[nodiscard]] auto log_type() const -> std::string_view {
        return log_surgeon_log_event_log_type(m_event);
    }

    /**
     * Used to iterate over leaf captures of a log event;
     * done when this function returns `std::nullopt`.
     *
     * @param i Try to get the `i`th capture.
     * @return `std::nullopt` iff out of range.
     */
    [[nodiscard]] auto get_leaf_capture(size_t i) const -> std::optional<CCapture>;

    /**
     * Used to iterate over all captures of a log event (including the variable itself);
     * done when this function returns `std::nullopt`.
     *
     * @param i Try to get the `i`th capture.
     * @return `std::nullopt` iff out of range.
     */
    [[nodiscard]] auto get_capture(size_t i) const -> std::optional<CCapture>;

private:
    LogEvent const* m_event;
    Parser const* m_parser;
};

inline auto ParserHandle::next_event(std::string_view input, size_t* pos)
        -> std::optional<EventHandle> {
    if (!log_surgeon_parser_next(m_parser, CCharArray::from_string_view(input), pos, m_event)) {
        return std::nullopt;
    }
    return std::make_optional(EventHandle{m_event, m_parser});
}

inline auto EventHandle::get_leaf_capture(size_t i) const -> std::optional<CCapture> {
    CCapture const capture{log_surgeon_log_event_get_leaf_capture(m_event, i, m_parser)};
    if (nullptr != capture.lexeme.pointer) {
        return std::make_optional(capture);
    }
    return std::nullopt;
}

inline auto EventHandle::get_capture(size_t i) const -> std::optional<CCapture> {
    CCapture const capture{log_surgeon_log_event_get_capture(m_event, i, m_parser)};
    if (nullptr != capture.lexeme.pointer) {
        return std::make_optional(capture);
    }
    return std::nullopt;
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LOG_SURGEON_HPP
