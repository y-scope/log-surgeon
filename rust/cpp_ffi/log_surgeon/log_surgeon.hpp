#ifndef LOG_SURGEON_LOG_SURGEON_HPP
#define LOG_SURGEON_LOG_SURGEON_HPP

#include "log_surgeon/generated_bindings.hpp"

#include <algorithm>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>
#include <vector>

namespace log_surgeon {
class ParserHandle;
class EventHandle;
class Variable;

class ParserHandle {
public:
    /**
     * Creates a parser (handle) for the given schema.
     *
     * @param schema A borrowed `Schema const*` (doesn't take ownership).
     */
    ParserHandle(Schema const* schema) : ParserHandle{} {
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
    [[nodiscard]] auto next_event(std::string_view const& input, size_t* pos)
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
    EventHandle(LogEvent const* event) : m_event(event) {}

    [[nodiscard]] auto log_type() const -> std::string_view {
        return log_surgeon_log_event_log_type(m_event);
    }

    /**
     * Used to iterate over variables of a log event;
     * done when this function returns `std::nullopt`.
     *
     * @param i Try to get the `i`th variable.
     * @return `std::nullopt` iff out of range.
     */
    [[nodiscard]] auto get_variable(size_t i) const -> std::optional<Variable>;

private:
    LogEvent const* m_event;
};

class Variable {
public:
    using CaptureIterator = std::vector<uint32_t>::const_iterator;

    /**
     * @param event The log event this variable belongs to.
     * @param index This variable's index in its event.
     * @param variable The `CVariable` struct with the rule ID/index, name, and lexeme.
     */
    Variable(LogEvent const* event, size_t index, CVariable variable);

    [[nodiscard]] auto get_rule() const -> size_t { return m_rule; }

    [[nodiscard]] auto get_name() const -> std::string_view { return m_name; }

    [[nodiscard]] auto get_lexeme() const -> std::string_view { return m_lexeme; }

    [[nodiscard]] auto captures_begin() const -> CaptureIterator { return m_captures.begin(); }

    [[nodiscard]] auto captures_end() const -> CaptureIterator { return m_captures.end(); }

    [[nodiscard]] auto capture_by_id(uint32_t id) const -> std::vector<CCapture> const& {
        return m_captures_by_id.at(id);
    }

private:
    size_t m_rule;
    std::string_view m_name;
    std::string_view m_lexeme;

    LogEvent const* m_event;
    size_t m_index;
    std::vector<uint32_t> m_captures;
    std::vector<std::vector<CCapture>> m_captures_by_id;
};

inline auto ParserHandle::next_event(std::string_view const& input, size_t* pos)
        -> std::optional<EventHandle> {
    if (!log_surgeon_parser_next(m_parser, CCharArray::from_string_view(input), pos, m_event)) {
        return std::nullopt;
    }
    return std::make_optional(EventHandle{m_event});
}

inline auto EventHandle::get_variable(size_t i) const -> std::optional<Variable> {
    CVariable const variable{log_surgeon_log_event_variable(m_event, i)};
    if (nullptr != variable.name.pointer) {
        return std::make_optional(Variable{m_event, i, variable});
    }
    return std::nullopt;
}

inline Variable::Variable(LogEvent const* event, size_t index, CVariable variable)
        : m_rule(variable.rule),
          m_name(variable.name),
          m_lexeme(variable.lexeme),
          m_event(event),
          m_index(index) {
    for (size_t i{0};; ++i) {
        CCapture const capture{log_surgeon_log_event_capture(m_event, m_index, i)};
        if (0 == capture.id) {
            break;
        }

        if (m_captures.end() == std::find(m_captures.begin(), m_captures.end(), capture.id)) {
            m_captures.push_back(capture.id);

            assert(m_captures_by_id.size() < capture.id);
            m_captures_by_id.resize(capture.id + 1);
        }

        assert(m_captures_by_id.size() >= capture.id);
        m_captures_by_id.at(capture.id).push_back(capture);
    }
}
}  // namespace log_surgeon

#endif  // LOG_SURGEON_LOG_SURGEON_HPP
