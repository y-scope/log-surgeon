#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>


namespace clp::log_mechanic {

struct LogEvent;

struct Parser;

struct RegexError;

struct Schema;

template<typename T>
struct CSlice {
// Custom
CSlice(std::string_view const& view)
	requires std::is_same_v<T, char>
	: CSlice(view.data(), view.size())
{}

CSlice(char const* c_str)
	requires std::is_same_v<T, char>
	: CSlice(std::string_view { c_str })
{}

operator std::string_view() const
	requires std::is_same_v<T, char>
{
	return this->as_cpp_view();
}

std::string_view as_cpp_view() const
	requires std::is_same_v<T, char>
{
	return { this->pointer, this->length };
}

// Generated
  const T *pointer;
  size_t length;

  CSlice(const T *const& pointer,
         size_t const& length)
    : pointer(pointer),
      length(length)
  {}

};

using CStringView = CSlice<char>;

struct CCapture {
  CStringView name;
  CStringView lexeme;
  uint32_t id;
  bool is_leaf;

  CCapture(CStringView const& name,
           CStringView const& lexeme,
           uint32_t const& id,
           bool const& is_leaf)
    : name(name),
      lexeme(lexeme),
      id(id),
      is_leaf(is_leaf)
  {}

};

struct CVariable {
  size_t rule;
  CStringView name;
  CStringView lexeme;

  CVariable(size_t const& rule,
            CStringView const& name,
            CStringView const& lexeme)
    : rule(rule),
      name(name),
      lexeme(lexeme)
  {}

};


extern "C" {

CCapture clp_log_mechanic_log_event_capture(const LogEvent *log_event, size_t i, size_t j);

void clp_log_mechanic_log_event_drop(Box<LogEvent> value);

bool clp_log_mechanic_log_event_have_header(const LogEvent *log_event);

CStringView clp_log_mechanic_log_event_log_type(const LogEvent *log_event);

Box<LogEvent> clp_log_mechanic_log_event_new();

CVariable clp_log_mechanic_log_event_variable(const LogEvent *log_event, size_t i);

void clp_log_mechanic_parser_drop(Box<Parser> value);

Box<Parser> clp_log_mechanic_parser_new(const Schema *schema);

bool clp_log_mechanic_parser_next(Parser *parser, CStringView input, size_t *pos, LogEvent *out);

void clp_log_mechanic_regex_error_drop(Box<RegexError> value);

Option<Box<RegexError>> clp_log_mechanic_schema_add_rule(Schema *schema,
                                                         CStringView name,
                                                         CStringView pattern);

void clp_log_mechanic_schema_drop(Box<Schema> value);

Box<Schema> clp_log_mechanic_schema_new();

void clp_log_mechanic_schema_set_delimiters(Schema *schema, CStringView delimiters);

}  // extern "C"

}  // namespace clp::log_mechanic
