# Review of commit 6a8dbbd

Reviewed `6a8dbbd5750853a04a0f386ea42a682ad85bf55b` ("match format") against its parent. References below are paths and line numbers **at that commit**, not the current checkout. Engine, app/CLI, and GUI were reviewed by separate sub-agents; findings were checked against the recorded design and implementation plan.

## Correctness findings

### 1. P2 — Accepted scores can overflow their tallies and standings

**Locations:** `tournament-engine/src/plugins/match_format/best_of.rs:109–112`; `tournament-engine/src/plugins/metrics_calculator/points_balance.rs:140–141`.

Score fields accept any `u32`, but the best-of tally adds these into another `u32`. Two games scored `4294967295–0` and `1–0` pass structural validation and then panic during tallying in an overflow-checked build. An unchecked release build wraps the total to zero. Points accumulated across matches have the same problem in `PointsBalanceCalculator`.

This can crash result reporting or silently corrupt score-based standings. Checked and unchecked builds also disagree when replaying the same accepted log. The large input makes this an edge case, but the declared input domain permits it and the design deliberately accepts unusual scores.

**Recommendation:** use wider aggregate types and define a deterministic overflow policy. Validate any remaining limits before mutating competition state. Cover both summing games and accumulating matches.

**Evidence:** a temporary integration reproduction on the target commit failed with `attempt to add with overflow` at `best_of.rs:111`.

### 2. P2 — A best-of result stays undecided when all games finish with any draws

**Location:** `tournament-engine/src/plugins/match_format/best_of.rs:126–131`.

The completion condition counts wins (`won.a + won.b`), rather than decided games. For a best of three with an A win, a B win, and a draw, all games are decided but `suggest` returns `None`. A best of one containing a drawn game fails in the same way.

Drawn game decisions are accepted by the engine, and the method explicitly promises a suggestion when every game is decided. In the GUI the quick-entry row therefore cannot report this completed play; the operator must open Details and manually override the missing outcome.

**Recommendation:** count decided games separately from wins, and use the win totals to select the outcome once the configured games are exhausted. Preserve the phase format's existing restriction on match-level draws.

**Evidence:** a direct `MatchFormat::suggest` reproduction expected `Some(Draw)` and received `None`.

### 3. P2 — Completing a score overflows on valid numeric input

**Location:** `tournament-app/src/result_form.rs:300`.

With a winning-score hint of 21, enter `4294967295` into one side and leave the input or press Tab. The condition `count + 2 <= winning` panics with checked arithmetic. In unchecked builds it wraps and incorrectly fills the opponent's score with 21.

This is independent of aggregate overflow: it happens while editing, before a report is submitted or tallied.

**Recommendation:** use checked addition or a checked subtraction from the winning score. An entered count above the hint must leave the other side untouched.

**Evidence:** a temporary app test invoking `complete_score` with `u32::MAX` reproduced the overflow panic.

### 4. P2 — A valid best-of configuration can exhaust browser memory

**Locations:** `tournament-engine/src/plugins/match_format/best_of.rs:33`; `tournament-app/src/result_form.rs:62–70,464–479`.

The games configuration accepts any positive `u32`. `Entry::empty` eagerly allocates an entry for every possible game, and `part_view` constructs every game's view before truncating to the visible games. A sufficiently large accidental configuration can freeze or exhaust the browser's memory when result entry is constructed. The progressive UI does not limit this allocation.

**Recommendation:** impose a documented practical upper bound, or construct entries and views lazily for entered/visible parts. Lazy construction also better matches the progressive-entry concept. Do not exercise the maximum allocation in a regression test; test bounded construction instead.

**Evidence:** static trace from the unrestricted schema to eager allocation; no destructive OOM reproduction was attempted.

### 5. P2 — Cross-table scores are hidden from screen readers

**Location:** `tournament-gui/src/cross_table.rs:90–98`.

The new visible score, such as `21–17` or `2–1`, is placed inside `aria-hidden="true"`. The accessible replacement still says only win, draw, or loss. Screen-reader users cannot compare the scores while navigating the table and must open each match's details individually.

**Recommendation:** include the row-oriented score in the accessible text alongside the outcome. Keep the outcome because an overridden decision need not agree with the numeric score. Add an assertion on accessible cell text; an automated accessibility scan alone will not detect this missing information.

**Evidence:** static inspection of the rendered accessible-text branches; no screen-reader session was run.

### 6. P2 — GUI number inputs reject valid negative decimal values

**Locations:** `tournament-gui/src/result.rs:221–225,508–512`.

Both numeric input renderers set `min="0"` for every `ValueKind`. A decimal field carries a finite signed `Score`; neither the engine's `ValueKind::Decimal` nor its template contract restricts it to nonnegative values. A third-party format accepting a judged score of `-0.5` can be represented by the app and engine, but native HTML constraint validation blocks submission in the GUI.

This affects the public plugin interface; the built-in formats currently use counts, so it does not affect their ordinary entry flows.

**Recommendation:** apply the zero minimum only to counts. If decimal bounds are needed, describe them explicitly in the template and render those bounds consistently.

**Evidence:** static mismatch between input constraints and the public value type; no custom decimal plugin/browser reproduction was run.

## UX improvements

### Preserve scores while correcting a reported result

The Reopen action (`tournament-gui/src/play.rs:699–702`) undoes the report, after which `Session::result_form` creates a blank form (`tournament-app/src/session.rs:1218`). Correcting one game of a best of five requires re-entering all its scores. The undo also removes dependent results where applicable, so this is a consequential action for a small typo.

Capture the reported play as a correction draft before reopening and populate the new form from it. The actual correction should still use logged undo/report transitions. Make the consequences for dependent results clear. This is a UX suggestion, not a claim that the existing undo semantics are incorrect.

### Accept pasted displayed scores in the CLI

The CLI score parser (`tournament-cli/src/command.rs:95–107`) accepts a hyphen or colon, while displayed scores use an en dash. Copying `21–17` into `report 1 21–17` fails. The app's entered-result parser (`tournament-app/src/session.rs:1597–1620`) already accepts more score syntax, including the en dash and decimal values.

Share score tokenization and numeric conversion between consumers, retaining their different result-selection grammars. This improves copy/paste and avoids parser drift.

### Make result details discoverable

Finished matches expose details through an invisible button covering the card. The accessible name and focus ring help keyboard users, but touch users have little indication that the score/card opens a dialog. A small visible Details label or icon would make the interaction easier to discover without making the finished match much taller.

## Code simplicity and unnecessary work

- **Balance calculators:** `games_balance.rs` and `points_balance.rs` repeat accumulation, difference calculation, dependent-metric updates, and much of their test structure. Consider a small internal helper for shared arithmetic/update operations. Keep the explicit public metric/plugin types and persistence tags; a generic plugin framework would add conceptual cost merely to reduce similar declarations.
- **Numeric input rendering:** `QuickValues` and `ResultField` in `tournament-gui/src/result.rs` duplicate input attributes, parsing, score completion, and Tab handling. A shared input component would give bounds and keyboard fixes one implementation while preserving the two layouts.
- **Template observation constructs full forms:** `ResultOf`'s memo (`tournament-gui/src/result.rs:46–53`) calls `session.result_form(id)` on session changes merely to obtain its template. This clones the format and constructs the entire entry tree for displayed open matches. Expose a lightweight template query and instantiate a form when needed. No performance benchmark was run; this is unnecessary work visible in the call path, amplified by finding 4.

## Validation and limits

- Read the commit's match-details/persistence design, implementation plan, relevant tests, and the engine, app/CLI, and GUI changes. The implementation plan explicitly defers result-reason controls and nested fixture presentation; those are not counted as regressions.
- Target commit was extracted to `/tmp/review-6a8dbbd`, leaving the current checkout's source unchanged.
- App baseline: `cargo test -p tournament-app --offline` passed 191 unit tests, five golden-save tests, and three doc tests; the save generator remained ignored.
- Temporary targeted tests reproduced findings 1–3. These deliberately failing review probes were outside the repository and are not part of this documentation change.
- The prescribed `prek run --all-files cargo-test` attempt could not start because its default log/cache directory was read-only. The direct offline tests above supplied targeted evidence instead. No full workspace, wasm, browser e2e, or assistive-technology run was performed for this review.
- Documentation validation is separate: the all-files hooks were also run on the current checkout using a writable copy of the hook cache. These checks validate this report's checkout, not the reviewed commit.
- GitHub issue #5 could not be fetched because `api.github.com` was unreachable. Review used the design and plan included in the commit.
- No additional confirmed JSON persistence regression was found. Passing existing tests does not invalidate the boundary cases above.
