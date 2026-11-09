# Changelog

What changed for those running competitions with Tournamo, and for the data
they saved. The format follows [Keep a Changelog](https://keepachangelog.com),
and versions follow [Semantic Versioning](https://semver.org).

From 0.1.0 on, competitions saved by an earlier version keep opening in every
later one: the tags in a saved log (`tournament_engine::...`) never change, and
a file whose `"format"` changes is migrated, not refused.

## [Unreleased]

## [0.4.0] - 2026-11-09

Competitions saved with 0.3.0 and earlier open unchanged. Earlier versions
cannot open what this version saves: the files it writes, and the competitions
it changes in the browser.

### Added

- Match formats: right after its format, a phase chooses what is entered for
  each match: who won ("winner only", as every phase so far), the score of each
  side (21–17), or the games of a best of (up to 99), each with its score.
  Scores are entered in the row of the match, or in its dialog, which shows who
  wins by them and takes another result only on request; what looks like a
  typo is pointed out but taken. They are typed in like a result
  (`3 21-17 19-21 21-15`) and reported at the command line (`report 3 21-17`).
  Finished matches show their scores in the list of matches, the bracket and
  the cross table, and on a click what was reported; a match reopened keeps
  its scores. The standings show and rank by the points (or goals) scored and
  conceded and by the games won and lost, and their differences.

### Changed

- A competition is saved in format 2: a reported result is saved as a decision,
  so that it can later say why a match was decided and hold its score.
  Competitions of format 1 open unchanged; earlier versions refuse format 2 as
  a file of a newer version.

### Fixed

- In the browser, the button reopening a match no longer takes no click for a
  moment each time its match shows, as on going through the rounds: only right
  after a report, against a double tap.

## [0.3.0] - 2026-11-03

Competitions saved with 0.2.0 and earlier open unchanged. Once a browser has
opened this version, earlier ones can no longer open the competitions stored in
it.

### Changed

- The browser stores each competition as its file, the one it exports,
  rewritten with every change, rather than as a record per log entry and
  person. A competition stored by an earlier version is moved over by its next
  change. The database moves to version 3: a tab of an earlier version still
  open can no longer save and has to be reloaded.
- A competition that cannot be opened says whether it was saved by a newer
  version or is stored damaged, rather than which log entry or name could not be
  read; the detail still says where.

## [0.2.0] - 2026-11-03

Competitions saved with 0.1.x open unchanged. Once a browser has opened 0.2.0,
0.1.x can no longer open the competitions stored in it.

### Added

- An exported file, and a competition the CLI saves, also holds the competition
  as its log played out, so that a later version can still show it should it no
  longer replay the log. Still format 1: older versions ignore it and open the
  file as before.
- The browser stores the same next to each competition. A competition stored
  by an earlier version gets it when it is next opened. The database moves to
  version 2: a tab of an earlier version still open can no longer save and has
  to be reloaded.

## [0.1.1] - 2026-11-03

Competitions saved with 0.1.0 open unchanged.

### Added

- A help page on how a competition is run and on every format, criterion and
  metric, linked from the list of competitions, the entrants and the phase. It
  names the version.
- Entrants are renamed in place; pasting a list adds an entrant per line.
- Before the start, the phase says how many rounds and matches it takes.
- The view (matches, standings, bracket, cross table, phase) is in the URL, so
  a reload and the back button keep it.
- The cross table shows each entrant's total and tints wins and losses.
- Deleting a competition offers to export it first.

### Changed

- The form of the phase lists every choice and reports conflicts after
  selection. A new phase follows the format's suggested ranking and scoring
  until either is edited; "Apply suggested ranking" follows them again. Fields
  about draws are hidden for formats without draws.
- The bracket draws no box for a bye, so it is about half as high, and scrolls
  within itself.
- Rounds of a knockout are named by their stage (semifinals, final), also in
  the standings.
- The standings show the metrics ranked by right after the names, in the order
  of the criteria.
- A Monster DYP needs at least four entrants and takes at most 100 rounds
  (phases saved with more still load).
- Names of entrants differing only in case are refused.
- The command line uses the same operations and wording as the app.
- Plugin authors: ranking and scoring suggestions moved from the `Plugin` trait
  and derive to `PhaseFormat::suggestions`; `plugin-factory`'s `Draft` exposes
  its fields as a tree (`Draft::fields`) instead of `Candidate`s.

### Fixed

- Accessibility: the focus no longer falls to the page after an action,
  actions and results are announced, and buttons, the standings and the
  preview of a typed result are named properly.
- Phones: matches stack their sides, names break only between words, the
  standings keep rank and name in sight, and the bar and views no longer shift
  or get cut off.
- A number a field of the phase does not take keeps the phase from being saved,
  rather than being dropped unnoticed.
- Escape in a dropdown closes only the dropdown.
- A double tap reporting a result no longer reopens the match.
- Errors show at their field and are gone after the next action; a refused
  change of the phase names the match preventing it.
- An import that fails says why; a competition named only by spaces gets the
  file's name.
- Many smaller fixes to the look (dark theme, tabs, touch targets) and to the
  German wording.

## [0.1.0] - 2026-10-29

The first release, hosted at <https://blacktemplar.github.io/tournamo/>.

### Added

- Formats: round robin, random rounds of a round robin, single elimination
  (with an optional match for third place), double elimination (with an
  optional grand final reset) and Monster DYP.
- Rankings by criteria chosen and ordered per phase: points, wins, draws,
  losses, matches played, Buchholz, Buchholz of Buchholz, Sonneborn-Berger, and
  for knockouts the round of elimination or of the first loss.
- Results reported by tap, in the bracket, or typed (`3 1`, `ali`); every
  result and every action of a session can be undone and redone.
- Standings, a bracket and a cross table, for phones, tablets and laptops, in
  light or dark; English and German; WCAG 2.1 AA.
- Offline and installable; competitions stored in the browser, one tab changing
  a competition at a time; export and import as a file (format 1), which the
  command line (`tournament-cli`, also in the browser as `tournament-web`)
  opens and saves as well.
- A help page, which says that nothing entered leaves the device and that there
  is no tracking, that GitHub Pages, which serves the page, sees the IP address,
  and who the media owner is (§ 25 MedienG).

[Unreleased]: https://github.com/blacktemplar/tournamo/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/blacktemplar/tournamo/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/blacktemplar/tournamo/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/blacktemplar/tournamo/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/blacktemplar/tournamo/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/blacktemplar/tournamo/releases/tag/v0.1.0
