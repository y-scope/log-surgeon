#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>


namespace clp::log_mechanic {

struct Lexer;

struct Schema;

template<typename T>
struct CSlice {
CSlice(std::string_view const& view)
	requires std::is_same_v<T, char>
	: CSlice(view.data(), view.size())
	{}

CSlice(char const* c_str)
	requires std::is_same_v<T, char>
	: CSlice(std::string_view { c_str })
	{}
  const T *pointer;
  size_t length;

  CSlice(const T *const& pointer,
         size_t const& length)
    : pointer(pointer),
      length(length)
  {}

};

using CStringView = CSlice<char>;

struct Capture {
  CStringView name;
  const uint8_t *start;
  const uint8_t *end;

  Capture(CStringView const& name,
          const uint8_t *const& start,
          const uint8_t *const& end)
    : name(name),
      start(start),
      end(end)
  {}

};

struct LogComponent {
LogComponent() = default;
  size_t rule;
  const uint8_t *start;
  const uint8_t *end;
  const Capture *captures;
  size_t captures_count;

  LogComponent(size_t const& rule,
               const uint8_t *const& start,
               const uint8_t *const& end,
               const Capture *const& captures,
               size_t const& captures_count)
    : rule(rule),
      start(start),
      end(end),
      captures(captures),
      captures_count(captures_count)
  {}

};


extern "C" {

void clp_log_mechanic_lexer_delete(Box<Lexer> lexer);

Box<Lexer> clp_log_mechanic_lexer_new(const Schema *schema);

bool clp_log_mechanic_lexer_next_token(Lexer *lexer,
                                       CStringView input,
                                       size_t *pos,
                                       LogComponent *log_component);

void clp_log_mechanic_schema_add_rule(Schema *schema, CStringView name, CStringView pattern);

void clp_log_mechanic_schema_delete(Box<Schema> schema);

Box<Schema> clp_log_mechanic_schema_new();

void clp_log_mechanic_schema_set_delimiters(Schema *schema, CStringView delimiters);

}  // extern "C"

}  // namespace clp::log_mechanic
