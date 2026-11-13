# Tournamo — Design

Tournamo is a generic core library for running tournaments of matches
between two sides (single elimination, round robin, Swiss, ...), whose key
components are pluggable and selected at runtime, with a CLI and a browser GUI
on top. [`README.md`](README.md) says what it does today and
[`requirements.md`](requirements.md) what it is meant to do. Each section here
states the decision, why, and the alternatives rejected. Sections marked
*planned* are not implemented.

## Global constraints

These apply to every mechanism below.

- **Runtime selection.** Which implementation of a pluggable axis is used is
  chosen at runtime, not by monomorphization.
- **Third-party openness.** Another crate can add an implementation of any axis,
  a metric, or consumer data, without changing this crate and without
  special-casing.
- **Two persistence paths.** A `Competition` persists as a snapshot and as its
  transition log. Both survive a recompile, a dependency bump or a redeploy as
  long as the concrete types are still linked in, so nothing process-local (raw
  `TypeId`) appears in a serialized form.
- **Determinism under replay.** Progress is a log of transitions replayed from
  the initial state, so `Transition::apply` is a pure function of state and the
  transition's data. Anything from the clock, the environment or a service
  arrives as transition data.
- **No untyped data bags.** `HashMap<String, Value>` turns wrong keys and shapes
  into runtime failures and carries no behavior. Open-ended data is keyed by
  Rust type instead (`TypeMap`).

## Pluggable axes

Every axis of behavior (`PhaseFormat`, `SliceRanker`, `MetricsCalculator`, more
as they turn up) is its own public, unsealed trait, used as `Box<dyn Trait>`.
Writing an implementation for one axis never touches another's trait. The
mechanics come from `plugin-factory`: the trait is declared with `#[axis]` and
has `plugin_factory::Plugin` as a supertrait; a plugin derives `Plugin` and
registers with `#[register]` (see [The plugin catalog](#the-plugin-catalog)).
Every engine axis also has `DynClone` as a supertrait, since `AddPhase::apply`
clones the plugins on every replay.

**The ranker is not an axis.** A phase is always ranked by a
`HierarchicalRanker` built from its list of `SliceRanker`s, each breaking the
ties the ones before left. Every ranking foreseen is such a chain. A criterion
sorting by one metric (e.g. `MorePoints`) implements `RankingCriterion` (metric
type plus comparison) and is a slice ranker through `rank_by`; others (e.g.
head-to-head) implement `SliceRanker` directly.

Rejected:
- A closed `Kind` enum per axis (`enum_dispatch`): third parties cannot add
  variants.
- Generic type parameters per axis: conflicts with runtime selection, and every
  combination is a distinct type.
- A `Ranker` axis above `SliceRanker`: a layer of trait objects and tags with no
  second implementation.

## Plugin state

A plugin may need memory beyond what matches and rankings record (a precomputed
bracket, a structure it would otherwise rebuild on every reported result).

**Plugins own their state and update it through `&mut self`.** The methods
following the phase's progress take `&mut self` and the phase's data:

```rust
fn create_new_matches(&mut self, phase: &PhaseView<'_>) -> Schedule;       // PhaseFormat
fn update_after_match_result(&mut self, phase_data: &PhaseData, /* ... */)
    -> Vec<(RankingId, Metric)>;                                           // MetricsCalculator
```

Methods describing configuration (`default_metric_value`) stay `&self`; whether
a format allows draws is the fact `NoDraws` (below). A slice ranker follows no
progress: it ranks whenever the standings are read (see
[Metrics](#metrics-plugin-produced-ranking-data)), so `SliceRanker::rank` takes
`&self`.

- **`PhaseData` is everything in a `Phase` but its plugins** (rankings, matches,
  byes, seed). A call borrows a plugin field mutably and `data` immutably, two
  disjoint fields, so no plugin is moved out. A plugin therefore does not see
  the other plugins; what it needs of them reaches it through `PhaseData`
  (metrics, annotations). The format gets a `PhaseView`: the data, and the
  standings on request, ranked by the ranker, a third field borrowed
  immutably. `data` is a nested field, not `#[serde(flatten)]`, which postcard
  cannot read.
- **State is a field left out of the schema**: `#[plugin(skip)]` plus
  `#[serde(default)]`. A plugin built from a configuration starts at the
  default; a saved one comes back with its state. It is always serialized
  (`skip_serializing_if` breaks postcard); an answer to a skipped field is
  dropped.
- **Undo needs nothing extra.** The plugin is part of the state, so a replay
  rebuilds it from its configuration through the same calls. The contract is
  determinism: its state depends only on the `PhaseData` and calls it has seen.

Rejected:
- Stateless plugins recomputing from `&Phase` on every call: cost grows with the
  phase on the hot path.
- State in a `TypeMap` of `PhaseData`: exposed to every plugin, and two plugins
  of one type collide.
- Interior mutability: hides the mutation the replay contract governs.
- Passing `&Phase` with the plugin taken out: the plugin sees a phase that
  exists nowhere else.
- A configuration plugin building a separately tagged runtime object: a second
  trait and tag per stateful axis.
- Rebuilding state lazily after a load: every stateful plugin would need its own
  replay.

## Compatibility between plugins

Not every combination of plugins is valid, and the check must not be a list of
concrete type pairs, which is blind to third-party plugins. Dependencies run
between every pair of axes and both ways: a calculator reads another's metric
(Buchholz on points) or annotations only some formats write (round of
elimination on `Eliminates`); a criterion sorts by a metric some calculator must
produce; a format needs something of the ranker (Swiss pairs by rank) or of the
phase's input (Monster DYP needs rankings of single players). A phase is a loop
(schedule, results, metrics, ranks, schedule), so there is no pipeline order to
sort these into. *Compatibility* (does a set fit) and *execution order* (in
which order plugins of one pass run) are kept apart.

**Compatibility is checked on typed facts.** A fact is a value of a type
implementing the marker trait `plugin_factory::compatibility::Fact`, identified
by `TypeId`. A plugin states what it provides and requires in a `Declaration`:
`declare_static` for what every configuration declares (read by the catalog
before configuring) and `declare` for what depends on configuration.

```rust
impl Declaration {
    pub fn provide<T>(&mut self, value: T) -> &mut Self;
    pub fn require<T>(&mut self, name: &'static str) -> &mut Self;  // present at all
    pub fn require_that<T>(&mut self, name: &'static str, description: &'static str,
                           holds: fn(&T) -> bool) -> &mut Self;
}
```

- A plugin's declaration includes those of the plugins in its fields. Each slice
  ranker is checked on its own, so an issue names the one that has it.
- **Facts are opted into** by `impl Fact for T {}`, no blanket impl: requiring a
  type never provided as a fact (a metric instead of its `CalculatesMetric`)
  does not compile, and by the orphan rule each fact belongs to the crate owning
  its concept, so crates cannot collide on `bool` or `u32`.
- Facts come from the phase's plugins and its context, the competition
  (`Competition::declare_phase_context`, e.g. `EntrantsPerRanking(1)`). The
  check is flat: any plugin may require any fact, mutual requirements are fine,
  since a requirement states what is true of the phase, not what ran first. A
  fact provided twice (also by a plugin and the context) is an error.
- The vocabulary is open: core defines facts about what it owns
  (`MatchAnnotation<A>`, `EntrantsPerRanking`), other crates about theirs. A
  fact carries a value, so a requirement can ask *how* as well as *whether*,
  and a fact holding a `fn` or trait object carries behavior.
- **Calculating a metric is a fact**, `CalculatesMetric<M>`. Two calculators of
  one metric are a fact provided twice; a metric nobody calculates is unmet. A
  criterion requires it through `requires_metric::<Self>`, read off its
  `C::Metric`, so it cannot drift from what it compares. A test checks that each
  calculator declares the metric its `default_metric_value()` returns.
- **A field can be irrelevant while a fact holds**
  (`#[plugin(irrelevant_with = NoDraws)]` on the points for a draw), so that a
  form leaves it out (`Draft::is_relevant`); it stays answered, and a plugin
  reads it as before. The condition is negative, a fact making the field
  irrelevant rather than one it needs: a plugin of another crate forgetting to
  provide a fact shows a field too many rather than hiding one that matters.
  `NoDraws` is the format's only statement about draws: `allows_draws` of a
  `dyn PhaseFormat` reads it, so the form and the rejection of a draw cannot
  disagree.
- **Self-reported, accepted.** Facts like "rankings are pairs" are not something
  the compiler can derive. A criterion leaving out `declare_static` requires
  nothing and panics in `rank_by`; test helpers building a phase from a plugin
  plus only its declared facts catch undeclared reads.
- **The mechanism knows no axes.** `plugin-factory` holds declarations, the
  check, ordering and implied plugins; only the composite `PhasePlugins` (one
  format, a list of slice rankers, a set of calculators) knows the phase's axes.

**Execution order is fixed by the engine, except among calculators.** On a
result the calculators run, then the ranker, then the format schedules; matches
and their annotations exist before their results. The only plugins reading what
another wrote in the same pass are calculators, so only they are ordered: one
requiring a fact runs after its provider, ties keep the configured order, a
cycle is an error. The calculators are a `DependencyOrderedSet`, which sorts
itself on `new` and on deserialization (how `Draft::build` builds a composite
and how a log loads), rejects duplicates and cycles, and keeps an order that is
valid already. The slice rankers stay a `Vec`, since their order is the ranking.

**Implied plugins.** For a required fact nothing provides, `Draft::add_implied`
adds the registered plugin providing it if exactly one exists and it has no
fields (nothing to choose or configure; the win-draw-loss record is never asked
about). Several candidates, or a provider with fields, add nothing: a default is
never filled in unasked, and a UI offers such a provider instead. It only
appends to list fields (never a second format), repeats until a pass adds
nothing (so it is idempotent), and adds a fact's plugins **all or nothing**:
only if the chain raises no issue (a fact twice, one still missing, a condition
unmet, a cycle), so an addition never leaves a problem with a plugin nobody
chose. `Draft::build` adds nothing; whether to add the implied plugins is the
consumer's choice, and a consumer whose draft may still change adds them to a
clone.

**Adding implied plugins happens before the transition; validation is in it.**
In production, building from `Competition::phase_draft` (which carries the
phase context) is the only way to make `PhasePlugins` besides deserializing.
`AddPhase` holds the complete sorted list, so neither log nor snapshot depends
on the catalog, which depends on what is linked in. `AddPhase::check_applicable`
checks facts; facts known only at the start (the number of entrants) are checked
at `StartCompetition`. All of it runs once per phase; the hot path pays nothing.

**Tests build plugins directly**, behind the `test-util` feature: constructors
of plugins with fields, `PhasePluginsBuilder` (defaults: round robin, empty
ranking, no metrics: a phase that can be played) and `PhaseBuilder`. A consumer
enables the feature only in dev-dependencies, so production code cannot skip
configuration. Tests of configuration still use drafts.

*Planned, once a metric has a second calculator:* a preference among candidates,
so a fact can imply a plugin where it is ambiguous today.

Rejected:
- Marker traits via `intertrait`: answers only "does some plugin implement X",
  no conditions on values, a link-time registry, no generic impls.
- A closed `Capability` enum, or a match over concrete type pairs: closed to
  third parties.
- Fixed stages requiring only earlier stages: the format needs the ranker and
  the ranker the format.
- Consumers adding needed calculators themselves: a table of plugins in every
  consumer.
- A default provider on the fact type, or the requiring plugin naming one: core
  facts would name concrete plugins, and two plugins could suggest different
  defaults.
- Adding a provider whatever it brings along: turns one missing fact into a
  problem with a plugin the operator never chose.
- Resolving inside `AddPhase::apply`: linking a crate in would change every
  saved log's replay.
- A typed builder for `PhasePlugins` in production: a second way to make a
  phase, skipping implied plugins.
- A plugin replacing a fact of the context (a draw providing
  `EntrantsPerRanking(2)`): a fact provided twice for now; to revisit when a
  phase's context comes from the phase before.

## The plugin catalog

A consumer configuring a phase must not hold a table of plugins (which a
third-party plugin would never appear in), and the engine needs the same
knowledge to imply plugins.

**A plugin registers itself, described.** Deriving `Plugin` states its id,
label and description and reads its schema (the questions a UI asks, with the
answers offered) from its fields. `#[register]` submits it to its axis's catalog
via `inventory`, so the catalog is whatever is linked in:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Plugin)]
#[plugin(id = "tournament_engine::points", label = "points",
         description = "…", declare_static = Self::declare_static)]
pub struct PointsCalculator {
    #[plugin(label = "points for a win", default = Score::ONE)]
    points_for_win: Score,
    // …
}

#[register]
impl MetricsCalculator for PointsCalculator { /* … */ }
```

- **One typed registry per axis**, generated by `#[axis]`: a consumer gets a
  `Box<dyn MetricsCalculator>`, never something to downcast.
- **Ids are a contract**: the id is the plugin's `typetag` tag, naming it in
  logs, snapshots and configurations; namespaced by crate and pinned in a test.
- **Defaults are offered, never filled in.** `#[plugin(default ..)]` is what a
  UI offers; building still needs every field answered. It is separate from
  `#[serde(default)]`, which only says how a missing field loads: offering an
  answer must not make a saved plugin lacking the field load silently.
- **UI text lives on the plugin** in English; translation is keyed by the id.

Rejected: one global registry returning `Box<dyn Any>` (a downcast instead of a
type); generating the CLI's enums from a plugin list (still a consumer table);
hand-written registrations (strings repeated, no schema); a field's serde
default as its offered answer (one attribute for two concerns).

## Metrics: plugin-produced ranking data

A `Ranking` carries the data it is sorted by (points, wins/draws/losses,
Buchholz, ...), which varies per phase and must be extensible.

**Metrics are identified by Rust type**: a metric implements `TypeMapValue`,
`Metric = Box<dyn TypeMapValue>`, `MetricId = TypeId`. `TypeId` is fine here
because the checks run in-process on live values, never on serialized data,
where the `typetag` tag is used. A `Ranking` stores its metrics in a `TypeMap`
(`Ranking::metric::<T>()`), a hash lookup rather than a scan.

**Calculators work incrementally:**

```rust
#[axis]
pub trait MetricsCalculator: Plugin + DynClone {
    fn default_metric_value(&self) -> Metric;
    fn update_after_match_result(&mut self, phase_data: &PhaseData, r#match: &Match,
                                 old_metrics: &OldMetrics) -> Vec<(RankingId, Metric)>;
    fn update_after_bye(&mut self, phase_data: &PhaseData, bye: &Bye,
                        old_metrics: &OldMetrics) -> Vec<(RankingId, Metric)>;
}
```

`default_metric_value()` seeds every ranking and names the metric the
calculator owns. A calculator depending on another (Buchholz on points) reads
the pre-update values through `OldMetrics::of::<M>()` and the new ones in
`PhaseData`, which is why calculators run in dependency order.

- **The matches a ranking played are core data** (`Ranking::played_matches`, in
  report order), not a metric every opponent-based metric had to require and
  that was cloned on every result.
- **Comparison stays typed.** `rank_by` looks up its metric once per ranking of
  the slice and sorts the typed values, so a comparison neither looks up nor
  downcasts. Each criterion sees only the slices still tied. One sort over
  pre-extracted columns of every metric was measured ~20% slower on
  `large_tournament`.

**Ranks are computed when the standings are read**; only the metrics are
stored. `Phase::standings` ranks the rankings by the phase's slice rankers on
every call and returns them best first, each with its rank and its unique rank
(the rank, ties broken by the initial unique rank). The standings borrow the
phase, so they are never stale; a reader asks once and keeps them. A format
asks through the `PhaseView` it schedules with, as a Swiss system will once per
round; the standings count every result reported and every bye reached.
Sorting after every result made each replay (undo, redo, loading, editing a
phase) sort once per result in the log, about 90% of its time. Natively, on the
`large_tournament` benchmark (1000 entrants, 5000 results, every metric, four
criteria):

| ranks | replay (`full_tournament`) |
|---|---|
| sorted and stored after every result | 644 ms |
| computed when the standings are read | 58 ms |

Reading the standings there sorts once: 0.08 ms.

Rejected: string-keyed metrics; a fixed `RankData` comparison per ranking type;
discovering produced metrics by running calculation; recomputing all metrics
per result (cost grows with the tournament on the hottest path); ranks stored
and sorted after every result (most of a replay, for standings nobody reads in
between); a cache of the standings in the phase (interior mutability, for a
sort of 0.08 ms); computing the metrics when read as well (most of the
remaining replay, but every calculator would be rewritten from its incremental
updates).

## Scores

Points and the tiebreaks built from them are compared, sorted and hashed, and
must be identical after a replay on every platform.

**They are `Score`, a finite `f64` with a total order**: never NaN or infinite,
`-0.0` made `0.0`, so `Eq`, `Ord` (`total_cmp`) and `Hash` (the bits) agree; a
non-finite result panics.

- **Deterministic**: `+`, `-`, `*` are exactly rounded by IEEE 754 natively and
  in wasm; scores use no transcendental functions.
- **Exact for what is played**: scoring systems count in halves or quarters,
  which `f64` holds exactly. A field that must rule out values like `0.1`
  narrows itself with `#[plugin(multiple_of = 0.25)]`.
- A wrapper was needed anyway, since `FieldType` and float types are both
  foreign to the engine.

Rejected: plain `f32`/`f64` (no `Eq`/`Ord`/`Hash`, NaN); `OrderedFloat` (orders
NaN instead of ruling it out); an `i32` of quarters (own parsing, display,
serialization, rounding of products); `rust_decimal` (16 bytes, slower);
`fixed` (lossy display and parsing, odd serde form); rationals (gcd and
cross-multiplication on the hot path).

## Ranges of numbers

A numeric field states its numbers in the schema, so a UI can show and check
them before anything is built.

- `FieldKind::Integer(IntegerRange)` always has `min` and `max` (the type's
  bounds unless narrowed) and `multiple_of` (1 unless narrowed). A bound is
  *stated* if chosen for its meaning, else only a *limit* (`u32::MAX`); a UI's
  description names only stated ones. `FieldKind::Decimal(DecimalRange)` has
  each optionally.
- **`multiple_of` is anchored at 0**, as JSON Schema's `multipleOf`; `min` and
  `max` are multiples. For decimals it is a whole number or a power of two, so
  `%` checks it exactly.
- **The type declares, a field narrows** (`FieldType::kind`, then
  `#[plugin(min, max, multiple_of)]`), never widens. An impossible narrowing or
  a default out of range panics when the schema is built.
- **Checked by the schema and by the type**: `FieldKind::walk` rejects with
  `ConfigError::OutOfRange`, and the type's `Deserialize` still checks, since
  logs and snapshots load without a schema.

Rejected: a step anchored at `min` (HTML's `step`: depends on two declarations,
inexact for decimals); checking only in `Deserialize` (late, and tells a UI
nothing).

## Consumer data on entities

Consumers attach data core never reads (a table, a database id, a club).

**It lives in a `TypeMap` field on the entity, and enters through the log at the
transition where it becomes known.** Determinism forces the second half: a
factory at creation could only compute derivable data, which needs no storage.
A field on the entity (not a side map) makes it reachable from a bare `&Match`
(`r#match.annotation::<TableNumber>()`).

- **Entrants** are written by `AddEntrant`
  (`Entrant::new(id).with_annotation(..)`).
- **Matches** are annotated by the format when scheduling
  (`NewMatch::new(..).with_annotation(..)`). Consumer annotations of a match
  need a second step, *planned* as an `AnnotateMatch` transition; the `Option`
  on lookup is honest, as the annotation does not exist at birth.
- **Rankings** get nothing: data fixed at the start derives from the entrants,
  data evolving with results is a metric.
- The cost is accepted: an empty `TypeMap` (56 bytes) does not allocate.
  `TypeMapValue: DynClone`, since `apply` clones its payload on every replay.

**Personal data stays out of the log.** Names and contacts must be deletable,
and a log replays as written. So they live in `People`, keyed by `EntrantId` and
saved next to the log, keeping everyone ever entered, since undo can bring them
back. Renaming an entrant changes only its `Person`, at any time; being in no
log entry, it is no action of a session's undo.

Rejected: a sparse `Map<MatchId, TypeMap>` on `Phase` (unreachable from a bare
`&Match`); consumer side-tables in general (in neither persistence path, not
undone); a core `name` field (no principle to refuse the next field); generic
entities (below).

## Why entities are not generic

`Competition<E, M, R>` with the library working through traits is the natural
idea, and fails on three blockers verified against `typetag` 0.2.23's source:

1. Generic entities make `PhaseData<M, R>` and so every plugin trait generic,
   and `typetag` rejects generic traits for deserialization
   (`tagged_trait.rs:7`), so `Box<dyn PhaseFormat>` would lose `Deserialize`.
2. Reusable third-party plugins need generic impls
   (`impl<M: MatchLike> PhaseFormat<M> for RoundRobin`), which `typetag` rejects
   too (`tagged_impl.rs:7`).
3. A generic `AddEntrant<X>` cannot be registered for the log, so core could
   ship no loggable transitions.

One `Extension` parameter with associated types per entity avoids 1 and 2 but
not 3, buys only non-optional access where `Option` is honest anyway, and cannot
compose: a CLI, a web layer and a third-party scheduler could not annotate
independently. Generics would only decide the *type* of consumer data, never how
it arrives, which is the actual constraint.

## Core-owned identity

Ids are deterministic so replay reproduces them: `MatchId`, `RankingId` and
`ByeId` are UUIDv5s of their content (the rankings, plus the round for a match
or bye), so a match is identified by who plays it in which round.

UUIDs are no good for human input, so a match also has a number:

- **A format schedules a `NewMatch`; the phase stores a `Match`** with a
  `number: NonZeroU32`, so no reader needs a fallback.
- **Numbered in scheduling order unless the format asks**
  (`NewMatch::with_number`, unique within the phase, checked in debug builds).
- **A bracket numbers its pairings from the start** (`BracketPairing::number`,
  by round, skipping pairings that can only be byes, worked out from the tree on
  every call rather than stored), so a semifinal keeps its number whichever
  quarterfinal ends first, and a side not scheduled yet reads "Winner of 9".
- **Not part of the identity**: `MatchId` excludes it, so old logs replay and
  get their numbers on replay. Numbers are per phase.

Rejected: the insertion index (a knockout's match not scheduled yet has none to
be named by); a number annotation (every reader needs a fallback); a
`PhaseFormat::match_number` method (format code at every reader); the number in
`MatchId` (breaks saved logs).

## Rounds and stages

**A round is a scheduling slot, nothing more.** `Match::round` is a
`NonZeroU32` with a contract: a ranking plays at most once per round, and its
later rounds are played after earlier ones. Core relies on that alone (the
check, `MatchId` telling rematches apart, metrics like `RoundOfFirstLoss`).

**Stages are a typed annotation written by the format** (e.g.
`KnockoutRound::RoundOf(8)`, `KnockoutRound::ThirdPlace`), since the round alone
cannot tell the final from the match for third place, or a winner-bracket from a
loser-bracket round. Consumers name a stage through `PhaseFormat::stage`, a
`Stage` (a `Phrase`: key, numbers, English `Display`) they translate. Formats
without stages attach nothing.

Rejected: names as strings in core (language-dependent, an untyped bag); a core
enum of round kinds (closed); a format-defined round type (unrelated types to
order, virtual calls on the hot path); consumers interpreting the round number
(cannot tell stages sharing a round).

## Byes

A ranking without a match in a round (odd field, a bracket not full, sitting a
Monster DYP round out) often scores anyway, and only the format knows whether a
missing ranking has a bye, was knocked out or is not scheduled yet.

- **A bye is an entity of its own, scheduled by the format**: `Bye` holds its
  ranking, round and annotations; the format returns `Schedule { matches, byes }`
  and the phase stores byes apart from matches. A ranking is in at most one
  match or bye per round; the bracket formats copy a pairing's annotations onto
  its bye, so its stage can be named.
- **A bye counts once its ranking reaches it** (`Bye::is_reached`: no open match
  of the ranking in an earlier round), as if it were a match reported as soon as
  playable. For a format scheduling round by round that is when it is
  scheduled; for a round robin, which schedules everything at once, later, so
  the standings never count a bye of a round the ranking has not reached. Then
  `MetricsCalculator::update_after_bye` runs. After a result only the byes of
  that match's rankings are checked.
- **`update_after_bye` has no default**: whether a bye counts differs per
  metric (points: configurable; rounds of loss: no; Buchholz: the bye's points
  count for opponents), and the compiler makes every calculator answer.
- **Scheduling a bye does not schedule again**: the format knew of it when it
  returned it.
- `WinsDrawsLosses` counts byes (not as matches played); `PointsCalculator`
  scores them with `points_for_bye`.

*Not covered yet:* kinds of byes worth different points (requested half-point
byes). Forfeits are not byes but match results.

Rejected: one `Match | Bye` enum for calculators (an arm that never runs); a bye
as a match without opponent (every calculator silently wrong); a phantom bye
ranking (gets metrics and a rank); inferring byes from gaps; the format stating
a bye's worth (the points calculator's concern); a default no-op hook; counting
a bye when scheduled (a round robin's odd field led with future byes); hiding
unreached byes only in the standings (metrics stay wrong).

## Match details

A result is more than who won:
a score (21–17), games of a best of three, rubbers of a league fixture each with
games of points, legs of a relay whose score carries over. Some rankings read
these details (goal difference, sets won, fair-play cards), and a UI must show
and enter them for sports it does not know.

**One generic shape, interpreted by a plugin.** What was played is a tree of
`Part`s with a fixed shape; what it means is the business of the phase's
`MatchFormat`, the `match_format` of `PhasePlugins` (default `WinnerOnly`: who won,
nothing more):

```rust
pub struct Part {                      // a match, or a game, set, rubber, leg of one
    decision: Option<Decision>,        // Some ⇔ decided
    values: Vec<(FieldKey, Sides<Option<EnteredValue>>)>, // goals, games, cards, …
    parts: Vec<Part>,                  // in playing order, as deep as entered
    annotations: TypeMap,
}
pub struct Decision { result: MatchResult, reason: Option<ResultReason> }
pub enum EnteredValue { Count(u16), Decimal(Score) }  // durations when a sport needs them
```

- **A match is a part**: `Match::contest`, holding the match's annotations too.
  A match is finished if and only if its decision is `Some`; finished and open
  matches have one type. `ReportMatchResult` carries a `Decision` (not an
  `Option`), so reporting always decides; the decision is set once, and
  formats and rankers never see a result change (a correction undoes the
  report, as before).
- **The decision is stored, not derived.** The log says who won, so replay does
  not depend on a plugin's reading of the values, and changing a plugin cannot
  change a saved winner. A part without a winner of its own (a relay leg, an
  unfinished set) has none. `ResultReason` names a decision not reached by
  play (walkover, retired, overtime).
- **The engine trusts what is reported.** 25–21 in a game to 21, or values
  disagreeing with the decision, are accepted: the referee is the authority,
  and rules vary from tournament to tournament. The engine refuses only what it
  cannot use: a draw where the format allows none, a field, reason or kind of
  value the template does not declare.
- **The match format** gives per match a `Template` (levels with their names,
  their parts, fields with kind and `Hints` like the winning score, reasons),
  `suggest`s a result from a part's values for a consumer filling one in (the
  app's `ResultForm`), and derives **tallies** when a match is decided: typed
  `TypeMapValue`s (`PointsScored`, `GamesWon`, …) stored on the match, which
  metrics read with one lookup (`PointsBalanceCalculator`,
  `GamesBalanceCalculator`). A tally is a fact in the compatibility check
  (`MatchTally<T>`): a calculator requiring `PointsScored` needs a match
  format providing it. The template sees the
  match, so it may depend on the stage or on annotations. Match formats:
  `WinnerOnly`, `Scored`, `BestOf`.
- **Scores are shown generically**: a match's own score, else those of its
  parts in order (`MatchRow::scores`), after the parts each side won of a
  match played in parts at one level (`MatchRow::parts_won`: 2–1 in games),
  counted from the parts' decisions rather than read from a tally, so that a
  form shows them before the match is decided too, and a warning of parts
  entered after the match was decided says where it was (3–0). Always in the
  order of the sides, as the scores beside them, also after who won (Bob wins
  (1–3)), rather than the winner's first. A `MatchFormat::show` for a sport
  writing its scores its own way (tennis: 40–30, 7–6⁽⁵⁾) waits for such a
  sport.
- **Count limits**: entered counts and winning-score hints are `u16` (0–65535).
  Point tallies and points/games balances use `u32`; accumulated totals saturate
  at `u32::MAX`, identically in checked and unchecked builds. Differences use
  those saturated totals.
- **Hints are not rules**: the app turns them into warnings ("23–19 in a game
  to 21?"), never refusals.
- **The winning score is what a winner has, not a maximum**: in a game to 21
  the winner has 21, or more when winning by one or two (22–20, 30–29). Both
  sides below it, or a lead other than one or two from it on, is warned of.
  Whether a game needs a lead of two, and where it is capped, is not
  configured: the warning takes either, which covers sports and tournaments
  varying it game by game (a fifth game won by two where the others end at
  5–4). A draw needs no check of its own: with a winning score every drawn
  score is warned of, and without one (football) draws are usual.
- **A score far below the winning score completes the other side**
  (`ResultForm::complete_score`): with at most two below it (19 in a game to
  21), the winner can only have the winning score itself, as more would lead by
  three or more. From one below on it cannot be told (20: 21–20 or 22–20; 21 or
  more: either side may have won), so nothing is filled in. The GUI completes
  a value once it is left, not while it is typed (the 1 of 15 would complete
  it too early), only when the other side is empty, and selects what it filled
  in when Tab moves there, so that typing replaces it. Tab completes before it
  moves, and moves only once the page shows the score: else it would pass a
  game the completed score makes the match need, which appears after it.
- **What a field cannot take stays as typed** (`ResultForm::enter`): a count
  above 65535, below 0 or with a fraction, read as the phase form reads a
  number (comma or point, `1e3`), leaves the side without a value but shown
  with why (`issue-out-of-range`), and keeps the match from being reported
  (`ReportError::NotTaken`). The form keeps the text, not the GUI: values
  commit on input, and Dioxus writes an input's value whenever the one
  rendered changes, so a value dropped to none would empty the field at the
  digit that makes it too large. Nothing is completed onto such a side, as it
  is not empty. Rejected: refusing the keystroke (a `type=number` input has no
  selection to tell where a key went, and reverting the text from script says
  nothing to a screen reader). The inputs stay `type=number`, keeping the
  browser's own check on submit; text the browser cannot read as a number
  (`1e`) reaches the form as nothing, which the browser flags on submit.
- **Field keys and reasons are strings** (`Cow<'static, str>`), declared by the
  template and checked on report, as a config's field names are: a generic UI
  needs keys it can list. What the engine reads, the tallies, stays typed.
- **Counts, not events**: cards or tries that a ranking reads are fields. A
  timeline of goals, assists or a chess game's moves is no ranking's business
  and stays outside, keyed by match id, or in annotations.

- **A result in one row where it fits** (`ResultForm::quick_entry`): read from
  the shape of the template, not from a list of match formats or a flag one
  declares: a match with one field and no parts (a score), or with no field of
  its own and parts of one level with one field each (the games of a best of).
  The GUI enters it below the pairing, its parts side by side and wrapping on a
  phone rather than dropped there; the dialog is a button away (*Details*,
  starting from what the row holds) for overriding, reasons and what a row
  cannot hold, such as a fixture's rubbers. The row never chooses a result: it
  reports what the values suggest, or says that they decide nothing.
- **Best-of completion counts decisions, including draws**: a side wins early
  when its lead in games won exceeds the games still undecided. Otherwise all
  configured games are played, and equal win totals suggest a draw (subject to
  the phase format allowing match draws).
- **A best of takes at most 99 games**: its form shows the fewest games that
  may decide the match from the start, more than half of them, so a mistyped
  number of games would hold the page up. The longest played, a snooker final,
  has 35 frames.
- **Parts appear as the match needs them**: `Parts::Same` says the fewest parts
  that may decide a part (`min`, two of a best of three), and the form shows
  those, the parts up to the last one entered, and one more while each shown is
  decided and the match is not, in the row and the dialog alike. It never hides
  a part entered, and warns of parts entered after the match was decided
  rather than refusing them. It makes entries only for the parts entered and
  views only for those it shows, so a large `max` costs nothing before it is
  reached. Values commit on input, so the next game is there before Tab moves
  on. Rejected: every part at once (a best of five shows five
  boxes for a 3–0, as Challonge does), one part at a time behind an *Add game*
  button (a tap per game, and start.gg's fixed best of three makes adding
  games by hand a chore), and moving the focus to *Report* once decided as
  ezBadminton does (typing 21–17 decides the game at 21–1, so the focus would
  jump away mid-score; ezBadminton avoids it by filling in the winner's
  score, a guess the row does not make).

- **A finished match is shown as reported** (`Session::reported_result`): the
  view of a `ResultForm` rebuilt from the match's contest, a result chosen
  where the values do not suggest it, so that a result reported against them
  shows with its warning. The GUI shows it read-only in a dialog opened by a
  click anywhere on the finished match, in the list or the bracket, but its
  *Reopen*: a button without text covering the card, named for screen readers
  ("Details of the result of match 1"), its focus ring around the card. On
  every screen alike, rather than in a second row on a phone: a finished match
  keeps to one row, and a fixture's rubbers would not fit one. Its parts are a
  table, a row each, the winner's values marked with ✓ as in the list of
  matches, rather than a column naming the winner, which long names would push
  off a phone. What shows that a finished match opens is its score, underlined
  as a link is (opening changes nothing), in the list, the bracket and the
  cross table alike, since touch has no hover to tint it; its colour stays, so
  that finished matches still recede. A match showing no score is not marked:
  it opens to no more than who won. Rejected: a *Details* label or an icon by every
  finished match (room in a row kept to one line), a chevron (the card steps
  through rounds by chevrons).
- **A reopened match starts out at its result** (`Session::result_form`), so
  that a typo in one game of a best of five is corrected in that game, still
  by an undo and a report. The session keeps what was reported for each match
  whose result it took back (reopening, undoing the report, redoing a
  reopening), the latest of each, as it keeps the configuration of a phase
  whose adding it undid; the form starts out at it while the match format
  takes it (`Level::check`). A result chosen against the values stays chosen:
  the match takes no row, which holds values only (`ResultForm::quick_entry`),
  and the dialog opens overriding. Scores typed in are the whole result and
  replace it (`ResultForm::enter_scores`). Catching up drops what the session
  kept, as another session may have reported and reopened the match since.
  Rejected: reading the result from the undone report in the log (it would
  come back after a reload or in another tab, in a form nobody reopened, and
  every form would search the log).

*Planned:* lineups on parts (with rosters of players), league fixtures nesting
the match formats of their rubbers, live reporting (an `UpdatePlay` on open
matches carrying the whole play, a plain assignment no format or metric reacts
to).

Rejected: a plugin-defined details type (metrics and UIs could not read it
without knowing the plugin); events (no ranking needs more than counts); an
optional decision on a reported match, or one derived from the values (replay
would depend on plugin code); a separate type for the match level (two places
for the match's decision); checking the rules of a sport in the engine; the
name `MatchRules` (the plugin describes the shape of a match, as a
`PhaseFormat` does of a phase, not the rules of a sport).

## Persistence

### Snapshots

`Competition` and everything in it serializes through `typetag`: each trait
opts in once, each impl names an explicit namespaced tag (`#[axis]` and
`#[register]` do it for plugins, with the plugin id):

```rust
#[typetag::serde(tag = "type", content = "value")]
pub trait TypeMapValue: Any + Debug + DynClone {}

#[typetag::serde(name = "tournament_engine::eliminates")]
impl TypeMapValue for Eliminates {}
```

A metric's tag may equal its calculator's id, since every trait has its own
registry. Two findings from `typetag`'s source:

- **Adjacent tagging is required for non-self-describing formats** (postcard).
  Internal tagging serializes the content through an adapter claiming
  `is_human_readable() == true` while deserialization sees the real format, so a
  `Uuid` is written as a string and read as bytes. (Upstream bug, reported
  against 0.2.23.)
- **A tag collision fails loudly** on deserialization ("non-unique tag").

`IdMap` and `TypeMap` serialize as plain sequences with keys omitted (each value
carries its key); sound because both wrap an `IndexMap`, so insertion order
round-trips. Tags are strings, so keep them short.

### The transition log

The log holds a `CompetitionTransition`, an enum of the engine's transitions,
each written under the name of its variant as `{<variant>: <transition>}`
(serde's external tagging):
`CompetitionStateMachine = StateMachine<Competition, CompetitionTransition>`.
The transitions are a closed set: plugins are the extension points, and a
consumer recognizes what an entry did by an exhaustive `match` rather than by
downcasting. Until format 3 the log held a `typetag` trait object, written
`{"type": <tag>, "value": <transition>}` under a namespaced tag
(`tournament_engine::add_entrant`); a converter rewrites it. A variant's name is
its tag: no namespace, since nothing outside the enum shares the name space,
and renaming one needs a converter, which the pinned names in
`tests/transition_log_serialization.rs` make deliberate.

Rejected: a `typetag` trait open to third-party transitions (none has been
needed, and every consumer recognizing an entry would downcast with a
fallback); serde's derived adjacent tagging, which kept the old layout but
reads the tag as an identifier, which postcard does not support; serializing
the enum by hand in the old layout (a serializer of its own, to save one
converter).

With `undoable-state-machine`'s `serde` feature the machine
serializes as its log, undo entries included. Deserializing replays it
(`StateMachine::build`) rather than trusting a stored state, so the state cannot
drift from its log, and a log that no longer replays fails to load.

A snapshot loads without replay but holds only the current state; a log keeps
how it came about, so a result reported before a restore can still be undone.

### The competition next to its log

A log replays only as long as every transition and plugin behaves as it did:
a changed pairing leaves later results naming matches that no longer exist, a
fixed metric changes the standings of a finished competition. Keeping every
version of every behaviour is not sustainable, and not wanted where an old one
was a bug. So a file holds the competition its log replayed into next to the
log, and so does the browser's storage (which stores the file), for a version that can no longer
replay it to show it read-only instead (*planned*), and to tell a replay that
comes out otherwise. The log stays what
is loaded; the competition is only read in place of it. Its serialized form is
a persistence contract too: a change of it raises the file's format and adds a
converter, as one of the log does (see [Golden saves](#golden-saves)). Holding
it did not raise the format, since older versions ignore it. It still
holds the rank and unique rank of every ranking and the order of the standings,
as when they were stored: a phase computes them when it is written, and reads
but drops them when it is read (see [Metrics](#metrics-plugin-produced-ranking-data)).

Rejected: every version of every transition (unbounded, and keeps old bugs);
only the competition (no undo, no history); a smaller, display-only archive
instead of the competition (cannot be continued from or compared with a
replay).

### Tag stability

A tag is a persistence contract, not a name: renaming or moving a type must not
change it, so tags are written out and pinned in tests, for plugins, metrics
and annotations alike (a transition is named by its variant, see
[The transition log](#the-transition-log)). Since v0.1.0 saved competitions of real users
depend on them: a tag may be added, never changed or given another meaning.

The namespace is the crate defining the type (`tournament_engine::points`), not
the product: published as Tournamo, the tags stayed, since a crate's tags
should not change with what an application built on it is called. Rejected:
renaming them to the product's namespace before the release (churn in every
tag, plugin id and Fluent key, for a name the crates do not carry).

Rejected: `TypeId` (stable only within a build); a hand-rolled binary tag
registry (reimplements `typetag`); auto-derived tags from the type path
(silently change on a rename).

### Golden saves

Stable tags keep a log loading, not playing the same: a changed format, metric
or check replays an old log into another competition, or not at all, and every
round-trip test still passes, since it saves and replays with the same code.
So each release commits saves of a few scripted competitions (`tournament-app/tests/saves/v<version>/`:
every format, byes, undo, redo, reopened results, edited phases), each the file
that version exports next to the competition it held, serialized. Every later
version must load them, replay each into that very competition and still show
it. Saves are never rewritten: a failing one is either a bug, or a deliberate
change of how old competitions are shown, which drops the save and goes into
`CHANGELOG.md`.

The serialized competition is the expected result, compared as JSON since
`Competition` has no `PartialEq`, so a test also catches nondeterminism (e.g.
iterating a `HashMap`) and differences between native and wasm.

**Older layouts are converted, then read by the current types.** A change of
what is saved raises the file's format (`file::Format`) and adds one converter
from the format before to the chain in `tournament-app/src/file/convert.rs`:
a function on the untyped JSON of a log entry and one on that of the
competition, each writing literal JSON of its format, never through the
engine's types, which keep changing. Reading a file of an older format applies
the converters one format at a time and only then deserializes; a file of the
current format is read directly. So a change costs one small function rather
than a reader in every changed type, listing all its fields again; the engine's
types read only the current layout, and no old struct is kept. The crate does
not build while a format lacks its converter, and a converter, like a save, is
never changed once released. The golden saves of older versions are what shows
that the chain reads them. Only a file is versioned: a consumer that serializes
a bare `Competition` or log by itself reads back only what the current version
writes.

Rejected: typed readers per changed type (each repeats all its fields, and
they live in the engine), a crate for versioned structs (keeps every old
struct).

## Progression as an undoable state machine

Progress is a sequence of transitions on an initial state, through the generic
`undoable-state-machine` crate. Undo replays the still-active entries from the
initial state, which is why determinism is global: it is why ids derive from
content, consumer data arrives as transition data, and plugin state needs no
undo of its own. `Transition` keeps `check_applicable` and `apply` separate, and
`Competition`'s mutators are crate-internal, so state changes only through
logged transitions.

### Undoing a change by what it did

`CompetitionStateMachineExt::undo_match_result` takes a phase and match, finds
the latest entry in effect that reported it and undoes that, so an operator
correcting an old result does not walk back through everything since. It lives
in the engine because it knows the shape of the log.

- **Found by its variant**: the latest `CompetitionTransition::ReportMatchResult`
  in effect for the match.
- **Refused, not cascaded,** when a later entry depends on it (a final reported
  after the semifinal): the state machine names the failing entry and the log
  stays unchanged.
- **No new transition**: an undo entry, itself undoable.

### Changing a phase after it was added

A consumer reads `Phase::config`, puts it into a draft (`Draft::with_answers`),
lets the operator change it, and hands the built plugins to
`CompetitionStateMachineExt::reconfigure_phase`, which appends a
`LogEntryKind::Replace` of the phase's `AddPhase` with one of the same phase id
(found by downcasting), unless nothing changed.

- **A replacement, not a transition of its own**: everything after it replays on
  the changed phase, results staying while it still schedules their matches,
  metrics recalculated rather than patched. A change a later entry does not fit
  is refused. Changing in place would make every plugin migrate state from
  another configuration.
- **The config is serialized from the state, not stored**: a plugin serializes
  as its answers plus skipped state fields, which `Config::of` drops by the
  schema. So snapshots can be reconfigured too. Rejected: `AddPhase` holding the
  config and building on every replay (built twice, and non-self-describing
  formats cannot read a JSON value back).
- **A format drawing at random draws what does not depend on a field first**, so
  a change keeps what it can: random rounds shuffles every rotation and takes as
  many as it has rounds, so rounds can be added after results.

### Undo and redo of a session

`undo`/`redo` work as in an editor: they undo what the operator did, not what
the log ends with. `undoable_state_machine::SessionHistory` keeps the ids of the
entries the session appended as actions (any change, including a targeted undo
and a reconfiguration); `undo_last` undoes the latest in effect, `redo_last`
redoes what it undid last while no action was taken since, each by an ordinary
undo entry naming its target. Operations appending an action take the history
(`apply_in`, `undo_match_result`, `reconfigure_phase`).

- **Per session, outside the log**: with a shared log, an operator's undo must
  revert what they did, not what was pulled in or done in another session. A
  loaded competition has nothing to undo; a single result can still be undone
  by `undo_match_result`. The GUI says so under the pointer on Undo while it
  has nothing to undo, since a reload or another tab taking over loses it.
- **A targeted undo is an action**: after `undo-report`, `undo` brings the
  result back.
- **Explicit targets**, so an undo means the same however logs are merged.
- **Changes of other sessions leave the history as it is**: what this session
  can undo and redo stays after catching up with them.
- **Fails only because of other sessions** (a dependent change, or the action
  undone or redone elsewhere); then it fails rather than skipping the action,
  and that direction ends there: a failed undo leaves nothing to undo, a failed
  redo nothing to redo, while the other direction stays. `next_undo` and
  `next_redo` do not offer an action undone or redone elsewhere, which is
  cheap to tell; whether a change depends on it shows only by trying, which
  replays the log.
- **The history holds ids, the log says what they did**: `Session` describes
  an action (`Action`, for the announcement) from its log entry, an exhaustive
  `match` on `CompetitionTransition`, rather than keeping descriptions next to
  the history. What a form starts out at after an undo (the phase config, a
  result taken back) is kept apart, since it outlives what can be redone.
- **The seed is no action**, applied outside any history, never undone.

Rejected: deriving the history from the log (`undo` after `undo-report` undid an
unrelated change); dedicated undo/redo log entries (record navigation, not
whose); an origin per user (a user with two sessions); persisting the history
(not part of the competition); ending the history when catching up with another
session (a result reported elsewhere would keep the operator from undoing their
own); keeping an action to try again after it failed (it fails again until the
other session's change is undone).

## Dependencies this implies

- `typetag` (with `inventory`, `erased_serde`) and `serde` for trait objects;
  `postcard` and `serde_json` in tests for a non-self-describing and a
  self-describing format.
- `plugin-factory` (with `inventory`) for axes, catalog, schemas, drafts and
  compatibility; `dyn-clone` for boxed plugins and values.
- `indexmap` for deterministic order, `rustc-hash` for small id keys; `uuid`
  (v5); `rand` and `rand_seeder` for seeded shuffling; `vec1`.
- No `enum_dispatch`, `linkme`, `ambassador` or `intertrait`: each served only
  an alternative ruled out above.

## Frontend

The GUI an organizer runs a competition with, in the browser and offline
(`requirements.md` says what it must do). `tournament-web`,
the CLI's command line in the browser, stays a thin text layer; the GUI is a
separate app, not built on it.

### Framework

**Dioxus** (0.7), rendering real DOM, built with `dx`.

- **Real HTML**: WCAG 2.1 AA, print, embeds and phones need the browser's
  accessibility tree, print styles and text.
- **Readable code**: components are plain functions of the rows they get,
  re-run and diffed.
- **Fast where it matters**: updating long lists was 1.5–2× faster than Leptos
  (see [Performance](#performance)).
- Desktop and mobile stay possible with the same components (a PWA for now).
- **Plain CSS** (`assets/style.css`), no Tailwind: no extra tool, a small page.

Rejected: **Leptos** 0.8 (spiked alongside Dioxus on a shared core, commit
`0902482`): loaded slightly faster, but reporting a result took 160–200 ms
against 75–90 ms, and `view!` with memos read worse than `rsx!`. **Canvas GUIs**
(egui, iced, Slint): no accessibility tree, print or browser text.

### The app crate

**A UI-free crate between engine and its interfaces** (`tournament-app`) owns
the operator's session: competition operations, undo history, names (`People`),
storage, forms, data for screens and messages. Both the GUI and the CLI use its
`Session`, which returns typed data instead of text and builds natively and for
wasm, so the operations and what a screen shows are tested without a browser.

The CLI's session wraps it with command parsing, English tables, and explicit
whole-file saves. It does not own another state machine, people map, or undo
history. Action errors come from the app; errors specific to commands, match
numbers, file I/O, and configuration questions belong to the CLI. Its tables
keep their own layout and column order.

`file::File` is the shared serialization envelope. `Session::file` borrows the
log and people for export; a decoded file opens through `Session::from_file`
with no undo history. The CLI omits the optional name and tracks its last saved
log length, keeping its explicit save/exit policy independent of the app's
incremental storage. Neither file path persists history.

Adding several entrants validates the whole batch before appending any entry;
each entrant remains a separate undoable action. The batch API keeps names as
supplied, including spaces inside the CLI's quotes. The single-entrant API
trims input for the GUI. Both share validation and the logged operation.

- **An error type per action**, so a GUI handles exactly what its action can
  return; engine errors are recognized by downcasting, anything unknown is
  `Rejected` with the engine's message.
- **Names are `Option`s** in rows: a log from elsewhere may lack them.
- **The session keeps the actions it did** (`view::Action`, with rows and names
  as they were), so `undo`/`redo` can say what they walked through without
  rebuilding rows from a changed state.

Rejected: in the engine (mixes the log with one screen's needs); in the GUI
crate (testable only in a browser); the CLI's `Session` as it is (answers with
text).

### Storage

**IndexedDB, one record per competition: its file** (see below), without the
name, rewritten whole with each change as it is accepted, so no accepted result
is lost. One format for files and the browser means one thing to keep loading
(golden saves cover both) and one to migrate. **One writer tab per
competition**, holding a Web Lock; other tabs are read-only and follow it
through a `BroadcastChannel`.

- **The `Storage` trait is async** (IndexedDB is), returning `impl Future`
  without `Send`; so not dyn-compatible, and the session takes the storage where
  it loads or saves rather than being generic over it. It knows records (the
  text the session hands it) and listings (key, name, progress).
- **The session changes in memory first; storage catches up.** A transition is
  known accepted only once applied, and the log cannot take an entry back. A
  save is take (`to_save`), write (no borrow of the session across the
  `await`), mark (`mark_saved`); a failed write leaves its changes for the next,
  which writes everything anyway. The session counts its changes (log entries
  and people), so a save marks only those made before it was taken, whatever
  order writes finish in. A storage stores writes in the order they are made:
  IndexedDB runs read-write transactions over the same stores in the order they
  were created.
- **The listing carries the progress** (not started, the round being played,
  won by whom), written with each save, so the list need not replay or read
  every competition. It is a record of its own, since every list and every
  write reads all listings.
- **What the log replays into is in the record** (see [The competition next to
  its log](#the-competition-next-to-its-log)). A competition an older version
  saved lacks it; the session counts that as unsaved, and the tab changing it
  saves right after opening it (not a read-only tab, which catches up
  regardless).
- **A write is one transaction** over the listings and the records. Opening at
  a higher version closes older pages' connections (`idb` does so on
  `versionchange`) rather than waiting for them; their next write fails. A
  transaction stays active only in the microtasks after a request's result, so
  it runs on wasm-bindgen-futures' executor, not the awaiting one. The crate
  `idb` binds IndexedDB (thin, few dependencies).
- **Older databases are moved over lazily.** Up to the database's version 2
  (0.2.0) a competition was stored as records of each log entry and person
  (`entries` under `[key, position]`, `people` under `[key, entrant]`) plus
  `replayed` under the key. Reading a competition without a record puts its
  file together from those; its next write stores the record and removes them.
  So no upgrade has to read and write everything inside IndexedDB's
  `versionchange` transaction, and a competition that no longer loads stays as
  it was.
- **Cost.** Measured in Chrome (wasm, desktop CPU) with the benchmark's
  competition after its last result (log and competition 3.5 MB of JSON at 1000
  entrants, 7.0 MB at 2000): a save takes ~28 ms at 1000 entrants and ~54 ms at
  2000, of which serializing is about two thirds and the write the rest; before,
  with records per entry but the competition rewritten anyway, ~18 and ~34 ms.
  A mid-range phone takes about four times as long for the serializing.
- **The lock** is held by the tab with the competition open and by each save
  under way, so a change made just before leaving is written before another tab
  takes over; the save outlives the screen. Web Locks are bound by hand, since
  web-sys has them only behind `web_sys_unstable_apis`.
- **Catching up appends rather than reloads**: a session with N entries reads
  the record, reads its log entries without replaying them, and applies those
  from N on, replaying only for undo or replace entries, as the writer did. It
  refuses a session with unsaved changes (the log would fork). The same will
  serve syncing with a server.
- **Persistent storage is asked for when a competition is opened** (some
  browsers ask the operator, so not on load); a refusal shows a warning, see
  [Offline and installing](#offline-and-installing).
- **Files are the CLI's**: `{"format", "log", "people", "competition"}` plus `"name"`, so
  competitions move between CLI and GUI. `"format"` (`file::Format`, first, a
  missing one is 1) is raised with every change of what is saved; a newer one
  is refused as such rather than as a broken log, and older ones are converted
  when read (see [Golden saves](#golden-saves)), a record in IndexedDB as well.
  Importing makes a new competition under a new key, numbered (`Cup (2)`) if
  the name exists (a number, not a translated word in stored data). Exporting
  from the list writes the stored record as it is, with the name, so a log that
  no longer replays can still be exported. Files are not compressed: gzip would
  make them about 3.5× smaller, but compressing takes longer than serializing
  (in Chrome `CompressionStream` at its default level ~170 ms for 7 MB,
  blocking the page; `flate2` at level 1 ~50 ms). It can be added later without
  breaking older files, which a reader tells apart by gzip's first bytes
  (`1f 8b`).
- **The competition open is in the URL** after `#`, and a fragment naming none
  shows the list saying so. Renaming changes only the
  listing (the name is not in the log); deleting removes everything in one
  transaction, under the lock, so it is refused while a tab holds it open.
- **Look-alike entrant names are refused** (case and NFKC,
  `Session::entrant_like`), and pointed out while typed: two entrants a list
  tells apart only by case are a typo far more often than two people.
- Without the APIs (plain HTTP away from `localhost`) every tab writes and
  persistence is not asked for.

Rejected: a synchronous trait with the GUI writing IndexedDB itself (record
format untested outside a browser); the session owning its storage (borrowed
across every write); a record per log entry and person (a second format next to
the file's, saving little, since the competition is rewritten with every save
anyway); postcard instead of JSON (a save 3–4× faster, ~16 ms at 2000 entrants,
and 2.7× smaller, but not self-describing: a field added with a default, one
left out or variants reordered no longer read, so every change of a saved type
would need a frozen copy of the old one, and a forgotten one may read as wrong
values rather than fail); saving in a Web Worker (the session lives on the
page's thread, so the data would have to be serialized to get there); several
writer tabs (forks the log: that is the sync problem).

### Configuring a phase

**Suggestions belong to the tournament layer.** `PhaseFormat::suggestions` returns
`PhaseSuggestions`: separate lists of ranking criteria and metrics, with any
suggested settings. `PhasePlugins::default_value` offers those answers for the
root ranking/metrics fields; `with_suggestions` fills unanswered sections while
preserving explicit choices and filling schema defaults. The form and the phase
questionnaire use these operations. Third-party formats implement the same trait;
consumers need no table of formats. A future match type can be combined at this
boundary, without introducing cross-plugin negotiation in the generic library.

`plugin-factory` offers only schema defaults. It does not collect sibling
recommendations, select their destination by axis, or filter them for compatibility.
Suggestions are ordinary answers: a suggested combination that is incompatible
remains visible and must pass the same validation as an operator's choices.
Implied fieldless dependencies remain a separate generic operation.

**The form is data of the app crate**: `PhaseForm` holds the `Draft` and hands
out a `FormView` (fields by `Path`, inputs by kind, options, implied plugins,
issues), taking changes by path. The GUI only lays it out and knows no plugin.
It serves adding a phase and editing it at any point (`reconfigure_phase`).

`Draft::fields` enumerates the schema and answers together: each field has its
schema, current answer, and chosen plugins, and each plugin has its fields.
Readers descend through these nodes instead of resolving each field from the
root. The draft's compatibility tree and the app's form use this enumeration.
It includes unanswered fields and adds neither defaults nor implied plugins.
The form enumerates a copy with implied plugins to distinguish them from choices.

The enumeration is an owned snapshot, rebuilt after edits. Nodes carry paths for
checked edits and diagnostics. Paths are addresses in the current structure, not stable ids
across replacing a plugin or moving a list item. Live mutable handles would need
invalidation and synchronization with those structural changes; snapshots keep
one owner of the answers. The CLI already descends through the schema as it asks
questions, discovering children after choosing their plugin; it keeps that walk
rather than snapshotting fields before they have been chosen.

- **One following/customized state for ranking and scoring**. A new form follows
  suggestions: changing basic settings replaces both ranking and metrics with
  defaults and suggestions for those settings. Editing either list or a field
  within it, including entering invalid numeric text, keeps the whole section
  from then on. Applying suggestions explicitly replaces both lists and their
  settings and resumes following. There is no per-field comparison or merging.
  Existing and undone phases start customized, preserving their configuration.
  The state belongs to the form and is not saved.
- **Visibility does not imply customization**. Future preset, advanced, and expert
  levels may expose different fields without changing this state. A preset can
  supply a complete draft; an actual edit takes control of the section. Keeping
  hidden fields following after customization would require a separate design;
  the current section-wide state intentionally freezes them too. Suggestion
  outputs are ranking and metrics; other basic settings remain inputs, allowing
  a future match-type axis alongside the format.
- **Implied plugins are shown, not chosen**: added to a copy for the view, never
  to the draft, so they go when nothing needs them.
- **Issues where they are fixed**, at the deepest field or item their path is in;
  **warnings** (a ranking of nothing) do not block.
- **A set keeps one order**, by label, so a checkbox stays under the pointer,
  also when another format recommends other metrics.
- **All registered options, recommended first**. Compatibility does not hide a
  choice. Selecting an incompatible combination shows issues where they can be
  fixed; adding or saving a phase requires a valid configuration. Duplicate list
  entries remain excluded. Partial library issues postpone a missing fact while
  an unanswered field's axis could provide it, without recursively proving a
  feasible completion. The CLI checks a completed copy with implied providers
  before returning, and retries unresolved conflicts and missing dependencies.
  Its lists start with recommendations or existing answers, without sole-supplier
  preselection. The recursive completion predictor was removed rather than kept
  behind a visibility switch; future preset/advanced/expert visibility is separate.
- **The form of a new phase starts at the phase undone last**, so adding it again
  adds the same phase.
- **Numbers are typed as text** (`type=text` with `inputmode`), taken on change
  or Enter, comma or point; what is not a number of the range stays shown with
  why, the answer unchanged. A `type=number` hands over nothing for what the
  browser cannot read (`0,5`).
- **The focus stays with the operator's work** after a change moves or removes
  what had it (`phase::focus_later`); buttons that become unavailable while
  focused are `aria-disabled`.
- **Once added, the phase is a summary** edited in a dialog starting at its
  config; a refusal shows in the dialog.
- **The help explains every plugin** from the catalog (`Session::glossary`), by
  its translated label and description, so it needs no text of its own per
  plugin. The phase and the empty list of entrants link to its sections
  (`#help-<section>`) in a tab of their own, which keeps the session open, and
  with it what can be undone.
- **How long a phase takes shows before the start** (`Session::scale`), found
  by playing it out apart from the competition with as many entrants, once
  with every match won by side A and once by side B, so that a number
  depending on the results (a grand final played again) is a range. It knows
  no format, and a few milliseconds suffice; the GUI plays it out again only
  when the phase or the entrants change.
- **Widgets from the Dioxus team's components** (`select`, `dialog`), copied
  into `src/components/` and changed there: a plugin's option shows its
  description, which a native `option` cannot.

Rejected: the GUI walking the schema (browser-only tests, repeated per
frontend); the questionnaire one question at a time (long, and a correction
walks through everything); replacing the draft by the format's defaults (loses
other changes); native selects (no descriptions where the choice is made).

### Running a competition

- **One list of matches at a time**: a round, or those **playable now**
  (`Session::playable`: open matches none of whose entrants has an open match of
  an earlier round; entrants rather than rankings, for Monster DYP's teams).
- **Results by keyboard, two ways.** A typed result (`Session::report_entered`):
  the match number and `1`/`X`/`2` (football pools' notation, no language
  needed; `3 1`, `3-1`, `3x`), or part of the winner's name, with the number
  and `X` where needed (`ali`, `ali 5 X`), as an entrant at the desk says who
  won. Names are found ignoring case and accents (`view::NameSearch`). It
  reports only what fits one side of one open match and **never guesses** among
  several, not even the one playable now: a wrong match reported is worse than a
  number typed. A trailing number or `X` is part of the name while a name
  contains it (`Malcolm X`, `Team 7`). Scores follow the number, side A's first
  (`3 21-17 19-21 21-15`), or the name, the named side's first (`ali 21-17`),
  as an entrant says them; the match's own or else one per part, as the CLI
  takes them. A result typed must be the one they say, if any: overriding the
  scores, rare, belongs in the result dialog. While typing, the list shows the matches
  it fits and the field says what Enter does (`Session::read_entered`). Result
  buttons move the focus to the next match without a result, so it never falls
  back to the page's start.
- **What an action did is announced** in a live region, kept as data and said
  when shown, so it follows a language change.
- **Undoing the start asks first** (`Session::next_undo`): it leaves play for the
  setup, unexpected when stepping back through results.
- **Finding an entrant is the browser's find in page**: with 10–20 matches a
  list, it answers "whom does Carol play" as well as a filter, which would have
  overlapped the typed entry. So every name shown is text on the page, and what
  is not shown is `hidden`. *Planned:* with running matches, entry belongs to
  their list, where a name fits one match. Starting a match is then an action of
  its own, perhaps following a suggestion of what to start next that the keyboard
  can confirm; any order of rounds stays allowed.
- **A finished competition says who won** (`Session::winners`, all tied for first
  by name); the list says it too through `Progress::Won` (older listings keep
  `Progress::Finished` until their next save).

Rejected: single-key shortcuts (clash with screen readers, differ by language,
a hidden mode); merging search and result entry (an Enter out of habit would
report, and a read-only tab can search but not report); `Finished` gaining the
winners (old listings would not read).

### The bracket

- **What the GUI cannot know is in the engine.** Matches exist only once both
  sides are known, so the pairings to come are in the knockout formats' state,
  their `Bracket`, handed out read-only (`SingleElimination::bracket`,
  `DoubleElimination::bracket`): pairings with round, annotations (so stage),
  match once scheduled, and each side's source (`BracketSide`).
- **The app knows the two formats, not the trait**: `Session::bracket` downcasts
  the format (`PhaseFormat: Any`), splits pairings into sections (`BracketPart`)
  and columns by stage (third place `below` the final), and works out the lines
  and how an undecided side is named (by match number, else stage and place).
- **What can only be a bye is worked out from the tree**: a pairing of two byes
  not left out (below) keeps its slot, empty (`Unplayed::Byes`), so the
  geometry holds; a side
  waiting on a pairing against a bye names who plays the bye; a reset says "if
  needed", then "not needed" (`Unplayed::NotNeeded`).
- **Drawn as columns of equal slots**: each column a grid of `1fr` rows, so a
  pairing is level with the middle of the two it follows from at any size,
  without measuring; lines are pseudo-element borders; long names wrap within
  the narrowest column (`7rem`); all sections scroll sideways together. Read in
  order it is the list of matches by stage, so no text alternative is needed.
- **Byes take no box**: a column with pairings decided by a bye is folded into
  the rows of the next one (`BracketColumn::rows`, `BracketPairing::row`),
  leaving them out and tagging the side they decide (`after_bye`), two
  pairings sharing a row where their winners meet; a column of byes only is
  left out. Twenty entrants take eight rows rather than sixteen. Only a column
  nothing leads into is folded, since the lines into it expect a row per
  pairing.
- **A bracket higher than the screen scrolls within a box as high as it**, the
  stages sticky on top, which they could not be with the page scrolling (the
  box scrolls sideways, so it is their scroll container either way). Its
  edges fade out where more is out of sight, over the pairings, measured on
  scroll and resize, since a shadow behind them is covered.
- **Shown in place of the columns** by the view switch, the rest kept `hidden`
  so state survives; the URL names the view (`#<key>/bracket`), so that a
  reload and the history keep it, and a competition opened shows its matches
  rather than what was chosen in another.
- **A result is a click on the winner's name** (a button "Alice wins", styled as
  a link, since touch has no hover); the focus moves on as in the list.
  Reopening and typed results stay in the list. A reopen a later result depends
  on is refused naming that match; knowing beforehand would need a trial replay
  per result on every render.

Rejected: a trait method for views (moves the table of kinds, self-reported);
the format drawing itself (the engine knows no display); rebuilding the tree
from matches (no future pairings); SVG (CSS lines keep pairings as HTML text);
the bracket below the columns (far down, unnoticed); result buttons in every
pairing (every slot grows); the bracket in half the width.

### The cross table

- **A phase whose teams stay the same shows a cross table** in the bracket's
  place: a row and column per team, each cell the matches between them from the
  row's side, each at the highest level of its result (`CrossMatch::score`: its
  own score, else the parts won, so `3–2` for a best of five rather than its
  games), or else `1`, `½`, `0`; said as won/drawn/lost, followed by the score
  if it shows one (`won, 3–2`), as an overridden result need not follow from
  the score. A click on a match with more than who won opens its details, as in
  the list. `Session::cross_table` offers it where every match is between two
  single rankings and the format plays no bracket (round robin, random rounds;
  not Monster DYP or knockouts), so no table of formats beyond the bracket's.
- **Rows by rank**, as published cross tables are; columns headed by row
  numbers (names would make 20 columns too wide), names said to screen readers
  and kept in sight while scrolling.
- **Read-only**, an open match's cell empty: with rows by rank, reporting in the
  table would move the cell under the operator's hand, and narrow cells fit
  many teams.
- **A total last** (`CrossRow::total`), the sum of the row's `1`, `½`, `0`, as
  published tables have, whatever the points of the phase; wins and losses
  are tinted (`--win-subtle`, `--error-subtle`), a win also bold, so that it
  reads without colours where a score does not say who won.

Rejected: rows in entry or draw order (stable, but an order seen nowhere else);
open matches shown by number (wider columns, the list tells it anyway).

### Layout

For laptops and tablets, and small events on a phone alone; typically 20–40
entrants, but any number and format.

- **One sticky bar on top**: back to the list, the competition's name, its
  actions (undo, redo, export; icons only on narrow screens), and the view
  switch below it, so undo is always at hand; `scroll-padding-top` keeps
  keyboard targets clear. The language and theme are in the footer.
- **Cards in columns where there is room**: the setup's entrants beside the
  phase from 64rem, stacked below; after the start matches beside standings,
  and below 60rem buttons choose which is shown (the other hidden by CSS only).
  How the phase is played, the bracket and the cross table are views of the
  switch at every width, since below the standings they were out of sight.
- **The entrants grow with the page** (no inner scroll box), the field below
  them keeping the focus.
- **What can be clicked looks like a button**: bordered, 44px where the pointer
  is coarse; one primary action per card.
- **A match is a card**: the two sides around its number, each side's button
  below its name, draw in the middle, columns aligned by subgrid; one per line,
  since a list is read to find who plays whom. With a result it shrinks to one
  row in place, so a round is mostly what is left to play without cards moving.
  Finished matches share the columns of the list (subgrid), so their pairings
  line up whatever their scores, as in a table; where the list is narrower
  than 40rem, the scores of the games of a best of give way to the games won
  (`3–1`), still read out.
- **Names are never cut or shrunk**, they wrap: a cut name may no longer tell
  two entrants apart, and reporting for the wrong one is the worst mistake
  possible. So result buttons say only "Wins", named "Alice wins" by
  `aria-label`.
- **Rounds are stepped through** (a select between previous/next), since a round
  robin of 40 has 39 rounds.
- **Standings** scroll sideways with the names sticky, abbreviate long headers
  as chess does (`SB`, `Bh`, the `.column` translation) with the whole name and
  the calculator's settings in the header's toggletip, and count matches
  played.
- **Matches are a list, not a table**: tables laid out as cards lose their
  semantics in some browsers.
- **Colours are tokens** named by role at the top of `style.css`, each
  `light-dark(light, dark)`, following `color-scheme`: the system's or the one
  chosen (`theme.rs`, `<html data-theme>`, applied by an inline script before
  the first paint). No rule names a colour. Neutral greys and one navy accent.
  Every pair meets WCAG AA (text 4.5:1, control borders and focus ring 3:1);
  what is chosen is tinted and bold, not colour alone. Animations end at once
  under `prefers-reduced-motion`.

Rejected: a match on one line (cramped); several cards per line (less room for
names, numbers run across and down); a draggable split (rarely found, hard to
make accessible); sticky add-phase buttons (covered the focused field).

### Offline and installing

The page is a PWA (files in `tournament-gui/public/`, copied next to
`index.html`).

- **A service worker of its own** (`sw.js`): `dx` names assets by content, so
  once kept they come from the cache; everything else comes from the network and
  from the cache only when that fails. A changed page drops the old assets.
- **The first visit is handed over**: `pwa.js` watches what the page loads
  (`PerformanceObserver`) and hands it to the worker once active, so a page
  opened once works at a venue without network.
- **Relative URLs**, so it works under any path (`dx build --base-path`).
- **The storage warning** words both causes the page cannot tell apart (a
  browser deciding by use or space, a private window) and offers what helps:
  exporting, installing while the browser offers it (`beforeinstallprompt`,
  kept by `pwa.js` since it may come before the wasm runs; persistence is asked
  again once installed), and not showing it again. It folds to one line once
  read.
- **The release build** is `dx build --release --debug-symbols false` (dx keeps
  debug symbols otherwise) with the `wasm-release` profile (`opt-level = "s"`,
  LTO, one codegen unit), keeping the wasm under 1 MB with gzip at about the
  speed of `release`; `"z"` is smaller but more than twice as slow. The e2e
  tests run on the same build.

Rejected: precaching a build-time list (dx makes none); network first for
assets (waits on a bad venue connection, and content-named assets never go
stale); dismissing the warning per session only (nags in every tab).

**Hosting**: static files over HTTPS on GitHub Pages, under `/tournamo/`
(hence `--base-path`), built and deployed by CI for each release tag
(`v<major>.<minor>.<patch>` on `main`, naming the version in `Cargo.toml`,
which the help shows; not a pre-release like `v0.2.0-rc.1`) that passes it, so that what is
hosted is always a release (deploying every
push to `main` would make releases meaningless): the wasm holds the absolute paths of its asset sources, so what is
hosted must not be built on a developer's machine. Navigation is after `#`, so
no 404 fallback is needed. The command line in the browser (`tournament-web`)
is a development tool and not hosted.

### Translation

**Fluent** (`fluent-bundle`) for all UI text, English and German. The machinery
is in `tournament-app` (`i18n`), tested natively and in wasm.

- A `Translator` of a `Language` holds the app's messages (`app.ftl`, what its
  errors say) and a consumer's, handed over as `Resources`, an exhaustive
  `match` over languages, so a new language does not compile until every
  consumer has its messages. `i18n::check` tests a consumer's resources.
- **Errors say a `Text`** (key and variables) through `Message`, not a string,
  so a shown error follows a language change. English says what `Display` says.
- **One message per plugin** (`plugin-<id>`, `::` as `-`), its texts as
  attributes (`.label`, `.description`, `.column`, `.field-<name>`,
  `.field-<name>-description`): kept together, no collisions. English is the
  code; a test checks German translates every engine plugin and nothing else.
  Third-party plugins show in English until translated. The phase's own fields
  are attributes of `phase`.
- **A missing message falls back to English**, then to its key, rather than
  failing.
- **Stages and cells are `Phrase`s** (a key, its numbers, an English `Display`),
  translated as `.stage-<key>` / `.cell-<key>` with the numbers as variables,
  so a language words them its way (German `Achtelfinale` for `Round of 16`).
  Rejected: the GUI translating the engine's annotations (a table of plugins);
  cells as `String`s (untranslatable).
- **A plugin's label can be a variable** (`Arg::Plugin`), translated when said,
  so issues name plugins in the operator's language; what a plugin says inside
  an issue stays English.
- **Numbers are written as the language does** (`Translator::number`, `0,5`),
  and a comma is taken when entered.
- **An error's detail stays apart** (`Text::with_detail`: serde's position, a
  storage error), folded away below the alert, outside it so screen readers do
  not read it out, marked `lang="en"`; `Display` leaves it to `source`.
- **Messages are lowercase fragments** like Rust's errors; `Translator::sentence`
  makes a sentence for the screen. A plugin label standing alone is capitalized
  by the GUI.
- **Isolation marks are kept**: Fluent wraps variables in U+2068/U+2069, so a
  right-to-left name next to numbers or punctuation is not reordered. Tests
  compare without them; the page title has none (tabs show them as boxes).
- **The language** is the one chosen last (local storage), else the browser's
  first preferred one the GUI speaks, else English; `<html lang>` follows.

### Accessibility

**WCAG 2.1 AA.** Semantic HTML everywhere; widgets needing more (select,
dialog) are the Dioxus team's components (`dx components add`, on the
unreleased `dioxus-primitives`, a git dependency pinned in `Cargo.lock`), whose
source in the project lets each be replaced on its own later. Rejected: own
components from the start (ARIA patterns are much work to get right).

**End-to-end tests**: `e2e-harness` serves a built page, runs Chrome, fails on
logged errors and on axe-core WCAG 2.1 A/AA violations (axe-core downloaded at a
pinned version, checked by SHA-256, cached; too large to commit), and keeps what
a failed test leaves. One crate per page (`tournament-web-e2e`,
`tournament-gui-e2e`) wraps a `Page` in what an operator does there, so each
page is built by its own tool (trunk, dx) in its own hook and fails on its own.

### Performance

Fine for the first use cases (20–50 entrants). Undo, redo and loading replay
the whole log, which at 1000 entrants and 5000 results took ~1.8 s in wasm and
several seconds on a phone, missing the requirement (< 1 s with 500 entrants on
a mid-range phone), while ranks were sorted after every result. Computing them
when the standings are read (see [Metrics](#metrics-plugin-produced-ranking-data))
cut that replay natively from 644 ms to 58 ms (`full_tournament`); reading the
standings takes 0.08 ms, and serializing the competition 0.1–0.2 ms more than
before, as it sorts once. Wasm and phones are not measured again yet.
*Planned:* snapshot checkpoints; reading the competition stored next to the log
takes 13 ms at 1000 entrants and 29 ms at 2000 (Chrome, JSON), against 1.3 s
and 6.3 s to replay its log (measured while ranks were sorted after every
result). The UI shows one round of matches at a time rather than thousands of
rows.

Measured while ranks were sorted after every result, with the
`large_tournament` benchmark's competition (1000 entrants, random rounds over
10 rounds, every metric, 4 criteria), the engine in wasm, in ms:

| results reported | report one | undo / redo / load (replay) | native undo |
|---|---|---|---|
| 0 | 2.0 | 7–13 | 5 |
| 2500 | 0.4 | 760–810 | 505 |
| 4999 | 0.5 | 1680–1810 | 1120 |

Building 5000 match rows and 1000 standings rows takes 2–4 ms; saving the log as
JSON 3–8 ms. In headless Chrome, also measured then, with all rows on the page
and 2500 results, in ms until the page shows the change (4× CPU is a mid-range
phone):

| | Leptos 1× | Dioxus 1× | Leptos 4× CPU | Dioxus 4× CPU |
|---|---|---|---|---|
| load and render | 1750 | 1850 | 6660 | 7470 |
| report one result | 160–200 | 75–90 | 480–510 | 320–350 |
| undo it | 850–890 | 770–820 | 3410–3760 | 3080–3140 |
| back to 5000 rows | 630–720 | 370–390 | 2710–2780 | 1550–1560 |
