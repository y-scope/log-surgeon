# Schema specification

A schema file specifies the delimiters and variables patterns (regular
expressions) necessary for `log-surgeon` to parse log events. `log-surgeon` uses
the delimiters to find tokens<sup>†</sup> (as in, strings of non-delimiters) in
the input, and categorizes any token that matches a variable pattern as a
variable. Any tokens that are not categorized as variables are treated as static
text. In essence, this allows the user to parse variables out of their
unstructured log events.

`log-surgeon` also assigns types to each variable based on the variable
pattern's name in the schema file. 

<sup>†</sup> Internally, `log-surgeon`'s lexer also treats a string of
delimiters as a token, just not one that matches a variable pattern.

## Schema syntax

A schema file essentially contains a list of *rules*, each of which has a *name*
and a *pattern* (regular expression).

There are three types of rules in a schema file:

* [Variables](#variables)
* [Delimiters](#delimiters)
* [Timestamps](#timestamps)

### Variables

**Syntax:**

```
<variable-name>:<variable-pattern>
```
* `variable-name` may contain any alphanumeric characters, but may not be
  the reserved names `delimiters` or `timestamp`.
* `variable-pattern` is a regular expression using the supported
  [syntax](#regular-expression-syntax), but it **cannot** contain characters
  defined as [delimiters](#delimiters).

Note that:
* A schema file may contain zero or more variable rules.
* Repeating the same variable name in another rule will `OR` the regular
  expressions (preform an alternation).
* If a token matches multiple patterns from multiple rules, the token will be
  assigned the name of each rule, in the same order that they appear in the
  schema file.

### Delimiters

**Syntax:**

```
delimiters:<characters>
```
* `delimiters` is a reserved name for this rule
* `characters` is a set of characters that should be treated as delimiters

Note that:
* A schema file must contain a single `delimiters` rule. If multiple
  `delimiters` rules are specified, only the last one will be used.

### Timestamps

**Syntax:**

```
timestamp:<timestamp-pattern>
```
* `timestamp` is a reserved name for this rule
* `timestamp-pattern` is a regular expression using the supported
  [syntax](#regular-expression-syntax)

Note that:
* Unlike [variable](#variables) patterns, timestamp patterns can contain
  delimiters.
* The parser uses a timestamp to denote the start of a new log event if:
  * ... it appears as the first token in the input, or
  * ... it appears after a newline character.
* Until a timestamp is found, the parser will use a newline character to denote
  the start of a new log event.
* The timestamp pattern is not used to match text inside a log event; since the
  pattern can contain delimiters, no token can match it. 

### Comments

**Syntax:** Comments are any text preceded by `//`.

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
equals:.*=.*[a-zA-Z0-9].*
```
* `delimiters: \t\r\n:,!;%` indicates that ` `, `\t`, `\r`, `\n`, `:`, `,`,
  `!`, `;`, `%`, and `'` are delimiters. In a log file, consecutive delimiters,
  e.g., N consecutive spaces, will be tokenized as static text.
* `timestamp` matches two different patterns:
    * 2023-04-19 12:32:08.064
    * [20230419-12:32:08]
* `int`, `float`, `hex`, `hasNumber`, and `equals` all match different user
  defined variables.

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
(abc)        Subexpression (concatenates abc)
\d           Match any digit 0-9
\s           Match any whitespace character (' ', '\r', '\t', '\v', or '\f')
.            Match any *non-delimiter* character
```
