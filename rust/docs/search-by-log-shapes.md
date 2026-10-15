## Searching Log Shapes

A **log shape** is a template for a set of log messages:
a sequence of fragments, where a fragment is either

- static text, e.g. `Scheduling blk_`, or
- a rule reference, e.g. `%blockID.blockNum%`.

A **search string** is a query using literal text and `*` globs, e.g. `*blk_1073746491_5667*`.

[`SearchString::search_by_log_shapes`][search-by-log-shapes] answers:
for each shape, **which interpretations of the query does that shape admit?**
An interpretation attributes each literal character of the search string
to a fragment of the log shape.
Notably, a "run" of literal search text may span multiple fragments of a log shape,
and globs may always span anything in between.
It is the same decomposition a user would perform by hand to understand why their query matched.

This document covers the implementation details of search;
users should see the [parsing specification][parsing-spec] for user-facing concepts.

### Index
- Public API
- Query Semantics
    - Anchoring
    - Empty captures
- Architecture
- Shape Models
    - Ambiguous Names
    - Supported Shapes
    - Rule matching
- Tier 1: Rejection
- Tier 2: Composition
    - Placement
    - Composition
    - Rendering
    - Budgets
- Tier 3: The Engine
    - Covering
- Shape Truncation
- Correctness
- Testing
- Complexity
- Known Limitations
- Related Code

### Public API

```rust
pub fn search_by_log_shapes(&self, spec: &ParsingSpec, log_shapes: &[&str])
    -> Vec<Vec<Interpretation>>
```

The result is **positional**: `result[i]` holds the interpretations for `log_shapes[i]`.
Each inner vector is a canonical, duplicate-free set.

Shape models are cached in the spec's [`ShapeModelCache`][shape-model-cache],
because building a shape's model walks and resolves the whole shape
and callers typically search the same shapes for many queries;
searching the same shape twice reuses its model.
A spec is the long-lived owner (`ParsingSpec::shape_models`),
so the cache is shared by every [`Parser`][parser] created from it,
and by direct searches on the spec.

Two further methods expose the engine directly and exist for tests, not users:

- `interpretations_for_log_shape_via_engine` -- bypasses `decompose` entirely,
    so the [differential test][differential] can pin the fast paths against the engine they stand in
    for.
- `interpretations_for_automata` -- takes an already-built automaton plus the shape parts
    the automaton is a truncation of, so the [truncation test][narrowing] can compare
    a truncated shape against the full one.

### Query Semantics

A query is created by [`SearchString::parse`][search-parse] into a `Vec<SymbolicChar>`,
where a symbol is either a literal character or `GlobStar` (`*`).
`*` and `\` are the only metacharacters; `\*` and `\\` are the way to write them literally.

The parsed form is **canonical**: adjacent wildcards are collapsed, so `a**b` is `a*b`.
`**` says exactly what `*` does, and establishing the invariant once at parse
means nothing downstream has to re-derive it --
the prefilter's transitions, `runs_of`, and the anchoring predicates below all rely on it.

#### Anchoring

A wildcard is a symbol like any other: the automata consume it as `.*`.
Every match therefore runs from the start of the message to its end,
and "unanchored" only means a `*` is there to absorb the slack.
Anchoring is not a mode; it is a **predicate read off the query**,
the same way at both ends:

> A query is anchored at a boundary exactly when it does not have a wildcard there.

| Query | Start anchored | End anchored | Meaning |
| --- | --- | --- | --- |
| `*foo*` | no | no | contains `foo` |
| `foo*` | yes | no | starts with `foo` |
| `*foo` | no | yes | ends with `foo` |
| `foo` | yes | yes | is exactly `foo` |
| `*` | no | no | anything |
| `` | yes | yes | the empty message |

These are [`SearchString::anchored_start`][search-anchored] / `anchored_end`.
Nothing is stripped from the query and nothing is restored later;
the engine, the prefilter, and composition all see the symbols exactly as parsed.
The predicates are consulted only where a decision genuinely depends on them:

- `truncated_automata` -- an end-anchored query must reach the shape's end, so nothing may be cut;
- `compose` -- an end-anchored query pins trailing nullable parts to the empty string,
    which rendering emits as *empty captures*
    (the predicate comes from the `Query`, not from its runs:
    the empty query has no runs at all, yet is anchored);
- the unanchored-tail rule -- only an unanchored query leaves the tail unconstrained;
- `is_obviously_not_ruled_out` -- an end-anchored query must be able to *finish* at the variable.

The cost of a trailing `.*`,
which must traverse whatever remains of the shape,
is controlled by **not building the rest of the shape**,
rather than by short-circuiting the automaton.
See [Shape Truncation](#shape-truncation).
Deciding *how much shape matters* belongs to placement,
which knows where the query's text can sit,
not to the intersection, which would otherwise have to guess.

> **History.** Three hacks lived here.
> First, the engine treated a missing trailing wildcard as if it were present,
> making `foo` and `foo*` equivalent;
> the uniform rule above replaced that.
> Second, the trailing wildcard was *simulated* by `Tnfa::intersect`'s `TO_END` const parameter,
> which made the intersection accept as soon as the query's automaton did,
> and `compute_paths`'s `WILDCARD_END`,
> which then glued a `*` onto the end of each path.
> Third, after those were removed the trailing wildcard was still *stripped* into an
> `AnchoredQuery { view, anchored_end }` and threaded as a flag through `can_match`,
> the alignment walk, and the engine, which put it back as a `.*` when building the query's
> automaton -- a round trip. Along the way `**` was collapsed in the walk but not elsewhere,
> `*` alone tripped an assertion on the engine path,
> and the empty query slipped past `compose`'s nullable-tail check because it has no runs.
> All of that is gone: the query is canonical from `parse`, and anchoring is a predicate.
> See `tests/search_anchoring.rs`.

#### Empty captures

An anchored query can pin a rule to producing *nothing*.
If a shape references a nullable sub-rule via `%foo.leaf%`
and the query is `x` against shape `x%foo.leaf%`,
the correct interpretation contains `(?<leaf>)` -- an empty capture.
This is more precise than reporting `*`, which would claim the rule could have produced anything.

The engine reports empty captures directly.
Note that the spec builder *rejects a whole rule that can match empty*
(`RegexErrorKind::NullableExpression`),
so this arises from nullable **captures inside** a rule, not from nullable root rules.

For an end-anchored query,
extra care must be taken to avoid losing this information in light of shape truncation.
Start-anchored queries do not have this issue as simulation always begins from the very start;
there is no prefix-truncation.

The empty query is the extreme case: anchored at both ends with no literal text,
it pins *every* variable to the empty string and matches only a shape that can produce nothing.
Composition has no runs to place; it renders the shape as a row of empty captures
(a shape with static text cannot vanish, so such a shape is rejected outright).

### Architecture

There are 3 "tiers" of decomposition, from cheapest to most expensive and complete.
Tiers 1 and 2 live in [`search::decompose`][decompose],
Tier 3 lives in [`nfa::search_decomposition`][engine].

```
                      +-----------------------+
   per query -------->|  Query{symbols, runs,    |  once, reused for every shape
                      |  anchored_*}            |
                      |  RunFitCache          |
                      +-----------+-----------+
                                  |
    per shape ---------------------v-----------------------------------------
                       +-----------------------+
                       | ShapeModel            |  parts + charsets + nullable flags
                       |  (cached; asserts     |  panics if a variable is
                       |   supported shape)    |  undefined or non-leaf
                       +-----------+-----------+
                                   |
                 +----------------v----------------+
        Tier 1   | can_match?  ---- no --> REJECT  |  O(atoms x query), usually O(variables)
                 +----------------+----------------+
                                  | yes
                 +----------------v----------------+
        Tier 2   | PlacementTable + compose        |
                 |  Impossible ------------------> |  answer: no match
                 |  Compositions ----------------> |  answer: interpretations
                 |  Unknown -----------+           |
                 +---------------------+-----------+
                                       |
                 +---------------------v-----------+
        Tier 3   | engine: intersect TNFA          |  full or truncated shape
                 +---------------------------------+
```

Two pieces of per-query state are computed once and shared across all shapes:

- **`Query`** -- the symbols exactly as parsed, the runs split out of them,
    and the anchoring predicates, read off the symbols once.
    A *run* is a maximal stretch of literal characters,
    carrying `anchored_start`/`anchored_end`.
    Only the first run can be start-anchored and only the last end-anchored;
    the predicates live on the query rather than being re-derived from its runs,
    because `*` and the empty query both have no runs yet differ in anchoring.
- **RunFitCache** -- see below.

### Shape Models

[`ShapeModel`][shape-model] is a coarse, allocation-light model of a shape,
built by [`ParsingSpec::split_log_shape`][split-log-shape] --
the same tokenizer the automaton builder uses,
so the model and the automaton can never disagree about where the variables are.
It is a sequence of [`ShapePart`][shape-part]s:

- `Static(String)` -- text a match must reproduce verbatim. Never empty.
- `Variable(Variable)` -- a rule reference, carrying:
    - `name` -- the rule name as written in the shape, used to label the captures it produces.
    - `charset` -- a **superset** of the characters any match of the rule can contain (see
        [`Charset`][charset]).
    - `alternatives` -- every row `rules_for_name` resolves the name to;
        see [Ambiguous names](#ambiguous-names).
    - `can_match_empty` -- whether some alternative is nullable.

#### Ambiguous Names

A shape variable's name can resolve to several rows of `ParsingSpec::rules_for_name`:
several root rules may share a name
(`%num%` with `num = [0-9]+` and `num = 0x[0-9a-f]+`),
or a capture name may occur in several places of one rule
(`%foo.bar%` with `(?<bar>[a-z]+)|(?<bar>[0-9]+)`).

*Feasibility* is decided over all of them:
`charset` is their union, `can_match_empty` their disjunction,
and rule matching goes through `search_by_name`, which tries every row.
*Rendering* attributes the value to `alternatives.first()`.
Every row shares the shape's `fully_qualified_name`,
so the reported sub-query is the same whichever row is picked,
and for both examples above the decomposer and the engine agree on the reported value.
What is lost is which `CaptureRef` accepted the value:
interpretations do not carry it,
whereas the engine's path records the rule it traversed.

Whether an ambiguous name should be rejected when building the model,
or reported once per accepting alternative,
is an open design question; until it is decided, `first()` is kept and documented.

#### Supported Shapes

Building a model **asserts** that every variable names a rule the spec defines
and that the rule is a leaf.
Both failures are treated as errors in the shape, not conditions to recover from,
so `search_by_log_shapes` panics with a message naming the shape and rule:

- An **undefined rule** has no regex, so there is nothing to place a run into.
- A **non-leaf rule** (one with nested captures) would have to be reported as its nested captures,
    which the model does not carry;
    reporting the whole rule instead would silently differ from the engine.
    The shape must reference one of the leaf captures (`%blockID.num%`) instead.

A shape with **no variables** (pure static text) *is* supported:
its text is exactly what a query must reproduce,
which placement and composition handle like any other part.
This was previously mistaken for "nothing to attribute query text to",
and deferred to the engine.

Because validation happens when the model is built --
and all models are built before any shape is searched --
an unsupported shape fails the whole call up front rather than part-way through the list.

> This restriction is specific to **log shapes**.
> [`SearchString::search_by_name`] searches a rule by name
> and reports the nested captures the engine finds,
> so it accepts non-leaf names (`"blockID"`) without issue.
> The two entry points share the query type but not this constraint.

The two anchoring questions are answered on the model:

```rust
model.can_start_at(part)         // every earlier part can emit nothing
model.can_end_at(part)           // every later part can emit nothing
```

They are what make anchoring correct in the presence of nullable variables:
a start-anchored run must be the first thing the *message* emits,
which does not require it to be in the first shape *part*
if the parts before it can vanish.

#### Rule matching

[`RunFitCache`][run-fit-cache] answers "can this rule satisfy this piece of the query?"
for one `(rule name, query)` pair, independent of any shape,
and memoizes the bool in a `Memo`
(`src/search/decompose/memo.rs`, the get-or-compute map both decomposer caches share;
its lock is released while computing, so a slow simulation does not block other keys).
This is the source of the algorithm's leverage:
a corpus mentions few distinct rule names relative to the number of rule *references*,
and a query has few runs,
so the number of distinct simulations is tiny.

A piece is asked about as a search query, with `*` standing for an unpinned end:
`*piece*` (contained), `piece*` (begin with), `*piece` (end with), `piece` (exactly).
That is the whole space of pinnings
([`Pinned { start, end }`][run-fit]),
so placement asks *only* what it needs --
`matches_piece` per split point --
rather than one simulation per split of the run.

### Tier 1: Rejection

```rust
decompose::can_match(model: &ShapeModel, symbols: &[SymbolicChar]) -> bool
```

`false` **proves** no message of the shape can match.
It runs a DP over `(query cursor, shape cursor)` where every transition advances one or the
other, so the state space is a DAG solved in one reverse sweep.
It allocates one bit per state.
Acceptance is `query consumed && shape can stop`;
a trailing wildcard walks the remaining atoms itself, so end anchoring needs no flag.

Variables are approximated by their charset and permitted to match empty.
Both **widen** what is accepted, so the result is a superset: a rejection is sound.

There is a cheap sufficient fast path, `is_obviously_not_ruled_out`:
if the query starts with a wildcard
and some single variable's charset admits every literal in the query,
that variable alone could emit the whole query,
so the shape cannot be rejected.
It is `O(variables + query)` against the walk's `O(atoms x query)`,
and it is the common case on real shapes --
paying for the full table there once made the rejection tier cost more than it saved.
When the query is end-anchored, the variable must additionally satisfy `can_end_at`.

Only the yes/no answer is taken from this tier:
because variables are over-approximated, any decompositions the walk would enumerate
form a superset of the engine's answers, so none are ever collected.

### Tier 2: Composition

This is the path that actually answers the question, and it never builds a shape automaton.

#### Placement

[`PlacementTable::compute`][placement-compute] asks, for each run, where it could go.
A run must be produced in full, and there are only three possibilities:

1. **wholly inside a rule** -- `matches_piece(.., Pinned { start, end })` with the query's
    anchoring for both ends, plus `can_start_at` / `can_end_at` so nullable neighbours
    can stand aside;
2. **wholly inside static text** -- a substring search over *every* occurrence,
    including overlapping ones
    (`str::match_indices` would skip them, so `*aa` against `aaa` would find nothing),
    reporting the character offsets the `Placement` records use.
    Interior occurrences render identically, so a row keeps only the earliest one at or
    after each position the previous run can leave off at
    (entries are computed once per run, into a per-part map, rather than the previous row
    being rescanned per part).
    Rows are therefore computed left to right and are complete *relative to the previous
    row's endings*, not as a standalone "everywhere this run can sit" -- the
    composition DP, `Reachability`, and `last_reachable_part` all respect that order.
    Otherwise a banner of repeated characters would exceed the placement cap;
3. **straddling** a boundary -- split between a rule and its neighbour,
    answered the same way with the corresponding end pinned.
    A nullable variable in the middle of a straddle may contribute nothing,
    so a run such as `ab` can cross `a%optional.pad%b`;
    the variable is then reported as an empty capture.

Anchoring constrains each in both directions:

- A start-anchored run must satisfy `can_start_at(index)` *and* begin at offset 0 of its part;
    inside a rule it is asked with `start: true`,
    so it must be a prefix of the rule's match, not merely contained --
    otherwise `N*` would be placed in a rule matching `WARN`.
- The end is the mirror image: `end: true` and `can_end_at`.
- Anchored at **both** ends, a rule must match the run *exactly*, with nothing around it.

A cap, `MAX_PLACEMENTS_PER_RUN = 2048`, bounds the table.
Exceeding it returns `None` and the caller falls back to the engine rather than answer wrongly --
placement enumeration is not polynomial in general
(a chain of back-to-back rules can split a run exponentially many ways).

#### Composition

[`compose`][compose] chooses one placement per run
such that they run left to right and do not overlap.
The state is `(run index, earliest usable position)`;
every placement advances both, so this is another DAG solved by a reverse sweep.
The position is a `(part, char offset)` pair, not a bare part index,
which is what lets several runs share one static part.

Feasibility is decided first, as bits;
only a shape that survives pays to materialize its compositions.
A multi-piece capture (one rule holding pieces of several runs) is re-verified against the rule
as a whole,
since a rule admitting each run alone need not admit them together --
an alternation such as `INFO|WARN` admits either and neither pair.

#### Rendering

[`Composition::to_interpretation`][to-interpretation] turns a composition into the reported result.
A wildcard separating two runs is emitted exactly where the query had one,
so a run crossing a part boundary stays contiguous.

The `*` *padding* around a value is decided differently for the two kinds of part,
because they mean different things --
this is `symbolic_value_of`'s `padding` parameter:

- A **rule** may emit text of its own around the query's characters,
    so it is padded wherever the query permits:
    suppressed where the run continues into a neighbouring part,
    and where the query anchors the run to that end of the message.
    The padding is then **tightened** to what the rule can use (`tighten_capture`):
    a leading `*` on `*v` is dropped when no match of the rule is `v` with text in front,
    decided by `RunFitCache::has_slack` as an automaton intersection of the rule with
    `.+ v`; the trailing `*` likewise, with `v .+`.
    So `*INFO*` against `INFO|WARN|ERROR` reports `INFO`, and against `0x[0-9a-f]+`,
    `*0xff*` reports `0xff*`, as the engine does.
    This cannot be asked as a glob query (`matches_query`): that only tests whether
    `L(rule)` meets `L(v*)`, not whether the `*` of `*v` is used --
    rule `a|ba` has `a*` satisfiable, yet `ba` needs the leading `*` of `*a`.
    Only the ends of a value are tightened; interior wildcards are run boundaries
    the query demands, and a vacuous capture (`*` alone) is left marking its position.
- **Static text** is reproduced verbatim,
    so its value must glob-match the part's text *exactly*.
    Padding is decided by the piece offsets recorded in `Placement` --
    a `*` stands for the characters of the part the run does not cover,
    and appears if and only if there are some.
    Anchoring and run continuation need no special case:
    a run flowing in from the previous part necessarily begins at offset 0,
    and one flowing out necessarily reaches the text's end.
    The offsets are reduced to two flags, `Literal::pad_before`/`pad_after`,
    when a composition is built,
    and compositions differing only in those flags are merged by OR-ing them
    (`merge_by_padding`):
    the text is fixed, so where within it a run sits is unobservable,
    and `*aa*` against `aaa` is one decomposition, `'*aa*'`, not `'aa*'` and `'*aa'`.
    The merge is sound but **not exact**: the merged glob can be strictly looser than
    the group (`L('*aa*')` holds `baab`, which neither member admits),
    since a union of globs is not in general one glob.
    It keeps what a user acts on -- which parts the text lands in, and in what order --
    and the differential tests cannot see the looseness, as they strip wildcards.

    This is the opposite of the intuition that "static text must match verbatim,
    so it is never padded".
    Precisely *because* it is verbatim,
    a value covering only part of it must be free to skip the rest;
    see [Correctness](#correctness).

Static text is reported whether or not the query constrains it,
matching the engine and `search_by_name`'s output shape:
a stretch the query's wildcard merely passes over becomes a `'*'` sub-query.
So a run `foobar` against `foo%rule%` yields two sub-queries, `'foo'` and `(?<rule>bar)`.
Consecutive static parts are merged into one sub-query
(an escaped `%%` tokenizes into two adjacent parts,
and `Interpretation::invariants` forbids two static sub-queries in a row).
This is pure rendering:
the parts and their attribution are already known,
so it needs no automaton or simulation.

Rule references after the last *constrained* one are omitted --
the query's trailing wildcard leaves them unconstrained,
and each would only be a vacuous capture carrying no information.
What sits past the last constrained part is rendered by the **tail rule**
(`crate::search::Tail`), shared with the engine:

- dropped *variables* are omitted;
- dropped *static text* still appears, because the message contains it:
  on a trailing static sub-query it becomes a `'*'`,
  and a rendering ending in a bare capture gets one `'*'` of its own
  to stand for the static text that followed.
  `*id=*` against `id=%digits%` is `'id=*'`, not `'id='`
  (which would claim the message ends after `id=`).

> When the query is end-anchored those trailing references are *not* unconstrained
> but pinned to the empty string.
> Composition renders exactly that, as one empty capture per trailing variable --
> the same thing the engine reports.

The rendering deliberately does **not** reproduce the engine's wildcard *placement* inside a value
(the engine may write `INFO*` where composition writes `*INFO*`).
Both describe the same decomposition;
the differential test compares a normal form that strips wildcards, so this stays pinned.
A trailing `*` is emitted only when something actually follows the last constrained part --
the engine obeys the same rule for the parts its automaton was truncated past,
so neither path appends it unconditionally.

#### Budgets

| Constant | Value | On exhaustion |
| --- | --- | --- |
| `MAX_PLACEMENTS_PER_RUN` | 2 048 | `PlacementTable::compute` -> `None` (+ `info!`) |
| `ComposeBudget::max_compositions` | 65 536 | `Composed::Unknown` |
| `MAX_UNPINNED_SPLITS` | 8 | `PlacementTable::compute` -> `None` (+ `info!`) |

Every budget degrades to "no conclusion", never to a wrong answer:
reaching one returns `None`/`Unknown`
and the caller falls through to a more expensive but complete tier.
The two placement caps also emit a `tracing::info!` naming the run and the cap,
so a silent performance cliff is visible.
Note this is a different treatment from unsupported shapes, which panic:
a cap is a resource limit an adversarial shape can reach,
whereas an unsupported shape is a mistake in the shape itself.

`max_compositions` is deliberately generous,
because falling back is only worthwhile when the engine would do something *different* --
not when it would derive the same answers more slowly.
A shape that genuinely has many decompositions has them on either path,
and enumerating them here is far cheaper than walking paths through an intersection.
On the HDFS corpus, `*INFO*blk*` against the repeated classpath shapes
(377 parts, ~190 placements per run)
yields ~18 000 distinct interpretations:
tier 2 enumerates them in ~2 s,
whereas the engine exceeds `PATH_TIMEOUT_MILLIS` and aborts.
At the old cap of 4 096 those four shapes deferred,
and the query could not be answered at all.

`MAX_UNPINNED_SPLITS` caps the candidate split points tried
when a run must pass through a rule
that has *another rule* immediately after it in the shape,
so nothing in the static text pins where the split falls.
When the run is longer than the cap, **not every split can be tried** --
and a dropped candidate would be a false rejection,
so `candidate_middle_lengths` returns `None` and poisons the whole table.
This propagation is what makes the cap safe:
the incomplete table is treated exactly like an unreasonable rule,
and the caller falls back to the engine.

> This was previously a real bug: the cap silently truncated the candidate list,
> so a shape whose correct split exceeded 8 characters
> (e.g. `%r1%%r2%%r3%` with `r2 = [a-z]{9}`, query `1aaaaaaaaa2`)
> was rejected even though the engine matched.
> Pinned by `middle_split_longer_than_the_candidate_cap_still_matches` and
> `adjacent_rule_chains_are_never_falsely_rejected` in the differential test.

### Tier 3: The Engine

Reached only when composition declines to conclude.
It builds the shape's TNFA and intersects it with the query's, then enumerates paths.
The intersection **always runs to the end** --
a state accepts only when both sides accept --
and the query is used exactly as written, so a trailing `*` is a real `.*`:

```rust
let search_nfa = Tnfa::for_regex(&self.to_regex());
let intersection = shape_nfa.intersect::<true>(&search_nfa);
let paths = intersection.compute_paths(spec);
```

The `FOR_SEARCH = true` parameter tells the intersection
to keep non-literal (wildcard) transitions maximal,
so a literal edge that "came from" the shape
is distinguishable from one that is a meaningful search value.
It is the only const parameter left.

Because a path now always reaches an accepting state, every capture it opened was explicitly closed.
`PartialPath::finish` asserts that,
and reports a capture's contents exactly as the rule produced them --
including *empty*, where the query pins the rule to producing nothing.

The parts of the shape the engine's automaton was truncated past and what it dropped on the
engine's own tail are reconciled by the [tail rule](#rendering) (`Tail`),
applied only when the query is unanchored:

- **Trailing captures are dropped.**
    Past the query's last literal character every rule reference is unconstrained
    and would be reported as a bare `*`.
    Composition omits exactly these.
    Positional identity is read left to right, so trimming the tail is safe --
    a *leading* or *interior* vacuous capture is always kept,
    since its position is what tells two references to one rule apart.
- **Trailing static text is kept, and given a `*`.**
    Unlike a capture it is not vacuous: it names text the message still contains.
    A bare `'a'` for a shape continuing `a%word%b...`
    would assert the message *ends* at `a`, which matches nothing;
    see [Satisfiability of static values](#correctness).

The result is sorted, deduplicated,
and `dedup_covered_interpretations` drops interpretations another already covers.

#### Covering

[`LeafQuery::covers`][covers] decides
whether one sub-query's value describes everything another's does.
It is **exact glob containment**:
the names must agree, and every string matching the other value must match this one.
For globs whose only wildcard is `*`,
that holds iff this value matches the other value with each of its `*` replaced by a fresh
symbol no literal equals.
So `glob_covers` is an ordinary `O(n x m)` glob match in which a `*` of the other value
is a single token only a `*` of this value can consume.
It sees, e.g.
`aa*` covering `aaa*`, which a segment-by-segment comparison would miss.
Values are query-length, so the quadratic match is cheap.

Being language containment, `covers` is reflexive and transitive on any values,
which is what makes the dedup loop reach a fixpoint,
and the result is a **minimal antichain** up to equal languages
(two values with the same language, e.g. `a**` and `a*`, cover each other;
the first one kept wins).
Producers still emit values with no adjacent wildcards --
`symbolic_value_of` emits at most one leading and one trailing `*`,
and `condense_wildcards` collapses any doubling introduced by merging --
and `Interpretation::invariants` asserts it, but `covers` no longer relies on it:
the form is purely canonical output.

The loop itself builds an antichain in one pass per candidate:
a candidate covered by a survivor is dropped,
otherwise anything it covers is removed (by `swap_remove`) and it is kept.
Exact duplicates are removed first, by sorting;
order is not preserved otherwise.

### Shape Truncation

Real shapes carry very long tails of static text --
in the HDFS corpus,
hundreds of shapes are over 20 KB of banner text wrapped around two small rules --
and the automaton builder emits **one state, and one formatted name, per literal character**.
Intersecting such a shape costs far more than the query warrants.

This is what makes the restored trailing `.*` affordable,
and it is the load-bearing half of the "don't fully simulate a trailing `*`" principle:
the wildcard is real, but there is little left for it to traverse.

[`PlacementTable::last_reachable_part`][last-reachable-part]
returns the last shape part the query's literal text can reach,
so the shape is built as a **prefix** ending there:

```rust
let end = table.last_reachable_part()?;
let fragments = model.fragments_in(0, end);
let truncated = spec.automata_for_fragments(&fragments)?;
```

On a 20 000-character shape this takes the automaton from ~20 000 states to ~56, with identical
results.

Four subtleties, all learned the hard way:

- **It is the *last* run that bounds the reach**, not the union over all runs.
    A composition lays its runs down left to right,
    so an earlier run that *could* sit late in the shape
    never does in a composition that also places the runs after it.
    Taking the union truncates far less --
    on the HDFS corpus it trimmed 377 parts to 375, i.e. not at all.
- **Only the tail may be dropped.**
    The head must be kept verbatim: the query's leading wildcard still has to traverse it.
    Replacing the elided region with `.*` is **not** equivalent --
    `.*` is a superset of the text it stands for,
    so it lets runs straddle where the real static text forbids it
    and invents interpretations the full shape does not have.
- **Never truncate when end-anchored.**
    The truncated parts are precisely the ones an end-anchored query still has to match.
    `truncated_automata` still builds from the model's fragments then; `end` is the last part.
- **A dropped tail containing static text must still be reported**, as a trailing `'*'`,
    because the full shape would have reported it.
    `TruncatedShape::dropped` carries that fact to the rendering as a `Tail`,
    which is the same rule composition uses for its own unconstrained tail.

`PlacementTable` is therefore computed even for shapes composition will not answer,
purely so the engine fallback can truncate.
This is why `composed_interpretations` takes the table as an argument
rather than computing it.

### Correctness

The properties the implementation must preserve, and where they are pinned:

- **Soundness of rejection.**
    A shape the rejection tier discards must produce no engine match.
    A rejection must never drop a real result.
    This is why every candidate cap that cannot be exhausted completely
    must return "no conclusion" rather than an empty placement set;
    see [Budgets](#budgets).
- **Transparency.**
    `search_by_log_shapes` must return exactly what the engine alone would.
    This is what the differential test asserts, comparing rendered structures as sets.
- **Truncation transparency.**
    A truncated automaton must give the same interpretations as the full one.
- **Invariant.**
    Every literal character of the query appears in the result, in order,
    however it was split between static text and rules;
    a wildcard appears exactly where the query had one.
- **Normal form.**
    No value carries adjacent wildcards, and no two static sub-queries are adjacent.
    Both are asserted by `Interpretation::invariants`.
    The first is canonical form only: `**` says what `*` does,
    and [`LeafQuery::covers`] is exact containment, so dedup is correct either way.
    See [Covering](#covering).
- **Satisfiability of static values.**
    A static sub-query's value must glob-match its shape part's text exactly.
    Reporting `'*ab'` for the run `ab` inside `abcdef` names the right characters
    but asserts the text *ends* in `ab`, which is false and matches nothing.
    A run covering only part of a stretch is therefore padded on both sides;
    see [Rendering](#rendering) for how the offsets decide it.
    Pinned by `static_leaf_query_values_are_satisfiable` --
    the structural comparison cannot see this,
    because stripping wildcards is blind to where they sit.
- **Never tighter than the engine.**
    Tightening a capture's padding must not drop a match:
    wherever composition and the engine report the same decomposition,
    every capture value composition reports must glob-contain the engine's.
    Pinned in `decompose_never_drops_a_match`;
    removing `tighten_capture`'s slack check makes it fail at once.
- **Support.**
    A shape either is supported -- every variable defined and leaf --
    or the call fails up front.
    It never silently falls back to the engine for an unsupported shape,
    because that would make the same query return answers in a different form
    depending on the shape.

### Testing

| Test | What it pins |
| --- | --- |
| `tests/decompose_differential.rs` | Transparency over 447 queries x 18 shapes, including end-anchored forms, nullable variables, and pure-static shapes, plus prefilter soundness. The primary safety net. Also pins satisfiability of static values (`static_leaf_query_values_are_satisfiable`), which the structural comparison cannot see, and the two `MAX_UNPINNED_SPLITS` regressions. |
| `tests/search_anchoring.rs` | The anchoring semantics through the public entry point, plus `decompose`/engine agreement. |
| `tests/search_shape_support.rs` | Which shapes are supported, pure-static handling, and the panic contract for unsupported shapes. |
| `tests/shape_narrowing.rs` | Truncation is transparent, and actually reduces state count. |
| `tests/decompose_invariant.rs` | The invariant holds over the real HDFS corpus (thousands of shapes). |
| `src/search/test.rs` | `covers` soundness against true glob containment (`covers_never_claims_an_unsound_containment`) and its partial-order properties in normal form (`covers_is_a_partial_order_in_normal_form`). |
| `src/search/decompose/*/test.rs` | Unit tests for placement, composition, run fit, prefilter, shape, and cache. |
| `tests/local_search.rs::blk_id_full_log_message` | End-to-end corpus run; also a performance baseline. |

The differential test is the one to update when semantics change:
it runs both sides (public entry point vs. `interpretations_for_log_shape_via_engine`)
over a query sweep that mixes literal and wildcard interleavings
plus a deterministic pseudo-random sweep,
and asserts they agree.

### Complexity

With `Q` = query length, `R` = number of runs, `P` = shape parts, `L` = shape literal characters:

| Tier | Cost | Notes |
| --- | --- | --- |
| Runs | `O(Q)` | once per query |
| Rule matching | `O(rule size)` per distinct `(rule, query)` | memoized, shape-independent |
| 1 (`can_match`) | `O(variables + Q)` fast, `O(P x Q)` worst | bits only |
| 2 (`compose`) | `O(R x positions x placements)` | plus enumeration, budgeted |
| 3 (engine) | `O(shape NFA x query NFA)` | shape NFA truncated to the last reachable part when unanchored |

### Known Limitations

- **Each piece of a rule is simulated separately.**
    `matches_piece` runs one automaton intersection per `(rule, piece, pinning)`
    (`src/search/decompose/run_fit.rs`).
    A single dynamic program over all substrings of the run would answer every split in one pass.
    This is the largest remaining constant factor in tier 2;
    correctness does not depend on it.
- **The engine fallback can abort on pathological input.**
    `compute_paths` enforces `PATH_TIMEOUT_MILLIS` (2 s) and then calls `todo!()`
    (`src/nfa/search_decomposition.rs`),
    i.e. it panics rather than returning an error or degrading.
    This predates the decomposition work
    and is only reachable when tier 1 and tier 2 both decline,
    which the tier-2 composition budget makes rare;
    it is still a live crash for an input that gets there.
- **Padding merges are looser than the group they replace.**
    See [Rendering](#rendering); sound, but a merged static value can admit strings
    that no single placement does.
- **Some values stay looser than the engine's.**
    After capture tightening, the differential sweep still finds 33 of 10488 pairs
    where the engine is strictly tighter, in two classes:
    a nullable rule an anchored query forces to emit nothing
    (`INFO*` against `%optional.pad%%word%`: the engine reports `optional.pad=`,
    composition the vacuous `*`), 30 pairs;
    and static text, where the padding merge and interior wildcards make composition
    looser (`*aa*` against `aaa`: `'*aa*'` versus `'*aa'`, `'aa*'`), 3 pairs.
    Both are sound.
- **An ambiguous variable name is attributed to its first alternative.**
    The reported name is right, but which rule accepted the value is not recorded;
    see [Ambiguous Names](#ambiguous-names).

### Related Code

- `src/search.rs` -- the public API (`search_by_name`, `search_by_log_shapes`), the tier
    cascade, shape truncation, and the engine path (`interpretations_for_shape`).
- `src/search/query.rs` -- `SearchString`, `SymbolicChar`, parsing, `anchored_start` /
    `anchored_end`.
- `src/search/interpretation.rs` -- `Interpretation`, `LeafQuery`, `Tail`,
    `LeafQuery::covers` / `glob_covers` and `dedup_covered_interpretations`.
- `src/search/decompose/shape.rs` -- `ShapeModel`, `Variable`, anchoring helpers.
- `src/search/decompose/query.rs` -- the `Query` the decomposer sees: symbols, runs, anchoring.
- `src/search/decompose/prefilter.rs` -- the rejection DP and its fast path.
- `src/search/decompose/placement.rs` -- `Placement`, `PlacementTable`,
    `last_reachable_part`, composition DP.
- `src/search/decompose/compose.rs` -- composition enumeration and rendering.
- `src/search/decompose/run_fit.rs` -- `Pinned` and `RunFitCache`, the per-rule match memo.
- `src/search/decompose/cache.rs` -- `ShapeModelCache`.
- `src/search/decompose/memo.rs` -- `Memo`, the get-or-compute map behind both caches.
- `src/parsing_spec.rs` -- `automata_for_shape` / `automata_for_fragments`, `split_log_shape`.
- `src/nfa.rs`, `src/nfa/search_decomposition.rs` -- the engine.

Tests: `tests/search_anchoring.rs`, `tests/search_shape_support.rs`, `tests/shape_narrowing.rs`,
`tests/decompose_differential.rs`, `tests/decompose_invariant.rs`.

[search-by-log-shapes]: ../src/search.rs
[search-parse]: ../src/search/query.rs
[search-anchored]: ../src/search/query.rs
[shape-model]: ../src/search/decompose/shape.rs
[shape-part]: ../src/search/decompose/shape.rs
[shape-model-cache]: ../src/search/decompose/cache.rs
[charset]: ../src/search/decompose.rs
[decompose]: ../src/search/decompose.rs
[engine]: ../src/nfa/search_decomposition.rs
[placement-compute]: ../src/search/decompose/placement.rs
[compose]: ../src/search/decompose/compose.rs
[to-interpretation]: ../src/search/decompose/compose.rs
[covers]: ../src/search/interpretation.rs
[run-fit]: ../src/search/decompose/run_fit.rs
[run-fit-cache]: ../src/search/decompose/run_fit.rs
[prefilter]: ../src/search/decompose/prefilter.rs
[last-reachable-part]: ../src/search/decompose/placement.rs
[split-log-shape]: ../src/parsing_spec.rs
[intersect]: ../src/nfa.rs
[parser]: ../src/parser.rs
[differential]: https://github.com/y-scope/log-surgeon/blob/log-mechanic/rust/tests/decompose_differential.rs
[narrowing]: https://github.com/y-scope/log-surgeon/blob/log-mechanic/rust/tests/shape_narrowing.rs
[parsing-spec]: parsing-specification.md
