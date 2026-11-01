# Code size and architectural review

Measured on 2026-11-01 with `cargo warloc --by-file -o json`, before adding
this report. Counts are Rust code lines, excluding comments and blank lines.
They measure source size, not runtime cost, generated macro output, or difficulty.
CSS, JavaScript, HTML, translations, and dependency implementations are excluded.

## What the headline includes

The reported totals are **19,339 main**, **15,371 test**, and **86 example**
lines across 119 Rust files. However, “main” means the tool's source category,
not necessarily production application code:

| Included in main | Lines |
| --- | ---: |
| E2E harness and GUI/terminal test drivers | 1,664 |
| Benchmarks | 138 |
| Metric test helpers (`metrics_calculator/test_utils.rs`) | 147 |
| Engine plugin builder (`plugins/builder.rs`) | 36 |
| Remaining main category | 17,354 |

This is a useful adjustment, not an exact production-only count: other files
also contain feature-gated test constructors and builders. Conversely, tests
defined using browser-specific attributes can be classified as main by the tool.
For example, `tournament-web` has browser tests inside its source, although the
tool assigns it zero test lines. Treat the test column as the tool's classification.

## Where the lines go

These are disjoint buckets assigned by file; their columns sum exactly to the
headline totals. Shared files stay together rather than attributing the same
infrastructure to several features. Tests in a shared suite are not apportioned
to individual features.

| Feature / responsibility | Main | Tests | Source boundary |
| --- | ---: | ---: | --- |
| Generic plugin infrastructure | 3,242 | 4,667 | `plugin-factory`, `plugin-factory-derive` |
| Engine state, transitions, types, plugin interfaces | 2,069 | 1,157 | Engine source outside concrete formats/metrics |
| Tournament scheduling formats | 912 | 1,105 | `plugins/phase_format/` |
| Metrics and their ranking criteria | 980 | 1,290 | `plugins/metrics_calculator/`, including test helpers |
| Engine integration and save compatibility tests | 0 | 1,067 | Engine `tests/` |
| App session, views, errors, persistence | 1,815 | 1,729 | App except form, translation, bracket/cross-table modules |
| App phase configuration form | 862 | 538 | App `form.rs`, `form/tests.rs` |
| App translation infrastructure | 400 | 412 | App `i18n.rs`, `i18n/tests.rs` |
| App bracket projection | 231 | 0 | App `session/bracket.rs` |
| App cross-table projection | 55 | 0 | App `session/cross_table.rs` |
| GUI competition lifecycle | 1,296 | 0 | `main`, `competition`, `competitions`, `setup` |
| GUI phase configuration | 906 | 0 | GUI `phase.rs` |
| GUI matches, standings, bracket, cross table | 969 | 0 | GUI `play`, `bracket`, `cross_table` |
| GUI shared widgets and utilities | 825 | 11 | Remaining GUI source |
| Browser storage, locking, messaging, downloads | 536 | 160 | `tournament-browser` |
| CLI questionnaire | 734 | 269 | CLI `questionnaire.rs` |
| CLI commands, session, storage, entry point | 889 | 1,229 | Remaining CLI source |
| Browser terminal | 816 | 0 | `tournament-web` |
| E2E support and scenarios | 1,664 | 1,737 | `e2e-harness`, GUI/web E2E crates |
| Benchmarks | 138 | 0 | Engine `benches/` |
| **Total** | **19,339** | **15,371** | |

The zeroes beside UI features do not mean they are untested: their tests live
in the app suites and E2E bucket. Examples are a separate category (86 lines).

## Individual tournament features are relatively small

| Feature | Main | Tests in the same files |
| --- | ---: | ---: |
| Round robin, including circle and randomized rounds | 173 | 185 |
| Single elimination | 140 | 321 |
| Double elimination | 204 | 450 |
| Shared elimination bracket | 309 | 11 |
| Monster DYP | 86 | 138 |
| Points and points ranking | 124 | 173 |
| Wins/draws/losses and associated ranking | 153 | 236 |
| Round of elimination | 109 | 130 |
| Round of first loss | 97 | 97 |
| Buchholz family, including shared incremental calculation | 350 | 654 |

The format rows total 912 main lines. The metric rows total 833; the other
147 lines in the metric bucket are test helpers. Shared bracket behavior is
also exercised through the elimination format tests and integration tests.

Scheduling formats and metric implementations together occupy 1,745 main
lines after excluding those helpers: about 9% of the headline main count.
There is no size-based evidence here for rewriting the tournament algorithms.
The Buchholz family already shares its weighted opponent-sum calculation.

## Configuration is the main concentration

| Configuration layer | Main | Tests |
| --- | ---: | ---: |
| Drafts, candidates, defaults, implied providers, validation | 903 | 1,551 |
| Schemas, field types, dependency-ordered collections | 483 | 541 |
| Numeric ranges | 294 | 99 |
| Config values and paths | 294 | 132 |
| Compatibility facts and dependency ordering | 355 | 486 |
| Catalog | 207 | 335 |
| Plugin crate root | 98 | 0 |
| Procedural macros | 608 | 359 |
| Plugin integration tests | 0 | 1,164 |
| App form model and editing behavior | 862 | 538 |
| GUI form, summary, dialog, focus handling | 906 | 0 |
| CLI questionnaire | 734 | 269 |
| **Total** | **5,744** | **5,474** |

This is about **30% of main code**. It is a broad configuration/extensibility
budget, not a claim that all these lines could disappear together. It includes
reusable collections and macros. It excludes configuration-related portions of
session files, terminal question handling, translations, and E2E tests.

The size buys more than choosing a format: third-party plugins, nested typed
fields, numeric constraints, compatibility filtering, dependency ordering,
automatic providers, recommendations, preserving earlier choices, editable
lists/sets, and two different interaction models. `DESIGN.md` sections
“Compatibility between plugins” and “Configuring a phase” explain those contracts.

## Where I would investigate simplification

These are hypotheses from source review, not proven savings estimates. A small
prototype and a net diff would establish whether each actually reduces complexity.

### 1. Consolidate schema navigation and field descriptions

The app form walks schemas and paths and has its own `kind_at`, `schema_at`,
`entry_at`, `position`, and list-path helpers. The draft engine also locates
fields and builds an internal question tree. The CLI separately walks field
kinds, defaults, candidates, and nested plugin questions.

Investigate a small read-only field/cursor API in `plugin-factory`, exposing
field kind, current answer, default, and candidates at a path. Let consumers
keep their interaction policies: the GUI updates later recommendations, whereas
the CLI asks sequential questions. Sharing policy prematurely could make the
API larger than the code it replaces.

Target: `plugin-factory/src/draft.rs`, `tournament-app/src/form.rs`,
`tournament-cli/src/questionnaire.rs`. Those three files contain 2,499 main
lines, only some of which overlap. This is the first experiment I would make.

Success means fewer independent navigation/interpretation paths and a smaller
combined implementation, with existing configuration behavior preserved.

### 2. Share competition operations between CLI and app

The CLI session directly owns and operates the engine state machine and its
session history. The app session does so too. Entrant handling, phase operations,
result reporting, undo/redo, and error interpretation therefore have two owners.
The browser terminal inherits the CLI implementation.

Investigate making CLI commands use the app's operation layer, while retaining
CLI text formatting, explicit save/exit semantics, and whole-file storage.
Do not force the CLI to adopt GUI autosaving or incremental record storage.
If the existing app session is too coupled to storage, extract only the common
competition operations rather than introducing another complete session layer.

Target: CLI `session.rs` (645 main lines) and app `session.rs` (997).
These 1,642 lines are an investigation boundary, not removable duplication.
The CLI's command language and the app's richer projections remain useful work.

Success means one implementation of shared operation rules and fewer repeated
error mappings, while CLI and GUI retain their behavior and file compatibility.

### 3. Decide how much interactive configuration behavior is essential

The app form is 862 lines and its GUI is 906. Automatic recommendation following,
preserving custom choices, implied providers, list reordering, nested options,
and stable focus all contribute. Merely splitting these files changes navigation,
not the amount of behavior.

The largest reduction may require a product choice: for example, explicit
“reset to recommendations” instead of automatically following later fields.
That would alter a deliberate behavior described in the design. Measure the
specific removed logic before accepting the usability tradeoff; most widget
rendering and validation would still be necessary.

### 4. Treat additional frontends as an explicit maintenance budget

CLI plus browser terminal occupy 2,439 main lines. Their dedicated E2E driver
adds 103 main-category lines, alongside 1,750 classified test lines across the
CLI and terminal E2E suites. Browser tests classified as main are included in
the 2,439. The shared E2E harness is excluded from this frontend subtotal.

If all frontends matter, shared operations are the better first step. Retiring
one is a product scope reduction, not a transparent architectural improvement.

## Areas I would leave alone initially

- **Elimination brackets:** scheduling, app projection, and GUI rendering have
  different responsibilities. Their 1,176 combined main lines (309 shared
  bracket, 140 single, 204 double, 231 projection, 292 rendering) do not by
  themselves demonstrate duplication.
- **Browser coordination:** 536 browser-crate lines cover persistent storage,
  locks, messages, downloads, and persistence requests. GUI orchestration adds
  more, but reducing correctness around concurrent tabs needs stronger evidence.
- **Incremental metric calculation:** replacing it with full recomputation may
  shorten code but change result-reporting costs. Benchmark before that tradeoff.
- **Persistence and replay contracts:** keep stable tags and old saves loading.
  Smaller code is not a reason to weaken replay determinism or validation.
- **Tests:** large suites around configuration reflect many behavioral cases.
  Test/main ratios alone do not establish implementation complexity or redundant
  coverage; shared fixtures also affect those ratios.

My priority would be a small shared field-navigation experiment, followed by
sharing session operations. The measured concentration is in extensibility,
configuration UX, and multiple consumers; the individual tournament rules are
already compact.

## Reproducing the numbers

Run `cargo warloc --by-file -o json`. For each file, sum `main.code` and
`tests.code` using the source boundaries above. The detailed tables are alternate
views of the same files and must not be added to the overview. Counts are a
snapshot; this document is not a generated size budget or a performance audit.

## Traversal experiment following this review

`Draft::fields()` now enumerates fields with their schema, answer, and chosen
plugin children. The app renders that tree directly, and the draft uses the
same enumeration to collect compatibility information. The app no longer
reconstructs schemas and plugin entries by interpreting paths. See
`plugin-factory/src/draft/fields.rs` and “Configuring a phase” in `DESIGN.md`.

This is a bounded change to reading the configuration. Checked edits, dependency
scoping, recommendation following, and diagnostics still use paths. The CLI's
existing depth-first questionnaire was left in place: replacing it with a tree
snapshot would complicate asking for children of a plugin chosen during the walk.
The size tables above remain the baseline before this experiment.
