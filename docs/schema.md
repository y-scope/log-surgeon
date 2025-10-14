# Schema specification

A schema file defines the **delimiters** and **variable patterns** (regular expressions) that
`log-surgeon` uses to parse log events. Delimiters conceptually divide the input into *tokens*,
where each token is either a variable (matched by a pattern) or **static text**. Variable tokens may
include delimiters and are treated as a single token. Static-text always begins and ends with a
delimiter (or the start/end of the log event). This structure enables `log-surgeon` to extract
variables from otherwise unstructured log events.

## Schema syntax

A schema file consists of a list of *rules*, each defined by a *name* and *pattern* (regular
expression). These rules dictate how `log-surgeon` identifies and categorizes parts of a log event.

There are three types of rules in a schema file:

* [Variables](#variables): Defines patterns for capturing specific pieces of the log.
* [Delimiters](#delimiters): Specifies the characters that separate tokens in the log.
* [Timestamps](#timestamps): Identifies the boundary between log events. Timestamps are also treated
  as variables.

For documentation, the schema allows for user comments by ignoring any text preceded by `//`.

### Variables

**Syntax:**

```txt
<variable-name>:<variable-pattern>
```

* `variable-name` may contain any alphanumeric characters, but may not be the reserved names
  `delimiters` or `timestamp`.
* `variable-pattern` is a regular expression using the supported
  [syntax](#regular-expression-syntax).

Note that:

* A schema file may contain zero or more variable rules.
* Repeating the same variable name in another rule will `OR` the regular expressions (perform an
  alternation).
* If a token matches multiple patterns from multiple rules, the token will be assigned the name of
  each rule, in the same order that they appear in the schema file.

### Delimiters

**Syntax:**

```txt
delimiters:<characters>
```

* `delimiters` is a reserved name for this rule.
* `characters` is a set of characters that should be treated as delimiters. These characters define
  the boundaries between tokens in the log.

Note that:

* A schema file must contain at least one `delimiters` rule. If multiple `delimiters` rules are
  specified, only the last one will be used.

### Timestamps

**Syntax:**

```txt
timestamp:<timestamp-pattern>
```

* `timestamp` is a reserved name for this rule.
* `timestamp-pattern` is a regular expression using the supported
  [syntax](#regular-expression-syntax).

Note that:

* The parser uses a timestamp to denote the start of a new log event if:
  * It appears as the first token in the input, or
  * It appears after a newline character.
* Until a timestamp is found, the parser will use a newline character to denote the start of a new
  log event.

## Example schema file

```txt
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

* `delimiters: \t\r\n:,!;%` indicates that ` `, `\t`, `\r`, `\n`, `:`, `,`, `!`, `;`, and `%` are
  delimiters.
* `timestamp` matches two different patterns:
  * `2023-04-19 12:32:08.064`
  * `[20230419-12:32:08]`
* `int`, `float`, `hex`, `hasNumber`, and `equalsCapture` all match different user defined
  variables.
* `equalsCapture` also contains the named capture group `equals`. This allows the user to extract
  the substring following the equals sign.

## Regular Expression Syntax

The following regular expression rules are supported by the schema. When building a regular
expression, the rules are applied as they appear in this list, from top to bottom.

```txt
REGEX RULE       EXAMPLE       DEFINITION
Concatenation    ab            Match two expressions in sequence (e.g., 'a'
                                 followed by 'b').
Alternation      a|b           Match one of two expressions (e.g., 'a' or 'b').
Range            [a-z]         Match any character within a specified range
                                 (e.g., any lowercase letter).
Negated Range    [^a-zA-Z]     Match any character not within the specified
                                 range (e.g., any non-alphabet character).
Kleene Star      a*            Match an expression zero or more times.
Kleene Plus      a+            Match an expression one or more times.
Repetition       a{N}          Match an expression exactly N times.
Repetition Range a{N,M}        Match an expression between N and M times.
Digit            \d            Match any digit (i.e., 0-9).
Whitespace       \s            Match any whitespace character (i.e., ' ', \r,
                                 \t, \v, or \f).
Wildcard         .             Match any non-delimiter character.
Subexpression    (ab)          Match the expression in parentheses (e.g., ab).
Named Capture    (?<var>[01]+) Match an expression and assign it a name (e.g.,
                                 capture a binary string as "var").

* Special characters include: ( ) * + - . [ \ ] ^ { | } < > ?
  - Escape these with '\' when used literally (e.g., \., \(, \\).
  - Special characters must be escaped even in ranges.

* For each regex rule, the expression(s) it contains can be formed by applying
  any sequence of valid regex rules, including the rule itself, thus allowing
  for recursive composition.
  - Note: Recursive capture groups are currently untested.
```
