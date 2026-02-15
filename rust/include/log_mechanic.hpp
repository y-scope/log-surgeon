#ifndef LOG_MECHANIC_HPP
#define LOG_MECHANIC_HPP

#include <type_traits>
#include <string_view>
#include <optional>

namespace clp::log_mechanic {

	template<typename T> using Box = T*;

	template<typename T> struct is_rust_box_t: std::false_type {};
	template<typename T> struct is_rust_box_t<Box<T>>: std::true_type {};
	template<typename T> constexpr bool is_rust_box_v = is_rust_box_t<T>::value;

	template<typename T> using Option = std::enable_if_t<is_rust_box_v<T>, T>;
}

#include "log_mechanic.generated.hpp"

namespace clp::log_mechanic {
	class Variable {
	public:
		size_t rule;
		std::string_view name;
		std::string_view lexeme;

		Variable(LogEvent* event, size_t index, CVariable variable)
			: rule(variable.rule)
			, name(variable.name)
			, lexeme(variable.lexeme)
			, m_event(event)
			, m_index(index)
		{}

		std::optional<CCapture> capture(size_t j) const {
			CCapture capture { clp_log_mechanic_log_event_capture(this->m_event, this->m_index, j) };
			if (capture.name.pointer != nullptr) {
				return std::make_optional(capture);
			}
			return std::nullopt;
		}

	private:
		LogEvent* m_event;
		size_t m_index;
	};

	class EventHandle {
	public:
		EventHandle(LogEvent* pointer) : m_pointer(pointer) {}

		std::string_view log_type() const {
			return clp_log_mechanic_log_event_log_type(this->m_pointer);
		}

		std::optional<Variable> operator[](size_t i) const {
			CVariable variable { clp_log_mechanic_log_event_variable(this->m_pointer, i) };
			if (variable.name.pointer != nullptr) {
				return std::make_optional(Variable { this->m_pointer, i, variable });
			}
			return std::nullopt;
		}

	private:
		LogEvent* m_pointer;
	};

	class ParserHandle {
	public:
		ParserHandle(Schema const* schema) {
			this->m_parser = clp_log_mechanic_parser_new(schema);
			this->m_event = clp_log_mechanic_log_event_new();
		}
		~ParserHandle() {
			clp_log_mechanic_log_event_drop(this->m_event);
			clp_log_mechanic_parser_drop(this->m_parser);
		}

		std::optional<EventHandle> next_event(std::string_view const& input, size_t* pos) {
			if (!clp_log_mechanic_parser_next(this->m_parser, input, pos, this->m_event)) {
				return std::nullopt;
			}
			return std::make_optional(EventHandle { this->m_event });
		}

	private:
		Parser* m_parser;
		LogEvent* m_event;
	};
}

#endif // LOG_MECHANIC_HPP
