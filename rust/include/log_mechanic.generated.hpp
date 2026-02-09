#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>


namespace clp::log_mechanic {

struct LogEvent;

struct Parser;

struct Schema;

struct Token;

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


extern "C" {

CStringView clp_log_mecahnic_event_token_name(const Token *token);

void clp_log_mechanic_event_delete(Box<LogEvent> event);

const Token *clp_log_mechanic_event_token(const LogEvent *event, size_t i);

size_t clp_log_mechanic_event_token_count(const LogEvent *event);

size_t clp_log_mechanic_event_token_rule(const Token *token);

void clp_log_mechanic_parser_delete(Box<Parser> parser);

Box<Parser> clp_log_mechanic_parser_new(const Schema *schema);

Option<Box<LogEvent>> clp_log_mechanic_parser_next_event(Parser *parser,
                                                         CStringView input,
                                                         size_t *pos);

void clp_log_mechanic_schema_add_rule(Schema *schema, CStringView name, CStringView pattern);

void clp_log_mechanic_schema_delete(Box<Schema> schema);

Box<Schema> clp_log_mechanic_schema_new();

void clp_log_mechanic_schema_set_delimiters(Schema *schema, CStringView delimiters);

}  // extern "C"

}  // namespace clp::log_mechanic
