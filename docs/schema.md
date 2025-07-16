# Schema specification

A schema file defines the **delimiters** and **variable patterns** (regular
expressions) that `log-surgeon` uses to parse log events. Delimiters
conceptually divide the input into *tokens*, where each token is either a
variable (matched by a pattern) or **static text**. Variable tokens may include
delimiters and are treated as a single token. Static-text always begins and ends
with a delimiter. This structure enables `log-surgeon` to extract variables from
otherwise unstructured log events.

## Schema syntax

A schema file consists of a list of *rules*, each defined by a *name* and
*pattern* (regular expression). These rules dictate how `log-surgeon` identifies
and categorizes parts of a log event.

There are three types of rules in a schema file:

* [Variables](#variables): Defines patterns for capturing specific pieces of the
  log.
* [Delimiters](#delimiters): Specifies the characters that separate tokens in
  the log.
* [Timestamps](#timestamps): Identifies the boundary between log events.
  Timestamps are also treated as variables.

For documentation, the schema allows for user comments by ignoring any text
preceded by `//`.

### Variables

**Syntax:**

```
<variable-name>:<variable-pattern>
```
* `variable-name` may contain any alphanumeric characters, but may not be
  the reserved names `delimiters` or `timestamp`.
* `variable-pattern` is a regular expression using the supported
  [syntax](#regular-expression-syntax).

Note that:
* A schema file may contain zero or more variable rules.
* Repeating the same variable name in another rule will `OR` the regular
  expressions (perform an alternation).
* If a token matches multiple patterns from multiple rules, the token will be
  assigned the name of each rule, in the same order that they appear in the
  schema file.

### Delimiters

**Syntax:**

```
delimiters:<characters>
```
* `delimiters` is a reserved name for this rule.
* `characters` is a set of characters that should be treated as delimiters.
  These characters define the boundaries between tokens in the log.

Note that:
* A schema file must contain at least one `delimiters` rule. If multiple
  `delimiters` rules are specified, only the last one will be used.

### Timestamps

**Syntax:**

```
timestamp:<timestamp-pattern>
```
* `timestamp` is a reserved name for this rule.
* `timestamp-pattern` is a regular expression using the supported
  [syntax](#regular-expression-syntax).

Note that:
* The parser uses a timestamp to denote the start of a new log event if:
  * It appears as the first token in the input, or
  * It appears after a newline character.
* Until a timestamp is found, the parser will use a newline character to denote
  the start of a new log event.

## Example schema file

```
// Delimiters
delimiters: \t\r\n:,!;%

// Keywords
timestamp:\d{4}\-\d{2}\-\d{2} \d{2}:\d{2}:\d{2}(\.\d{3}){0,1}
timestamp:\[\d{8}\-\d{2}:\d{2}:\d{2}\]
int:\-{0,1}[0-9]+
float:\-{0,1}[0-9]+\.[0-9]+

// Custom variables
hex:[a-fA-F]+
hasNumber:.*\d.*
equalsCapture:.*=(?<equals>.*[a-zA-Z0-9].*)
```
* `delimiters: \t\r\n:,!;%'` indicates that ` `, `\t`, `\r`, `\n`, `:`, `,`,
  `!`, `;`, `%`, and `'` are delimiters.
* `timestamp` matches two different patterns:
    * 2023-04-19 12:32:08.064
    * [20230419-12:32:08]
* `int`, `float`, `hex`, `hasNumber`, and `equalsCapture` all match different
  user defined variables.
* `equalsCapture` also contains the named capture group `equals`. This allows
  the user to extract the substring following the equals sign.

## Regular Expression Syntax

The following regular expression rules are supported by the schema. When
building a regular expression, the rules are applied as they appear in this
list, from top to bottom.
```
REGEX RULE   DEFINITION
ab           Match 'a' followed by 'b'
a|b          Match a OR b
[a-z]        Match any character in the brackets (e.g., any lowercase letter)
             - special characters must be escaped, even in brackets (e.g., [\.\(\\])
[^a-zA-Z]    Match any character NOT in the brackets (e.g., non-alphabet character)
a*           Match 'a' 0 or more times
a+           Match 'a' 1 or more times
a{N}         Match 'a' exactly N times
a{N,M}       Match 'a' between N and M times
\d           Match any digit 0-9
\s           Match any whitespace character (' ', '\r', '\t', '\v', or '\f')
.            Match any *non-delimiter* character
(abc)        Subexpression (concatenates abc)
(?<name>abc) Named capture group (matches 'abc' and saves it as 'name')
```
