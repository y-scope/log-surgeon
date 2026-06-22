## Parsing in Log Surgeon
An overview of the title.

### Parsing Specification
A parsing specification is used to determine non-static text in logs,
determined by regex patterns.
For example, a pattern `[0-9]+` may be used to identify numbers.
Log Surgeon supports common regex syntax; the exact syntax is specified [below](#regex-pattern-syntax).

A rule is a `name: "pattern"` pair, and rules are given in priority-order for a parsing specification.
These rules are also called root rules;
a pattern may also define sub-rules as `(?<sub_rule_name>sub_rule_pattern)`, arbitrarily nested.
A (sub)rule with no child sub-rules is called a leaf rule;
a root rule may also be a leaf rule.
The fully qualified name of a rule starts with its root rule name and is followed by any/all sub-rule names,
separated by periods, e.g. `foo.bar.baz`.

Note: We generally avoid "capture" terminology to avoid ambiguity between
top-level pattern matching and sub-match extraction (i.e. captures within a regex pattern),
which require very different implementations internally.
From the outside, Log Surgeon matches and stores both root rule matches and sub-rule regex captures alike,
so we simply refer to them as "rules", "patterns", and "matches".

A name denotes the "type" of the matched text,
and the same name/type may have multiple rules/patterns.
For more details on matching priority, see [below](#matching-root-rules).

A parsing specification also contains a set of delimiter characters,
which are additionally used to differentiate between static and non-static text.

### Matching Root Rules
A lexer processes input left to right and reports root rule matches.

At each step, if possible, the lexer takes the longest possible match of any root rule.
If multiple root rules match with the same length, the highest-priority (earliest) rule is returned.

If no root rules match at the current position,
the parser seeks to the first delimiter character after the current position,
and repeats attempting to match a root rule _after_ the delimiter.

WIP: We hope to generalize root rule matching to find the earliest possible occurrence of a root rule at each step.

#### Implementation Details
To determine if/what rule matches from an input position,
Log Surgeon builds an [automaton][dfa] for the combination of all root rule patterns in the parsing specification;
an automaton is just a state machine with transitions based on an input character.
Specifically, Log Surgeon implements the classical regex -> NFA -> DFA construction.

DFAs simulate matching multiple patterns/rules at once with just a single pass through the input.
"Executing" the DFA is just a loop that traverses the states:

```rust
let mut current_state: usize = 0;
let mut maybe_match: Option<(Rule, usize)> = None;
for (pos, ch) in input.char_indices() {
	if let Some(next_state) = states[current_state].lookup_transition(ch) {
		current_state = next_state;
		if let Some(rule) = states[current_state].accepting_rule {
			// Save the match, but don't stop yet; see if we can match longer.
			maybe_match = Some((rule, pos + ch.len_utf8()));
		}
	} else {
		break;
	}
}
return maybe_match;
```

Note: The theoretical complexity of determining whether input matches
is very different from finding the longest possible match;
longest match semantics inherently requires "looking ahead" to attempt a longer match,
even if the extra input does not result in a match.
In practice, the non-static text of logs is small compared to the static text,
and confirming the longest possible match for rules in a parsing specification
rarely require consuming much extra input.

Notice that each iteration of the loop requires looking up the transition for the current state and input character.
In fact, transitions for a state are stored as a list of (non-overlapping) Unicode code point intervals,
so this lookup means comparing the input character against these intervals.
Further, at each step, we need to check if the current state is an accepting state.
In a sense, this DFA loop is an "interpreter" for instructions "goto next state" and "record match".

To speed up matching, we compile DFAs into native functions.
Currently, Log Surgeon just in time (JIT) compiles them for ease of deployment,
but conceptually the DFAs are the same as if they were "ahead of time" compiled.

Each state in the DFA will be compiled to a short sequence of basic blocks
that looks like/roughly corresponds to the following pseudocode:

```
state10:
ch = read_next_character();
record_match(); // Only if this state is actually an accepting state.
if ('a' <= ch) && (ch <= 'z') {
	goto state20;
} else if ('0' <= ch) && (ch <= '9') {
	goto state30;
} else if ch == '_' {
	goto state40;
// etc.
} else {
	goto done;
}
```

When executing this code, the current state is naturally encoded by the CPU's instruction pointer
(pointing to the compiled instructions for the state).
Because we know everything about the states when compiling the DFA,
all the information is baked into the native instructions;
instead of looking up the list of transitions/intervals for the current state (as in the loop above),
the instructions for the state contain exactly the comparisons to choose the next state.
Also, instead of checking if a state is accepting during execution (as in the loop above),
we simply don't emit `record_match()` for a state if it isn't an accepting state.

##### Submatch Extraction
The classical DFA execution determines which rule matches;
once the rule and matched text is known,
Log Surgeon uses a [tagged DFA][tagged-dfa] for the specific rule to extract sub-rule matches.
Conceptually, a tagged DFA is just a DFA with operations to execute on state transitions;
in this case, the operations record sub-rule match positions.
Compared to the pseudocode for a classical DFA above,
the inner loop just changes by:

```rust
if let Some((next_state, operations)) = states[current_state].lookup_transition(ch) {
	execute_operations(operations);
	// Rest is the same.
	current_state = next_state;
	if let Some(rule) = states[current_state].accepting_rule {
		maybe_match = Some((rule, pos + ch.len_utf8()));
	}
} else {
	break;
}
```

Note: While it is possible to build a single tagged DFA for all the rules in the parsing specification,
executing this DFA to determine the root and sub-rule matches at once means
executing all the state transition operations for all the potential rule matches;
i.e. recording many potential sub-rule matches that are discarded.
Therefore, we have found it better to execute a classical DFA to determine which rule,
and a tagged DFA specifically for the matched rule.

#### Future Work
Many search tools and algorithms have optimizations for fixed strings, i.e. static text.
However, Log Surgeon inherently works with patterns for non-static text;
in fact, it needs to identify matches among many different patterns of non-static text,
so we have not found meaningful opportunity for these optimizations.

Currently, Log Surgeon does not report the earliest possible match for each rule;
heuristically, it makes use of delimiter characters to achieve similar results.

There are a few known ways to implement earliest match semantics
(sometimes referred to as "leftmost-longest",
though "leftmost" also means something different in an overlapping context,
so we avoid that terminology).

Theoretically, the best option is a reverse scan for potential starting positions:

1. Construct a DFA for the reversed patterns, prefixed with `.*`.
2. In a single pass from the end to start of the file, mark the position at each accepting state.

Each of the marked positions necessarily corresponds to a valid start position for the "forwards" DFA.
So, starting from the first possible position, we simply run the forwards DFA to determine the rule match,
and the tagged DFA to determine the sub-rule matches as before.
For the next root match, we continue at the first starting position after our previous match ends.

The downsides of this approach are:
- This approach necessarily requires reading from the end of the file.
- Reading in reverse is suboptimal compared to only reading forwards.
- Fully determinizing a DFA for a `.*`-prefixed regex is costly, though this can be worked around by lazy construction.
- Storing all starting positions is not necessarily prohibitive (e.g. using a bitmap), but not trivial either.

### Separating Log Events
While lexing input, to determine log event boundaries, newline characters (not part of a rule match)
and root rules named `header` are treated specially under the following conditions.

A `header`, if preceded by a newline (or at start of input), is a log event separator.
Otherwise, it is treated as an ordinary rule.

If no separating `header` has been encountered (yet), log events are separated on newlines.

Newlines are part of the line preceding it;
in other words, log events are always terminated by newlines
(but a newline doesn't necessarily terminate a log event).

### Regex Pattern Syntax
Regexes are defined recursively; one can think of them as expressions composed of terms and operators.
Terms are:

- individual characters/character sets, e.g. `.` (any single character), `a`, `[a-z]`, or `[a-z0-9ABC]`
- parenthesized expressions/sub-rules, e.g. `(hello)` or `(?<greeting>hello world)`

Operators are, from highest to lowest precedence (always left-associative):

- postfix repetition, e.g. `a*` (0 or more), `a+` (1 or more), `a?` (0 or 1),
	`a{n}` (exactly `n`), and `a{min,max}` (at least `min`, up to and including `max` times)
- binary concatenation, e.g. `a*b`, equivalent to `(a*)b`
- binary alternation ("or"), e.g. `a*b|c`, equivalent to `((a*)b)|c`

More explicitly, pattern syntax follows this [EBNF][ebnf] grammar.

```
// A top-level pattern may be "anchored".
top_level_pattern: "^"? alternation "$"?

alternation: sequence ("|" sequence)*

sequence: suffixed_term+

suffixed_term: term repetition_suffix?

term:
	"."
	bracketed_ranges
	symbol
	"(" alternation ")"
	"(?<" name ">" alternation ")" // A sub-rule.

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

### TODO
Explain:
- anchors
- leaf ambiguity

[ebnf]: https://en.wikipedia.org/wiki/Extended_Backus%E2%80%93Naur_form
[python-regex]: https://docs.python.org/3/howto/regex.html
[dfa]: https://en.wikipedia.org/wiki/Deterministic_finite_automaton
[tagged-dfa]: https://arxiv.org/abs/2206.01398
