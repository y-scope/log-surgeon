#ifndef LOG_MECHANIC_HPP
#define LOG_MECHANIC_HPP

#include <cassert>
#include <string_view>
#include <optional>
#include <vector>
#include <algorithm>

#include "log_mechanic.generated.hpp"

namespace log_mechanic {
	class Variable {
	public:
		size_t rule;
		std::string_view name;
		std::string_view lexeme;

		using CaptureIterator = std::vector<uint32_t>::const_reverse_iterator;

		Variable(LogEvent& event, size_t index, CVariable variable)
			: rule(variable.rule)
			, name(variable.name)
			, lexeme(variable.lexeme)
			, m_event(event)
			, m_index(index)
		{
			size_t i { 0 };
			while (true) {
				CCapture capture { logmech_log_event_capture(&this->m_event, this->m_index, i) };
				if (capture.id == 0) {
					break;
				}

				if (std::find(this->m_captures.begin(), this->m_captures.end(), capture.id) == this->m_captures.end()) {
					this->m_captures.push_back(capture.id);

					assert(this->m_captures_by_id.size() < capture.id);
					this->m_captures_by_id.resize(capture.id + 1);
				}

				assert(this->m_captures_by_id.size() >= capture.id);
				this->m_captures_by_id[capture.id].push_back(capture);

				i++;
			}
		}

		CaptureIterator begin() const {
			return this->m_captures.rbegin();
		}

		CaptureIterator end() const {
			return this->m_captures.rend();
		}

		std::vector<CCapture> const& operator[](uint32_t id) const {
			return this->m_captures_by_id[id];
		}

	private:
		LogEvent& m_event;
		size_t m_index;
		std::vector<uint32_t> m_captures;
		std::vector<std::vector<CCapture>> m_captures_by_id;
	};

	class EventHandle {
	public:
		EventHandle(LogEvent& event) : m_event(event) {}

		std::string_view log_type() const {
			return logmech_log_event_log_type(&this->m_event);
		}

		std::optional<Variable> operator[](size_t i) const {
			CVariable variable { logmech_log_event_variable(&this->m_event, i) };
			if (variable.name.pointer != nullptr) {
				return std::make_optional(Variable { this->m_event, i, variable });
			}
			return std::nullopt;
		}

	private:
		LogEvent& m_event;
	};

	class ParserHandle {
	public:
		ParserHandle(Schema const* schema) {
			this->m_parser = logmech_parser_new(schema);
			this->m_event = logmech_log_event_new();
		}
		~ParserHandle() {
			logmech_log_event_drop(this->m_event);
			logmech_parser_drop(this->m_parser);
		}
		ParserHandle(ParserHandle const& other) noexcept {
			this->m_parser = logmech_parser_clone(this->m_parser);
			this->m_event = logmech_log_event_clone(this->m_event);
		}
		ParserHandle& operator=(ParserHandle other) noexcept {
			swap(*this, other);
			return *this;
		}

		friend void swap(ParserHandle& first, ParserHandle& second) noexcept {
			using std::swap;

			swap(first.m_parser, second.m_parser);
			swap(first.m_event, second.m_event);
		}

		std::optional<EventHandle> next_event(std::string_view const& input, size_t* pos) {
			if (!logmech_parser_next(this->m_parser, input, pos, this->m_event)) {
				return std::nullopt;
			}
			return std::make_optional(EventHandle { *this->m_event });
		}

	private:
		Parser* m_parser;
		LogEvent* m_event;
	};
}

#endif // LOG_MECHANIC_HPP
