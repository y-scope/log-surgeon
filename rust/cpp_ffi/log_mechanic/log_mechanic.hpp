#ifndef LOGMECH_LOG_MECHANIC_HPP
#define LOGMECH_LOG_MECHANIC_HPP

#include <algorithm>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>
#include <vector>

#include "log_mechanic/generated_bindings.hpp"

namespace log_mechanic {
class ParserHandle;
class EventHandle;
class Variable;

class ParserHandle {
public:
    ParserHandle(Schema const* schema) : ParserHandle {} {
        this->m_parser = logmech_parser_new(schema);
        this->m_event = logmech_log_event_new();
    }

    ~ParserHandle() {
        if (nullptr != this->m_event) {
            logmech_log_event_drop(this->m_event);
        }
        if (nullptr != this->m_parser) {
            logmech_parser_drop(this->m_parser);
        }
    }

    ParserHandle(ParserHandle const& other) noexcept : ParserHandle{} {
        this->m_parser = logmech_parser_clone(other.m_parser);
        this->m_event = logmech_log_event_clone(other.m_event);
    }

    ParserHandle(ParserHandle&& other) noexcept : ParserHandle {} {
        swap(*this, other);
    }

    // Copy-and-swap idiom: it is important that `other` is taken by value.
    // This would handle both copy and move assignment,
    // but lints aren't smart enough to realize that.
    auto operator=(ParserHandle other) noexcept -> ParserHandle& {
        swap(*this, other);
        return *this;
    }

    auto operator=(ParserHandle&& other) noexcept -> ParserHandle& {
        swap(*this, other);
        return *this;
    }

    friend void swap(ParserHandle& first, ParserHandle& second) noexcept {
        using std::swap;

        swap(first.m_parser, second.m_parser);
        swap(first.m_event, second.m_event);
    }

    auto next_event(std::string_view const& input, size_t* pos) -> std::optional<EventHandle>;

private:
    ParserHandle() = default;

    Parser* m_parser;
    LogEvent* m_event;
};

class EventHandle {
public:
    EventHandle(LogEvent* event) : m_event(event) {}

    [[nodiscard]] auto log_type() const -> std::string_view {
        return logmech_log_event_log_type(this->m_event);
    }

    [[nodiscard]] auto get_variable(size_t i) const -> std::optional<Variable>;

private:
    LogEvent* m_event;
};

class Variable {
public:
    using CaptureIterator = std::vector<uint32_t>::const_iterator;

    Variable(LogEvent* event, size_t index, CVariable variable);

    [[nodiscard]] auto rule() const -> size_t {
        return this->m_rule;
    }

    [[nodiscard]] auto name() const -> std::string_view {
        return this->m_name;
    }

    [[nodiscard]] auto lexeme() const -> std::string_view {
        return this->m_lexeme;
    }

    [[nodiscard]] auto captures_begin() const -> CaptureIterator {
        return this->m_captures.begin();
    }

    [[nodiscard]] auto captures_end() const -> CaptureIterator {
        return this->m_captures.end();
    }

    [[nodiscard]] auto capture_by_id(uint32_t id) const -> std::vector<CCapture> const& {
        return this->m_captures_by_id.at(id);
    }

private:
    size_t m_rule;
    std::string_view m_name;
    std::string_view m_lexeme;

    LogEvent* m_event;
    size_t m_index;
    std::vector<uint32_t> m_captures;
    std::vector<std::vector<CCapture>> m_captures_by_id;
};

inline auto ParserHandle::next_event(std::string_view const& input, size_t* pos) -> std::optional<EventHandle> {
    if (!logmech_parser_next(this->m_parser, CArray<char>::from_string_view(input), pos, this->m_event)) {
        return std::nullopt;
    }
    return std::make_optional(EventHandle{this->m_event});
}

inline auto EventHandle::get_variable(size_t i) const -> std::optional<Variable> {
    CVariable const variable{logmech_log_event_variable(this->m_event, i)};
    if (variable.name.pointer != nullptr) {
        return std::make_optional(Variable{this->m_event, i, variable});
    }
    return std::nullopt;
}

inline Variable::Variable(LogEvent* event, size_t index, CVariable variable)
        : m_rule(variable.rule),
          m_name(variable.name),
          m_lexeme(variable.lexeme),
          m_event(event),
          m_index(index) {
    size_t i{0};
    while (true) {
        CCapture const capture{logmech_log_event_capture(this->m_event, this->m_index, i)};
        if (capture.id == 0) {
            break;
        }

        if (std::find(this->m_captures.begin(), this->m_captures.end(), capture.id)
            == this->m_captures.end())
        {
            this->m_captures.push_back(capture.id);

            assert(this->m_captures_by_id.size() < capture.id);
            this->m_captures_by_id.resize(capture.id + 1);
        }

        assert(this->m_captures_by_id.size() >= capture.id);
        this->m_captures_by_id.at(capture.id).push_back(capture);

        i++;
    }
}
}  // namespace log_mechanic

#endif  // LOGMECH_LOG_MECHANIC_HPP
