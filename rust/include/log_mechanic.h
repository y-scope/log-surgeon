#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>


typedef struct Lexer Lexer;

typedef struct Schema Schema;

typedef struct CSlice_c_char {
  const char *pointer;
  size_t length;
} CSlice_c_char;

typedef struct CSlice_c_char CStringView;

typedef struct Capture {
  CStringView name;
  const uint8_t *start;
  const uint8_t *end;
} Capture;

typedef struct LogComponent {
  size_t rule;
  const uint8_t *start;
  const uint8_t *end;
  const struct Capture *captures;
  size_t captures_count;
} LogComponent;

struct CSlice_c_char clp_log_mechanic_c_string_view(const char *pointer);

void clp_log_mechanic_lexer_delete(struct Lexer *lexer);

struct Lexer *clp_log_mechanic_lexer_new(const struct Schema *schema);

bool clp_log_mechanic_lexer_next_token(struct Lexer *lexer,
                                       CStringView input,
                                       size_t *pos,
                                       struct LogComponent *log_component);

void clp_log_mechanic_schema_add_rule(struct Schema *schema, CStringView name, CStringView pattern);

void clp_log_mechanic_schema_delete(struct Schema *schema);

struct Schema *clp_log_mechanic_schema_new(void);

void clp_log_mechanic_schema_set_delimiters(struct Schema *schema, CStringView delimiters);
