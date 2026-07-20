## Parsing Specification
A parsing specification is a list of regex rules used to determine semantically meaningful text in logs.
It can be defined programmatically using the C++ or Python API,
or using a Parsing Specification File.

In a parsing specification file,
a rule is written as `name: "regex-pattern"`.
Log Surgeon supports most common regex syntax/semantics, detailed [below][regex-pattern-syntax]
(the outside double-quotes are not part of the pattern).
For example, a rule `int: "[0-9]+"` may be used to identify numbers.

Parsing first considers longest match,
and priority is used as a tie-breaker when multiple rules match with the same length (from the same starting position).
There are two levels to priority.
The first is an optional integer value (defaulting to `0`) written after the rule name, e.g. `int (-10): "[0-9]+"`.
Higher integer values correspond to higher priority.
If multiple rules match with the same length, starting position, and integer priority,
the first such rule in the file is matched/returned.

The rules defined at the "top-level" are also called root rules;
"regex captures" in a pattern define subrules as `(?<sub_rule_name>sub_rule_pattern)`, arbitrarily nested.
Log Surgeon extracts both root rules and subrules when parsing;
the extracted values are called matches.

Note:
We generally avoid "capture" terminology to avoid ambiguity between the root rule match and sub-match extraction,
which require different implementations internally.
From the outside, Log Surgeon matches and stores both root rule matches and subrule regex captures alike,
so we simply refer to them as "rules", "patterns", and "matches".

A notable distinction is that subrules have a fully qualified name containing its parent/ancestor rules' names;
since a root rule has no parents, its fully qualified name is just its own name.
The fully qualified name of a rule starts with its root rule name and is followed by any/all subrule names,
separated by periods, e.g. `foo.bar.baz`.

A (sub)rule with no child subrules is called a leaf rule;
a root rule whose pattern has no regex captures is also a leaf rule.
For example, the root rule `email: "(?<user>\w+)@(?<hostname>((?<subdomain>\w+)\.)*(?<domain>\w+)\.(?<tld>\w+))"`
has leaf rules `email.user`, `email.subdomain`, `email.domain`, and `email.tld`.

A name denotes the "type" of the matched text,
and the same name/type may have multiple rules/patterns.
For example:

```
timestamp: "\d{2}-\d{2}-\d{4} \d{2}:\d{2}:\d{2}(\.\d{3})?"
timestamp: "\d{2}\\[A-Z][a-z]{2}\\\d{4}:\d{2}\d{2}:\d{2}"
```

A parsing specification must also contain a `delimiter: "..."` definition,
which is a set of characters additionally used to identify matches;
see [Parsing][parsing] for more details.
Newlines (`\n`) must be part of the delimiter set,
since they already delimit the start and end of log events.

### Example Parsing Specification File
Again, a parsing specification file primarily consists of a priority-ordered list `name: "pattern"` rules,
plus a delimiter set of characters.
For example:

```
delimiters: ".,?!:;[]{}() \n\r\t"

!hex_digit: "[0-9a-fA-F]"

username: "@(?<username>\w+)"
ipv4: "\d+\.\d+\.\d+\.\d+"
ipv6: "(?<hex_digit>)+(::(?<hex_digit>)){7}"
int: "^[0-9]+$"
```

In the above example, `!hex_digit` defines a placeholder,
later referred to by an empty regex capture `(?<hex_digit>)`.
The pattern of a placeholder is substituted in-place as a single subexpression,
and on its own **is not** a subrule,
though any subrules in the placeholder's pattern are likewise included in the substitution.
In the following example, `rule1` and `rule2` are semantically equivalent to each other, but not to `rule3` or `rule4`:

```
!greeting: "hello"

rule1: "(?<greeting>)+"
rule2: "(hello)+"
rule3: "hello+"
rule4: "(?<greeting>hello)+"
```

#### Other Notes
- Rules may not match the empty string;
	e.g. the root rule pattern `[0-9]*` or subrule pattern `(?<number>[0-9]*)` is not allowed.
- Placeholders must be defined before they are used.
- Rules must be defined on a single line (they cannot span multiple lines).
- Leading and trailing whitespace on a line are ignored.
- Empty lines are ignored.
- A line starting with a hashtag `#` (ignoring whitespace) is a comment and ignored.
- While a double quote `"` is not special in regex syntax,
	when writing a pattern in a parsing specification file, they must be escaped as `\"`.

### Regex Pattern Syntax
Regexes are ("regular") expressions composed of terms and operators.

Terms are:

- individual characters/character sets:
	- `.`, which matches any single character, including newlines
	- a literal character, e.g. `a`, `\.` (to match a literal `.`), or `\\`
	- bracketed ranges, e.g. `[a-z]` or `[a-z0-9ABC]`
	- dedicated character sets, e.g. `\d` equivalent to `[0-9]`
- parenthesized expressions, e.g. `(abc)`
- subrules, e.g. `(?<greeting>hello world)`
- references to placeholders, e.g. `(?<greeting>)`

See [Escape Characters][escape-characters] and [Bracketed Ranges][bracketed-ranges] below
for exact details on individual characters and character sets.

Operators are, from highest to lowest precedence (and always left-associative):

- postfix repetition, e.g. `a*` (0 or more), `a+` (1 or more), `a?` (0 or 1),
	`a{n}` (exactly `n`), and `a{min,max}` (at least `min`, up to and including `max` times)
- binary concatenation, e.g. `a*b`, equivalent to `(a*)b`
- binary alternation ("or"), e.g. `a*b|c`, equivalent to `((a*)b)|c`

Furthermore, a root rule's pattern may be anchored with a leading `^` or ending `$`,
meaning the match must be preceded/followed by one of the delimiter characters or the very start/end of input.
Note that this differs from common regex usage, where `^` and `$` anchor only to the very start/end of input.

More explicitly, pattern syntax follows this [EBNF][ebnf] grammar.

```
// A root pattern may be "anchored".
root_pattern: "^"? alternation "$"?

alternation: sequence ("|" sequence)*

sequence: suffixed_term+

suffixed_term: term repetition_suffix?

term:
	"."
	bracketed_ranges
	symbol
	"(" alternation ")"
	"(?<" name ">" alternation ")" // A subrule.
	"(?<" name ">)" // A reference to a placeholder.

repetition_suffix:
	"*" // 0 or more.
	"+" // 1 or more.
	"?" // 0 or 1.
	"{" decimal_integer "}" // Repeat exactly this many times.
	"{" decimal_integer "," decimal_integer "}" // Repeat min to max times (inclusive).

bracketed_ranges: "[" bracketed_item+ "]"

symbol:
	"\" escaped_character
	unescaped_character

bracketed_item:
	"^"
	"\^"
	symbol "-" symbol // Character range, inclusive.
	symbol
```

#### Escape Characters
The following meta-characters must generally be escaped with a backslash: `\()[]{}*+?.|^$`.
Furthermore, the following common escapes are supported:

- `\t`, `\r`, and `\n` correspond to ASCII tab, carriage return, and newline feed respectively.
- `\d`, `\w`, and `\s` correspond to `[0-9]`, `[a-zA-Z0-9]`, and `[ \t\r\n]` respectively.
- `\D`, `\W`, and `\S` correspond to the negation of their lowercase counterparts.
- `\u{xx}`, `\u{xxyy}`, and `\u{xxyyzz}` translate to the corresponding Unicode code points in hexadecimal.
	Hexadecimal digits may be upper or lower case and must come in pairs.
	Note that the maximum Unicode code point is `\u{10FFFF}`, so at most 3 pairs are necessary.
- Additionally, `\ ` (space), `\'` (single quote), and `\"` (double quote) correspond to their literal values,
	and may be used to avoid ambiguity.

#### Bracketed Ranges
The interpretation of `bracketed_item`s in a `bracketed_range` is not strictly context-free
(don't worry about this unless you care about formal languages),
but follows common regex syntax conventions:

- If the first item is `^`, the range is negated; `[^0-9]` matches any character that is not in `[0-9]`.
- A literal `^` may be escaped as `\^`.
- If not the first item, `^` has no special meaning and may appear escaped (with a backslash) or unescaped.
	In other words, `\^` may appear in any position to unambiguously indicate a literal `^`.
- The negation of an empty range matches any character, and is equivalent to the term `.`.
- An empty range matches no characters, and is (for the purposes of Log Surgeon) invalid.
- Character ranges are parsed optimistically; if a `symbol` is followed by a dash (`-`) and another `symbol`, it is treated as a range.
	In particular:
	- Ranges are not chained; `[0-5-9]` is interpreted as 3 `bracketed_item`s: the range `0-5`, a literal `-`, and a literal `9`.
	- If a dash is encountered first, it is interpreted as a literal dash; e.g. in the previous example, or in `[-a]` or `[^-a]`.
	- A dash at the end is also a literal dash; e.g. in `[a-]`.
	- A dash may appear escaped in any position; e.g. `[a\-z]` is 3 literal characters: `a`, `-`, and `z`.
- As shorthand, `\d`, `\w`, and `\s` may appear inside bracketed ranges with limitations;
	they may not be used with negation or as a range endpoint, to avoid potential ambiguity.
	For example, `[\w_-]` is equivalent to `[a-zA-Z0-9_-]`.

[parsing]: parsing.md
[regex-pattern-syntax]: #regex-pattern-syntax
[escape-characters]: #escape-characters
[bracketed-ranges]: #bracketed-ranges
[ebnf]: https://en.wikipedia.org/wiki/Extended_Backus%E2%80%93Naur_form
