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
  /// `0` iff no variable found (static text until end of input).
  size_t rule;
  /// Start of variable (if found).
  const uint8_t *start;
  /// End of variable (if found).
  const uint8_t *end;
  /// Pointer to an array of captures (if variable found).
  const Capture *captures;
  /// Number of captures.
  size_t captures_count;
  /// Whether this fragment starts a new log event (timestamp at start of line).
  bool is_event_start;

  CLogFragment(size_t const& rule,
               const uint8_t *const& start,
               const uint8_t *const& end,
               const Capture *const& captures,
               size_t const& captures_count,
               bool const& is_event_start)
    : rule(rule),
      start(start),
      end(end),
      captures(captures),
      captures_count(captures_count),
      is_event_start(is_event_start)
  {}

};


extern "C" {

void clp_log_mechanic_lexer_delete(Lexer *lexer);

Lexer *clp_log_mechanic_lexer_new(const Schema *schema);

/// Very unsafe!
///
/// The returned [`CLogFragment`] includes a hidden exclusive borrow of `lexer`
/// (it contains a pointer into an internal buffer of `lexer`),
/// so it is no longer valid (you must not touch it) after any subsequent borrow of `lexer`
/// (i.e. this borrow has ended).
CLogFragment clp_log_mechanic_lexer_next_fragment(Lexer *lexer, CStringView input, size_t *pos);

bool clp_log_mechanic_schema_add_rule(Schema *schema, CStringView name, CStringView pattern);

bool clp_log_mechanic_schema_add_timestamp_rule(Schema *schema,
                                                CStringView name,
                                                CStringView pattern);

void clp_log_mechanic_schema_delete(Box<Schema> schema);

Box<Schema> clp_log_mechanic_schema_new();

size_t clp_log_mechanic_schema_rule_count(const Schema *schema);

CStringView clp_log_mechanic_schema_rule_name(const Schema *schema, size_t index);

bool clp_log_mechanic_schema_set_delimiters(Schema *schema, CStringView delimiters);

void clp_log_mechanic_set_debug(bool enable);

}  // extern "C"

}  // namespace clp::log_mechanic
