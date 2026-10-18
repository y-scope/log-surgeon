## Leftmost-Longest Lexing
Status: design A implemented 2026-10-18; see Implementation Results.
Design B (split DFAs) remains an open performance follow-up.

This document records the evaluation of moving the [`Lexer`][lexer] from its previous
"attempt at a position, else glob to the next delimiter" behaviour
to grep-style leftmost-longest search over the same compiled DFA,
the plan for implementing it, and the results.
"Current Behaviour" below describes the engine *before* this change.

### Index
- Motivation and Target Semantics
- Current Behaviour
- Experiment: Fully Anchored Spec
- Design Space
- Chosen Design
- Plan
- Implementation Results
- Open Decisions
- Appendix: Running the Probes Safely

### Motivation and Target Semantics
From any starting position, the lexer should return the root rule match with:

1. the earliest starting position;
2. among matches from that position, the longest lexeme;
3. among matches of that length, the highest priority
    (integer priority, then earliest in the parsing specification).

This is the disambiguation used by POSIX `grep`/`lex`,
and it makes unanchored rules behave as a user would expect:
an occurrence anywhere in the text is found, not only one that begins at a delimiter.

### Current Behaviour
[`Lexer::next_token`][lexer] tries the main DFA at the current position.
If nothing matches, it emits the text up to and including the next delimiter as static text,
and tries again from there.
Consequently a rule can only match starting at the current position or just after a delimiter.

Two consequences follow,
and the second one is the key observation of this document:

- Unanchored rules behave neither as "find anywhere" nor as "anchored";
    they match only at delimiter boundaries, but may stop in the middle of a token.
    This produces truncated and substring matches (see the experiment below).
- **For a rule anchored with `^`, the current engine is already leftmost-longest.**
    An anchored rule can only start at a delimiter boundary (or input start),
    and those are exactly the positions the current engine attempts.
    So a spec whose rules are all `^`-anchored has identical output under both engines,
    which gives us an exact regression baseline for the new engine.

### Experiment: Fully Anchored Spec
`spec4.txt` is `spec3.txt` with `^` and `$` added to every root rule
(82 of 96 rules were unanchored; the rest already had both anchors),
with three exceptions described below.
Both were run over `logs/hadoop-reservoir-700k-seed42.log` (700k events)
with the current engine.

| | spec3 | spec4 |
|---|---|---|
| DFA states (after minimization) | 5045 | ~3300 |
| determinize + canonicalize | 24 s + 2.7 s | 24 s + 3.0 s |
| serialized cache | 4.8 MB | 3.2 MB |
| parse (cached spec) | 1.67 s | 1.64 s |
| total matches | 17,967,973 | 17,953,889 |
| events that differ | | 13,101 |

Every difference was classified by whether the character before or after the old match
was a delimiter.
All lost matches were one of:

| Rule | Lost | Cause | Verdict |
|---|---|---|---|
| `httpContext` | 11,518 | substring: `datanodeId`, `mapreduce.job` | false positive; keep |
| `rmAppState` | 473 | substring: `FINAL` in `FINALIZED` | false positive; keep |
| `hiveOpCode` | 67 | substring: `FIL` in `FILE`, `FS` in `FSDataset` | false positive; keep |
| `numKVSpaced` | 949 | truncated: `ip = 172` from `172.18.0.4` | truncated; keep |
| `javaClass`, `hadoopClass` | 144 | truncated: `..disk` from `..disk-validator` | truncated; keep |
| `numWithUnit` | 627 | `Wrote 166 bytes.` - sentence-final `.` | **drop `$`** |
| `hostPort` | 158 | `client 172.18.0.17:46508. Number` - same | **drop `$`** |
| `javaHash` | 330 | `Context@4bc222e` - see below | **drop `^`** |
| `objectHash`, `int_fallback` | 240 | displaced by the above | knock-on; neutral |

`javaHash` previously matched only as a continuation of an unanchored `javaClass` match;
in Java's `Class@hash` output the `@` is never preceded by a delimiter,
so `^@...` can never match.

Gains (`int_fallback`, `ipv4Addr`, `hostPort`, `float_fallback`, `objectHash`)
are places where the removed false positives used to consume the text.

#### Partial anchors in spec4
The three rules above are the only ones in `spec4.txt` that are not fully anchored:

- `numWithUnit (0): "^...(?<unit>...)"` - trailing punctuation.
- `hostPort (0): "^(?<host>...):(?<port>[0-9]+)"` - trailing punctuation.
- `javaHash (0): "@(?<javaHash>[0-9a-fA-F]+)$"` - mid-token start.

`.` should **not** be added to the delimiter set to handle the punctuation cases:
it is interior to `prefix.class`, `hostPort`, `blockPoolID.poolIP`, `hadoopClass`,
`javaClass`, `localPath`, `float_fallback`, and all `key_value` rules
(hundreds of thousands of matches), against 785 recovered.
Sentence-final punctuation is a per-rule concern, which is what partial anchors are for.

`javaHash` is the only spec4 rule without `^`,
so it is the only rule that will spawn mid-token search threads in the new engine;
its first character is the literal `@`, so those threads die after one transition.

### Design Space

#### A. Multi-thread simulation of the existing DFA (chosen)
Keep the single combined DFA and simulate it "like an NFA" with a set of threads,
each `(start position, DFA state)`.

- At each position `i`, while no match has been found yet (and `i` has not passed a newline),
    spawn a thread in state `row0[char_before]`.
    Row 0 is the existing lookbehind/anchor transition,
    so a `^`-anchored rule's thread dies immediately at a non-boundary position;
    anchors need no separate handling.
- Decode the character once per position and step every live thread.
- When a thread reaches an accepting state, record `(start, end = i, rule, encoding)`
    and drop every thread with a later start.
    Priority among rules from the same start and length is already resolved
    by kernel order inside the accepting state, as today.
- The implied trailing `\n` lookahead at end of input is applied to every live thread.
- Finish when no thread is alive.
    Longest match for the leftmost start is guaranteed
    because the thread for that start keeps running until it dies.

Two properties keep this cheap:

- **Dominance merging.**
    Two threads in the same DFA state at the same position have identical futures;
    keep only the earlier start.
    This bounds live threads by the number of DFA states,
    and in practice by a handful.
    Implemented with a `last_seen[state] = position` stamp array,
    checked after each step and after each spawn (merging must happen *after*
    the lookbehind transition, so it never conflates anchored and unanchored starts).
- **Single-thread fast path.**
    When one thread is alive and `row0[previous char] == 0`
    (no rule can start here), the loop is today's loop plus one extra load per byte.
    For a fully anchored spec this is the steady state,
    so parity with the current 1.64 s is expected.

Cost is driven only by rules without `^` whose first character class is wide
(e.g. an unanchored `objectHash` or `hdfsPath`);
rules with a literal first character die at once.

Checked for flaws:
lookbehind plus merging is sound;
encodings ride along in `accepting_rule`;
UTF-8 is decoded once per position;
minimization makes merging more effective, not less;
the `\n`-bounded spawn window preserves the current log event separation
(a match may still *span* newlines, e.g. `stackTrace`, since existing threads keep running).

#### B. Split anchored and unanchored DFAs
Run the anchored DFA single-threaded at delimiter boundaries only,
and a (smaller) unanchored DFA multi-threaded everywhere, then reconcile candidates.

- Pros: smaller tables, fewer live threads.
- Cons: two candidate streams to merge; two tables competing for cache;
    rules with only one anchor force the split to be by `anchor_before`.

Design A already gets "spawn only at boundaries" behaviour from row 0,
so B is a measured optimization to try if A regresses, not a design pillar.

#### C. RE2/Hyperscan-style `.*R` forward scan plus reverse DFA
Finds the leftmost *end*; needs a reverse pass for the start and a third for longest.
Prefixing ~100 rules with `.*` risks determinization blow-up
(determinization already takes 24 s and 3-6 GB),
and per-token reverse scans make a tokenizer roughly quadratic per line.
Hyperscan's start-of-match mode is closest in spirit to A,
but is leftmost-end rather than longest and has no lookaround.
Rejected.

#### D. Anchor by default in the spec language
A breaking change to the file format;
alternatives are a spec-level option, or explicit syntax for "unanchored".
Out of scope for the engine work; see Open Decisions.

### Chosen Design
Design A.
API sketch (names are provisional):

```rust
// src/dfa/flattened.rs
pub struct LeftmostMatch {
	pub start: usize,
	pub end: usize,
	pub rule_idx: RuleIdx,
	pub maybe_encoding_idx: Option<EncodingIdx>,
}

impl FlattenedDfa {
	/// Leftmost-longest search in `input[pos..]`, spawning threads at every position
	/// up to and including the first `\n` at or after `pos`.
	/// `char_before` is the character preceding `pos` (or `\n` at input start).
	pub fn execute_leftmost(
		&self,
		input: &[u8],
		pos: usize,
		char_before: u32,
		scratch: &mut LeftmostScratch,
	) -> Option<LeftmostMatch>;
}
```

`Lexer::next_token` becomes:

1. If a pending match is stashed, return it as `Token::Variable` and clear it.
2. Otherwise call `execute_leftmost`.
    If it returns a match starting at `*pos`, return it directly.
    If it starts later, stash it, and return the gap `[*pos, start)` as static text
    (split on `\n` so `Token::Newline` is still emitted for event separation).
    If it returns nothing, emit the rest of the line as static text / newline as today.
3. `Lexer::reset` clears the stash; `Parser::reset` calls it.

The static text glob and `is_delimiter` in the lexer go away;
nothing downstream consumes delimiter-split static text.
Whether the gap is emitted as one token or several is an implementation convenience,
because `Parser` only records `Variable`, `Newline`, and `EndOfInput`.

### Plan

1. **Baseline.**
    Promote the current-engine output for `spec4.cached.txt` to a git-ignored
    `tests/local_spec4.rs` + `tests/local_spec4.txt` regression,
    mirroring `tests/local_hive24hr.rs`.
    The new engine must reproduce it byte for byte.
2. **Reference implementation** (test only, on `Tdfa`).
    Brute-force leftmost-longest by calling `execute_without_captures` from every start,
    for differential tests against the real implementation.
3. **`FlattenedDfa::execute_leftmost`.**
    Threads in a `Vec` sorted by start; stamp-array dedup; truncate on accept;
    implied `\n` at end of input; single-thread fast path.
    Keep `execute` for existing tests and as the fast path body.
4. **Lexer and parser.**
    Rewrite `Lexer::next_token` as above; add `Lexer::reset`; call it from `Parser::reset`.
    Delete the glob/delimiter code from the lexer
    (`ParsingSpec::ascii_delimiters`/`non_ascii_delimiters` may become unused).
5. **Tests.**
    - Unit: unanchored rule matching mid-token; same-start ties by length then priority;
        `$`-only and `^`-only rules; UTF-8 lookbehind; end-of-input lookahead;
        a match spanning a newline (`stackTrace`); `header` after a newline;
        empty gap vs. multi-line gap.
    - Randomized differential against the reference from step 2 on small specs.
    - `spec4` equality with the baseline from step 1.
    - Eyeball the spec3 old-vs-new diff; only unanchored rules should change.
6. **Performance.**
    Compare spec4 parse time before/after; `perf record` if the regression exceeds ~10%;
    only then prototype design B.
7. **Documentation.**
    Rewrite "Matching Root Rules" in [`parsing.md`][parsing] (drop the WIP note),
    and add anchoring guidance plus the cost model to
    [`parsing-specification.md`][parsing-spec]:
    anchor unless there is a reason not to; an unanchored rule costs in proportion to
    the width of its first character class.

### Implementation Results
All seven plan steps are done.

- `FlattenedDfa::execute_leftmost` / `LeftmostScratch` in `src/dfa/flattened.rs`;
    lexer rewrite in `src/lexer.rs` (pending match + gap chunking, `Lexer::reset`).
    The delimiter glob and `ParsingSpec::{ascii,non_ascii}_delimiters` are removed.
- A position at which a rule starts may itself be a mid-line `'\n'`
    (only reachable by a rule whose pattern begins with `\n`);
    the old engine never attempted there. Considered degenerate; not special-cased.
- Tests: `src/dfa/test.rs` (`leftmost_*`, including a randomized differential
    against a brute-force reference over the unflattened TDFA),
    and `src/lexer/test.rs` (token streams).

#### Regressions
- spec4 (`tests/local_spec4.rs`): identical to the old engine
    except 3 of 700k events, all `ServletHandler@<hex>`,
    which now match `javaHash` (starting at `@`) instead of `objectHash` (starting one later).
    That is the intended leftmost behaviour for spec4's one start-unanchored rule;
    the reference was regenerated with it.
- spec3 (`tests/local_hive24hr.rs`): 6,188 of 700k events differ, all from unanchored rules
    now matching mid-token, e.g. `node` in `Datanode`, `FS` in `HDFS_READ`,
    `DataNode` in `srcDataNode=`, and `port = 7220` in `tag.port = 7220`.
    The reference was regenerated after review.

#### Performance
Parse-only, cached specs, 700k events (194 MB); old engine vs. new:

| | old | new |
|---|---|---|
| spec4 | 1.71 s | 2.71 s |
| spec3 | 1.69 s | 4.32 s |

Optimizations applied, in order of effect:
a two-thread path that merges by direct comparison;
a single-word monotonic mark for the position stamps (they were 16 bytes per state);
and fusing a spawn with its first step in a dedicated gap loop,
so spawns that die immediately never touch the thread list.

The remaining cost is structural, not bookkeeping:
a single unanchored rule makes every byte a viable start.
In spec4 that is `javaHash` alone (`@...`):
spawns go from ~15M (delimiters only) to ~180M (every byte), each with a table lookup.
A toy spec showed the same effect at small table sizes
(anchored only: 1.14 s; plus one unanchored `@` rule: 1.28 s);
at spec4's 845 KB transition table the per-byte lookups cost more.
spec3 has many unanchored rules with wide first sets, and averages ~3 live threads.

Options for closing the gap, not yet attempted:

- Design B (split anchored/unanchored DFAs), with a prefilter for the unanchored part,
    e.g. a SIMD byte-set scan for the first characters of the unanchored rules.
- For fully anchored specs, a SIMD scan to the next delimiter in the gap loop.
- Anchoring rules in the spec itself (D), which also fixes precision.

### Open Decisions
- **Default anchoring (D).**
    With leftmost-longest, an unanchored rule means "find anywhere", which is rarely wanted
    and is the only source of extra cost.
    Options: keep opt-in `^...$`; flip the default with explicit opt-out syntax;
    or a per-spec setting.
- **`javaHash`.** Alternatively fold it into `javaClass` as `^...(@(?<hash>[0-9a-fA-F]+))?$`
    instead of leaving it start-unanchored.
- **Sentence-final punctuation.** Partial anchors handle it today;
    a smarter `$` ("followed by a delimiter, or punctuation then a delimiter")
    would be a spec-language change and is not proposed here.

### Appendix: Running the Probes Safely
Building the main DFA for spec3/spec4 from source takes ~27 s and peaks between 3 and 6 GB.
Running two builds concurrently (e.g. `cargo test` with two probe tests in one binary)
OOM-killed the machine repeatedly.
A plain `cargo test` also runs every local test binary concurrently
(each loading the 194 MB log), which OOM-killed it too.
Run heavy tests one at a time under a cgroup cap:

```bash
systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 -- \
	cargo test --release --test local_spec4 -- --nocapture
```

The cap only helps if that much memory is actually free.
`ulimit -v` is not sufficient on its own
(it caps virtual size, which the allocator overshoots long before RSS matters).
Prefer the cached spec (`spec4.cached.txt`) for anything that does not change the DFA.

[lexer]: ../src/lexer.rs
[parsing]: parsing.md
[parsing-spec]: parsing-specification.md
