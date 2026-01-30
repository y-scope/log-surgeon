#include <cstdarg>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>


namespace clp::log_mechanic {

struct Lexer;

struct Schema;

struct CLogFragment {
// Custom
CLogFragment() = default;

// Generated
  /// `0` iff no variable found (static text until end of input).
  size_t rule;
  /// Start of variable (if found).
  const uint8_t *start;
  /// End of variable (if found).
  const uint8_t *end;

  CLogFragment(size_t const& rule,
               const uint8_t *const& start,
               const uint8_t *const& end)
    : rule(rule),
      start(start),
      end(end)
  {}

};

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

using LogFragmentOnCapture = void(*)(const void *data, CStringView name, CStringView lexeme);


extern "C" {

void clp_log_mechanic_lexer_delete(Box<Lexer> lexer);

Box<Lexer> clp_log_mechanic_lexer_new(const Schema *schema);

CLogFragment clp_log_mechanic_lexer_next_fragment(Lexer *lexer,
                                                  CStringView input,
                                                  size_t *pos,
                                                  LogFragmentOnCapture maybe_closure,
                                                  const void *data);

void clp_log_mechanic_schema_add_rule(Schema *schema, CStringView name, CStringView pattern);

void clp_log_mechanic_schema_delete(Box<Schema> schema);

Box<Schema> clp_log_mechanic_schema_new();

void clp_log_mechanic_schema_set_delimiters(Schema *schema, CStringView delimiters);

}  // extern "C"

}  // namespace clp::log_mechanic
