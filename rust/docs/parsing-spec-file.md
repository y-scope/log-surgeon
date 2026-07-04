## Parsing Specification
A parsing specification is a list of rules used to determine non-static text in logs.
A rule is a `name: "pattern"` pair;
Log Surgeon supports most common regex syntax/semantics (the outside double-quotes are not part of the pattern).
For example, a pattern `[0-9]+` may be used to identify numbers.

In a Parsing Specification File,
rules are listed in priority-order.
These rules are also called root rules;
"regex captures" in a pattern define subrules as `(?<sub_rule_name>sub_rule_pattern)`, arbitrarily nested.
Log Surgeon extracts both root rules and subrules when parsing;
the extracted values are called matches.

Note:
We generally avoid "capture" terminology to avoid ambiguity between the root rule match and sub-match extraction,
which require different implementations internally.
From the outside, Log Surgeon matches and stores both root rule matches and subrule regex captures alike,
so we simply refer to them as "rules", "patterns", and "matches".

A (sub)rule with no child subrules is called a leaf rule;
a root rule whose pattern has no regex captures is also a leaf rule.
For example, the root rule `email: "(?<user>\w+)@(?<hostname>((?<subdomain>\w+)\.)*(?<domain>\w+)\.(?<tld>\w+))"`
has leaf rules `user`, `subdomain`, `domain`, and `tld`.

The fully qualified name of a rule starts with its root rule name and is followed by any/all subrule names,
separated by periods, e.g. `foo.bar.baz`.

A name denotes the "type" of the matched text,
and the same name/type may have multiple rules/patterns.

A parsing specification also contains a set of delimiter characters,
which are additionally used to differentiate between static and non-static text.

### File Format
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

- individual characters/character sets, e.g. `.` (any single character), `a`, `[a-z]`, or `[a-z0-9ABC]`
- parenthesized expressions/subrules, e.g. `(hello)` or `(?<greeting>hello world)`

Operators are, from highest to lowest precedence (and always left-associative):

- postfix repetition, e.g. `a*` (0 or more), `a+` (1 or more), `a?` (0 or 1),
	`a{n}` (exactly `n`), and `a{min,max}` (at least `min`, up to and including `max` times)
- binary concatenation, e.g. `a*b`, equivalent to `(a*)b`
- binary alternation ("or"), e.g. `a*b|c`, equivalent to `((a*)b)|c`

Furthermore, a root rule's pattern may be anchored with a leading `^` or ending `$`,
meaning the match must be preceded/followed by one of the delimiter characters
(or the very start or end of input).

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
