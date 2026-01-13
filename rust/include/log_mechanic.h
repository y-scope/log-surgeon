#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>


typedef struct LogComponent LogComponent;

typedef struct Nfa Nfa;

typedef struct Regex Regex;

typedef struct Schema Schema;

void clp_log_surgeon_component_delete(struct LogComponent *component);

size_t clp_log_surgeon_component_matches_count(const struct LogComponent *component);

void clp_log_surgeon_component_matches_get(const struct LogComponent *component,
                                           size_t i,
                                           size_t *rule,
                                           const uint8_t **name,
                                           size_t *name_len,
                                           size_t *start,
                                           size_t *end);

const uint8_t *clp_log_surgeon_component_text(const struct LogComponent *component, size_t *len);

void clp_log_surgeon_nfa_debug(const struct Nfa *nfa);

void clp_log_surgeon_nfa_delete(struct Nfa *nfa);

struct Nfa *clp_log_surgeon_nfa_for_schema(const struct Schema *schema);

struct LogComponent *clp_log_surgeon_parse(const struct Nfa *nfa,
                                           const uint8_t *input,
                                           size_t len,
                                           size_t *pos);

void clp_log_surgeon_regex_delete(struct Regex *regex);

struct Regex *clp_log_surgeon_regex_from_pattern(const uint8_t *pattern, size_t len);

void clp_log_surgeon_schema_add_rule(struct Schema *schema,
                                     const uint8_t *name,
                                     size_t len,
                                     struct Regex *regex);

void clp_log_surgeon_schema_delete(struct Schema *schema);

struct Schema *clp_log_surgeon_schema_new(void);
