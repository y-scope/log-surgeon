## Leftmost-Longest Lexing
Status: design A implemented 2026-10-18 (2.71 s on spec4);
design B implemented same day (1.99 s vs. old-engine 1.78 s on spec4).
See Implementation Results and Design B.

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
- Design B: Prefilters over One Combined DFA
- Open Decisions
- Future Improvements
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

Closing the gap: design B below (implemented; spec4 1.99 s vs. old 1.78 s).
Further options: anchoring rules in the spec itself (D), which also fixes precision;
micro-tuning the per-hit path in `search`.

### Design B: Prefilters over One Combined DFA
Status: implemented 2026-10-18. Target was the spec4 regression (2.71 s vs. 1.71 s);
result: 1.99 s vs. old-engine 1.78 s (parse-only, 700k events, details below).
The original sketch split the DFA in two (anchored/unanchored);
that turned out unnecessary: see "One DFA" below.

The semantics are unchanged: earliest start, then longest, then priority.
Only the way candidates are found changes.
The combined-DFA `execute_leftmost` from design A stays as the reference implementation
and as the fallback.

#### Observation
A root rule either begins with `^` (start-anchored) or not.
A start-anchored rule can only begin at a *boundary*:
`pos` itself when `char_before` is a delimiter, or just after a delimiter.
Finding boundaries is a byte scan over the delimiter set, which SIMD does 16 bytes at a time.
An unanchored rule can begin anywhere, but only at a byte in its *first-byte set*.
For spec4 that set is `{'@'}`; finding the next `@` is the same routine.

#### One DFA
State 0's transitions are *lookbehind* transitions: delimiters for `^`-anchored rules,
any char otherwise. So `execute(&input[b..], delimiter)` from a boundary handles all rules
correctly (the lookbehind admits anchored rules and passes for unanchored ones),
- the startable byte set of the *unanchored* rules is
    `initial_bytes(lookbehind)` for any non-delimiter `lookbehind`,
    directly from the combined DFA.
No second DFA needed. Existing cached spec files stay valid as-is
(cache format unchanged), and build memory is unchanged.

Derived at spec build time (`ParsingSpec`):

- `is_ascii_delimiter: [bool; 0x80]`: fast hit classification.
- `scan: ByteSet`: ASCII delimiters, lead bytes of non-ASCII delimiters
    (confirmed by decoding at a hit), and -- when the unanchored set is usable --
    the unanchored first bytes.
- `unanchored_scan: Option<ByteSet>`: the unanchored first-byte set,
    `Some` only when exact (nibble construction did not merge rows)
    and sparse enough (len <= 64 today; a scan stopping at most bytes loses to
    the general engine). `None` with `has_unanchored_rules` means fallback.
- `has_unanchored_rules: bool`.

#### Search (`Lexer::next_token` -> `search`)
One fused ascending scan per search, within the newline window:

1. Probe `pos` if `char_before` is a delimiter (boundary).
2. Scan hits, each classified by byte:
    - ASCII delimiter: probe the position after it if unseen (`last_probed` dedup);
        if it was `'
'`, the window is done (its tail position was probed).
        If the delimiter byte is also an unanchored first byte
        (`unanchored_scan.contains(byte)`, exact), probe the delimiter position first
        (e.g. spec4's `@...` rule, since `@` is itself a delimiter).
    - Lead byte of a non-ASCII delimiter: decode and confirm, probe the position after.
    - Otherwise: an unanchored-rule start; probe it.
3. A probe is a single-threaded `execute` on the combined DFA
    (longest match, then priority, for that start);
    probes run in ascending order, so the first match found is the leftmost.
4. No hit at all: the rest of the window is static text, as before.

#### `src/byte_scan.rs`
`ByteSet`: `Eq` (broadcast + `simd_eq`, <= 4 bytes) or `Nibbles`
(SSSE3-style `shuffle_zeroing` table pair: `b` matches iff `lo[b & 0xF]` and
`hi[b >> 4]` share a bit; rows with distinct 16-bit low-nibble masks are *classes*;
with more than 8 distinct masks, classes are merged and membership over-approximates,
checked with `ByteSet::is_exact`).
`find_in` returns the first hit via `to_bitmask().trailing_zeros()`;
the < 16-byte tail is scalar (delimiters are ~13 bytes apart at spec4's rate,
so most calls are the tail anyway).

#### Results (parse-only, 700k events, `hadoop-reservoir-700k-seed42.log`):
old engine 1.78 s; design A 2.71 s; B naive (two scans per segment) 2.11 s;
B fused (one scan) 1.96-2.01 s; final 1.99 s.
The remaining ~0.2 s over the old engine is per-hit work in `search`
plus `execute` call setup at ~15M boundaries;
`perf` attributes `execute` 29 %, `find_in` 12 %, capture machinery 35 %
(shared with every engine).
spec3 takes the fallback (4.3 s, same as A) --
its unanchored first-byte set is dense.

Bug found and fixed along the way: a delimiter byte that is also an unanchored
first byte must probe both the position after it and the position itself
(caught by `tests/local_spec4`: `ServletHandler@<hex>` lost its `javaHash` match).

#### Measured byte sets and counts
(spec4/spec3 over 98 rules each, 31 ASCII delimiters,
`hadoop-reservoir-700k-seed42.log`, 194 MB, 1.3M newlines, 700k events;
scan-set sizes here count bytes, non-ASCII via lead bytes.)

| | spec4 | spec3 |
|---|---|---|
| rules without `^` | 1 (`javaHash`) | 82 |
| unanchored first-byte set | `{'@'}` | 69 bytes: `-./0-9<@A-Z[_a-z` |
| scan path | fused (exact) | fallback (`execute_leftmost`) |
| delimiter hits | 14,658,814 | n/a |
| non-delimiter first-byte hits | 0 (every `@` is itself a delimiter) | n/a |
| multi-byte lead-byte hits | 0 | n/a |
| `execute` probes | 16,134,766 (= 14.7M boundaries + pos probes + at-hit probes) | n/a |
| accepts | 6,502,869 | n/a |

Two observations:

- `@` occurs somewhere in only 642 of 700k events, so the first-byte side of the
    fused scan never fires on its own; every `javaHash` match comes through the
    delimiter-hit arm, via the dual-probe fix below.
- spec3's unanchored rules start on nearly half of all bytes;
    the prefilter would stop at most positions, so it falls back instead.
    The `<= 64` cut-off is what puts spec3 on the fallback path.

#### Walkthrough: a delimiter byte that is also an unanchored first byte
A real `local_spec4` failure, from the 2026-10-18 design-B draft.
The event text contains `Container ServletHandler@456d6c1e + safety as filter`.
Relevant spec4 rules (priority order):

- `objectHash (0)`: a long `^[hex]{2,}$`-style rule (hex words at a boundary);
- `javaHash (0)`: `@(?<javaHash>[0-9a-fA-F]+)$` (unanchored: starts on `@` itself);
- `@` is one of the 31 delimiters.

Leftmost-longest for this stretch expects `ServletHandler@456d6c1e` to match
`javaHash` starting *at* the `@` (start position `p`), not `objectHash` at `p + 1`:
equal priority, so earliest start wins, and the `@` start is earlier.

Correct algorithm at that scan:

1. Scan from the current position. The `@` is a scan hit (it is in both sets).
2. Hit classification asks first: ASCII delimiter? Yes.
    Because `unanchored_scan.contains('@')` too, **probe `p` itself**:
    `execute(&input[p..], 'r')` -- the lookbehind `r` is not a delimiter,
    so `^`-anchored rules die at once, javaHash's unanchored lookbehind passes,
    and the DFA consumes `@456d6c1e`, then `$` lookahead sees the space.
    Match: `(p, p + 9, javaHash)`. This is the answer; return it.
3. (If step 2 had failed, the algorithm would probe the boundary `p + 1`
    next -- where `objectHash` would have matched `456d6c1e` --
    and only then continue scanning.)

The buggy draft skipped step 2:
it classified the hit as a delimiter, probed only `p + 1`,
and returned `objectHash` starting one byte later.
Any rule whose first byte is a delimiter byte hits this case;
in specs like spec4 (where `@` is both) it is the *only* way `javaHash` can match,
since the fused scan otherwise never reports a non-delimiter first-byte hit.

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

### Future Improvements
- **Store each determinization kernel once.**
    An uncached spec build peaks at 3.1 GB (spec4) / 3.5 GB (spec3),
    nearly all of it kernels: the NFA configuration sets behind the raw DFA states
    (7,611 states and 353k configurations for spec4).
    `Tdfa::lookup_or_add_state` (`src/dfa.rs`) clones each new kernel twice:
    into the state (`DfaState::kernel`) and as the key of the lookup map (`Tdfa::kernels`).
    Sharing one allocation between the two (e.g. `Rc<Kernel>`,
    or an index into the states with the map keyed by the state's kernel)
    should roughly halve the peak.
    The map is also scanned linearly for register bijections on every miss,
    so a change here may be a chance to revisit that lookup too.
    Verify with `tests/local_uncached.rs` (`env SPEC=spec4.txt CACHED=spec4.cached.txt`):
    the rebuilt DFA must stay byte-identical to the cache.
- **Tune the unanchored prefilter cut-off.**
    The 64-byte limit on the unanchored first-byte set (`ParsingSpecBuilder::build`)
    is a guess; spec3's 69-byte set falls just past it.
    Measuring a few specs' hit rates around the cut-off would place it properly.
- **Per-hit overhead in `Lexer::search`.**
    spec4 is still about 10 % behind the pre-leftmost engine
    (1.96 s vs. 1.78 s): per-hit classification plus one `execute` call per delimiter.
- **Bound search time.**
    Memory is already bounded (live threads never exceed the DFA state count),
    but a thread may run arbitrarily far past the newline window;
    a rule like `a.*` matching across lines would make a search scan to the end of input.
    A cap on match length would bound time,
    at the cost of truncating or missing matches longer than the cap.

### Appendix: Running the Probes Safely
Building the main DFA for spec3/spec4 from source takes ~24 s
and peaks at 3.1 GB (spec4) / 3.5 GB (spec3) resident.
Nearly all of it is determinization's kernels
(the NFA configuration sets behind each raw DFA state: 7,611 states and
353k configurations for spec4, stored in the states and again as lookup keys).
Minimization (`partition_states`) itself adds about 8 MB.
`Tdfa::canonicalize` used to also merge every kernel into the minimized states
and re-index them while the raw DFA was alive,
which pushed spec4 past 4 GB (and was what OOM-killed earlier builds);
it now leaves the minimized DFA's kernels empty, since nothing reads them afterwards.
Storing each raw kernel once would roughly halve the remaining peak;
see Future Improvements.
Running two builds concurrently (e.g. `cargo test` with two probe tests in one binary)
OOM-killed the machine repeatedly.
A plain `cargo test` also runs every local test binary concurrently
(each loading the 194 MB log), which OOM-killed it too.
Run heavy tests one at a time under a cgroup cap:

```bash
systemd-run --user --scope -q -p MemoryMax=3G -p MemorySwapMax=0 -- \
	cargo test --release --test local_spec4 -- --nocapture
```

To regenerate a cached spec after the *cache format* changes,
`tests/local_regen_cache.rs` (git-ignored) rebuilds one spec at a time:
`env SPEC=spec4 cargo test --release --test local_regen_cache`.

The cap only helps if that much memory is actually free.
`ulimit -v` is not sufficient on its own
(it caps virtual size, which the allocator overshoots long before RSS matters).
Prefer the cached spec (`spec4.cached.txt`) for anything that does not change the DFA.

[lexer]: ../src/lexer.rs
[parsing]: parsing.md
[parsing-spec]: parsing-specification.md
