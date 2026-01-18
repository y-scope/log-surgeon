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

struct Capture {
  CStringView name;
  CStringView lexeme;

  Capture(CStringView const& name,
          CStringView const& lexeme)
    : name(name),
      lexeme(lexeme)
  {}

};

struct CLogFragment {
// Custom
CLogFragment() = default;

// Generated
  size_t rule;
  const uint8_t *start;
  const uint8_t *end;
  const Capture *captures;
  size_t captures_count;

  CLogFragment(size_t const& rule,
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

/// Very unsafe!
///
/// The returned [`CLogFragment`] includes a hidden exclusive borrow of `lexer`
/// (it contains a pointer into an interal buffer of `lexer`),
/// so it is nolonger valid/you must not use it after a subsequent exclusive borrow of `lexer`
/// (i.e. this borrow has ended).
CLogFragment clp_log_mechanic_lexer_next_fragment(Lexer *lexer, CStringView input, size_t *pos);

void clp_log_mechanic_schema_add_rule(Schema *schema, CStringView name, CStringView pattern);

void clp_log_mechanic_schema_delete(Box<Schema> schema);

Box<Schema> clp_log_mechanic_schema_new();

void clp_log_mechanic_schema_set_delimiters(Schema *schema, CStringView delimiters);

}  // extern "C"

}  // namespace clp::log_mechanic
