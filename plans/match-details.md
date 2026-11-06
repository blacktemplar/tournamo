# Plan: match details (issue #5)

Tracks the implementation of match details across commits. The design is
DESIGN.md, *Match details*; this file only says what is done and what is left.
Tick a step in the commit finishing it. Delete this file when the plan is done.

## Steps

### 0. Design record
- [x] DESIGN.md section *Match details* (*planned*)

### A. Engine groundwork (behaviour unchanged)
- [x] 1. `Decision` and `ResultReason`: `Match` holds `decision: Option<Decision>`
  (`result()` stays); `ReportMatchResult` carries a `Decision`; old log entries
  (`match_result`) and snapshots (`result`) still load
- [x] 2. `Part`, `Sides`, `Value`, `FieldKey`: `Match` holds `contest: Part`
  (its annotations move in); `ReportMatchResult` gets `values` and `parts`;
  old snapshots load; benchmark `large_tournament` (no change measured,
  `Match` grew from 160 to 208 bytes)
- [x] 3. `MatchRules` axis: `Template`, `Level`, `Parts`, `FieldSpec`, `Hints`,
  plugin `WinnerOnly`; `PhasePlugins.results` (old phases load as `WinnerOnly`);
  `ReportMatchResult` checks the structure against the template; questionnaire
  and `PhaseForm` show the rules
- [x] 4. Tallies: `Match::tally`, `MatchRules::tally` in `apply`, the fact
  `MatchTally<T>`; the engine's tallies (`PointsScored`, `GamesWon`) come with
  the rules producing them

### B. The first sports
- [x] 5. `Scored`: one game with a score, `suggest`, tally `Points`
- [x] 6. Score metrics: `PointsBalance` (scored and conceded), criteria
  `HigherPointsDifference` and `MorePointsScored`
- [x] 7. `BestOf { games }`: tallies `Games`, `Points`

### C. App
- [x] 8. `ResultForm`: view from the template, entering values and parts,
  suggested decision, hints and disagreements as warnings, `report` with the play
- [ ] 9. Views show the play (rounds, bracket, cross table) through `MatchRules::show`
- [ ] 10. (optional) `report_entered` reads scores

### D. Frontends
- [ ] 11. GUI: result dialog from `ResultForm`, scores shown; e2e tests
- [ ] 12. CLI and web: reporting with scores

### E. Release
- [ ] CHANGELOG entries; golden saves of scored and best-of competitions

## Later (separate issues)
- Rosters and players; `lineup` on `Part`
- `Fixture` (leagues) with nested rules
- `Tennis`, `Relay`
- `Advantage` and best-of per stage (Berliner Zeitmodell)
- Result reasons in the UI (walkover, retired)
- Live reporting: `UpdatePlay` on open matches

## Notes for the user
Questions and decisions put off while implementing, to bring up at the end.
- Step 1 raised the file format to 2 (as AGENTS.md demands of any change older
  versions cannot read): 0.3.0 refuses every file of this version, even one
  without any details. Writing the old form while a log has no details would
  keep those readable, at the cost of two forms to write.
- The golden saves now compare a replay with the saved competition *as this
  version reads it* (`as_read`), since the serialized `Match` changed. A field
  the converter silently dropped would go unnoticed there; the engine's tests
  of the old forms cover the conversion itself.
- `Value` is exported at the crate root and clashes with `serde_json::Value`
  wherever both are used (tests alias it). Rename it (`EnteredValue`?) or
  leave it?
- `Value` has `Count` and `Decimal` only; `Duration` (times) waits for a sport
  that needs it.
- Until a second rules plugin exists (step 5), the CLI asks "Results" with one
  choice and the GUI shows "Results: Winner only" in every phase.
- Step 1 was committed without running the manual-stage hooks; a browser test
  it broke was fixed in a commit of its own. Manual hooks are run per step now.
- The GUI e2e test `the_cross_table_shows_the_results_by_rank` failed once in a
  full run and passed alone: possibly flaky.
- `the_page_keeps_its_width_between_views` failed once in a full run and passed
  alone; the phase view grew by the "Results" row, so it may be close to
  scrolling at 1200 px.
- `GamesWon` is tallied, but no metric reads it yet (games won and lost, as
  badminton or volleyball rank by): a calculator like `PointsBalance` when
  wanted.
