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
- [ ] 2. `Part`, `Sides`, `Value`, `FieldKey`: `Match` holds `contest: Part`
  (its annotations move in); `ReportMatchResult` gets `values` and `parts`;
  old snapshots load; benchmark `large_tournament`
- [ ] 3. `MatchRules` axis: `Template`, `Level`, `Parts`, `FieldSpec`, `Hints`,
  plugin `Outcome`; `PhasePlugins.rules` (old phases load as `Outcome`);
  `ReportMatchResult` checks the structure against the template; questionnaire
  and `PhaseForm` show the rules
- [ ] 4. Tallies: `Match::tally`, `MatchRules::tally` in `apply`, `Points` and
  `Games` with compatibility facts

### B. The first sports
- [ ] 5. `Scored`: one game with a score, `suggest`, tally `Points`
- [ ] 6. Score metrics: points for/against, criterion on the difference
- [ ] 7. `BestOf { games }`: tallies `Games`, `Points`

### C. App
- [ ] 8. `ResultForm`: view from the template, entering values and parts,
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
