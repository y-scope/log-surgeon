## Parsing in Log Surgeon
Log Surgeon parses log data using a [Parsing Specification][parsing-spec].
The parsing specification includes:

- How root rules are matched.
- How subrules are extracted.
- How log events are separated.

The specification is the main entry point: [`ParsingSpec::create_parser`][create-parser] creates a
parser, and there may be many independent parsers per specification.
A parser advances through the input and builds structured log events,
using a lexer to identify matches in the input according to the parsing specification.

### Index
- Matching Root Rules
- Implementation Details
- Submatch Extraction
- Separating Log Events

### Matching Root Rules
The lexer identifies occurrences of root rules defined in the parsing specification.
A root rule match represents a piece of semantically meaningful text identified by the user.

The lexer processes input left to right.
At each step, it searches for the next root rule match ("grep semantics", leftmost-longest):

1. The earliest starting position.
2. Among matches from that position, the longest match.
3. Among matches of that length, the highest-priority (earliest) rule in the spec.

The text before the match is static text.
A match may start anywhere up to and including the position just after the next newline;
matches beginning later are found by subsequent searches.
A match may itself span newlines.

Anchors (`^`, `$`) restrict where a rule may start or end (next to a delimiter);
see [anchors][anchors].
Anchoring is also the main performance lever:
an anchored rule is only attempted after a delimiter,
whereas an unanchored rule is attempted at every position.
See [Leftmost-Longest Lexing][leftmost-longest] for the design and measurements.

#### Implementation Details
To determine if/what rule matches from a single input position,
Log Surgeon builds an [automaton][dfa]
for the combination of all root rule patterns in the parsing specification;
an automaton is just a state machine with transitions based on an input character.
Specifically, Log Surgeon implements the classical regex -> NFA -> DFA construction.

DFAs simulate matching multiple patterns/rules at once,
performing only a single pass through the input.
"Executing" the DFA is a loop that traverses the states:

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

To search for the leftmost-longest match rather than attempt a single position,
the lexer first scans for candidate starts:
delimiter characters (after which anchored rules may start) and,
when the unanchored rules have a sparse first-byte set, those bytes.
Each candidate position is then attempted with a single-threaded run of the same DFA.
When the unanchored rules' first-byte set is dense,
the lexer falls back to simulating the DFA "like an NFA":
an execution thread is started at every position (until a match is found),
threads in the same state are merged (keeping the earliest start),
and once a match is found, threads with a later start are discarded.
See [`Lexer::search`][lexer-search] and
[`FlattenedDfa::execute_leftmost`][execute-leftmost].

Notice that each iteration of the loop
requires looking up the transition for the current state and input character.
Specifically, characters are Unicode code points (with fast lookup for those in the ASCII range).
Additionally, at each step, we check if the current state is an accepting state.
In a sense, this DFA loop is an "interpreter" for instructions "record match" and "goto next state".

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

Note: While it is possible to build a single tagged DFA
for all the rules in the parsing specification,
executing this DFA to determine the root and sub-rule matches at once means
executing all the state transition operations for all the potential rule matches;
i.e. recording many potential sub-rule matches that are discarded.
Therefore, we have found it better to execute a classical DFA to determine which rule,
and a tagged DFA specifically for the matched rule.

### Separating Log Events
While lexing input, to determine log event boundaries, newline characters (not part of a rule match)
and root rules named `header` are treated specially under the following conditions.

A `header`, if preceded by a newline (or at start of input), is a log event separator.
Otherwise, it is treated as an ordinary rule.

If no separating `header` has been encountered (yet), log events are separated on newlines.

Newlines are part of the line preceding it;
in other words, log events are always terminated by newlines
(but a newline doesn't necessarily terminate a log event).

### See Also
Searching log shapes with a search string is a separate subsystem; see
[Searching Log Shapes][search-by-log-shapes] for its design.

### TODO
Explain:
- anchors
- leaf ambiguity

[parsing-spec]: parsing-specification.md
[anchors]: parsing-specification.md#regex-pattern-syntax
[leftmost-longest]: leftmost-longest-lexing.md
[execute-leftmost]: ../src/dfa/flattened.rs
[lexer-search]: ../src/lexer.rs
[create-parser]: ../src/parsing_spec.rs
[search-by-log-shapes]: search-by-log-shapes.md
[python-regex]: https://docs.python.org/3/howto/regex.html
[dfa]: https://en.wikipedia.org/wiki/Deterministic_finite_automaton
[tagged-dfa]: https://arxiv.org/abs/2206.01398
