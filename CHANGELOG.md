# Changelog

What changed for those running competitions with Tournamo, and for the data
they saved. The format follows [Keep a Changelog](https://keepachangelog.com),
and versions follow [Semantic Versioning](https://semver.org).

From 0.1.0 on, competitions saved by an earlier version keep opening in every
later one: the tags in a saved log (`tournament_engine::...`) never change, and
a file whose `"format"` changes is migrated, not refused.

## [Unreleased]

### Fixed

- Best-of matches count drawn games toward completion and finish early once
  the leading side cannot be caught by the games remaining.

- Entered counts and winning-score hints are limited to 65535. Point tallies
  and standings totals saturate at 4294967295 rather than overflowing, so
  result reporting and replay agree in checked and unchecked builds.

### Added

- A phase has a match format, chosen right after its format: what is entered
  for its matches, who won ("winner only", what every phase entered before),
  the score of each side, or the games of a best of (up to 99), each with its
  score.
- The points (or goals) scored and conceded, and their difference (+5), as
  metrics of the standings, and ranking by the difference, which shows it, or
  by the points scored.
- The games (or sets) won and lost in the matches of a best of, and their
  difference, as metrics of the standings, and ranking by the difference,
  which shows it, or by the games won.
- In the browser, a match whose results hold more than who won is entered in
  the row of the match: the score of each side, or of each game of a best of,
  side by side, reported by Enter or the button beside them. A best of shows
  the fewest games that can decide it, and one more game whenever those
  entered do not decide it yet (the fourth of a best of five at 2–1); games
  entered after the match was decided are pointed out. Its details
  button opens a dialog with what the row holds, for more. What looks
  like a typo is pointed out but taken, such as a score unlike one with the
  winning score of the match format (21–17, 22–20 or 30–29 in a game to 21).
  A score at least two below the winning score, once left, gives the other
  side, if empty, the winning score (15 gives 21 in a game to 21), selected
  when Tab moves there, so that typing replaces it. A value no score can be,
  such as a count above 65535, stays as typed, marked with why, and the match
  is not reported until it is corrected.
  The dialog shows who wins each game and the match by the scores entered;
  a different result is chosen only after asking to override it. Finished
  matches show their scores, in the list of matches and in the bracket; a best
  of shows the games each side won before them (2–1 (21–17 19–21 21–15)), as
  the result of a match being entered does from the first game decided on
  (Not decided yet (1–0), Bob wins (1–3)), always in the order of the sides,
  and a game entered after the match was decided says at which games it was
  (3–0). A click on a finished match with more than who won, in the list of
  matches or in the bracket, shows what was reported for it: each game with
  each side's score, the winner's marked, and the result of the match.
- The cross table shows a match's result as it was entered, from the side of
  the row: its score (21–15), or the games won of a best of (3–2), in place of
  1, ½ and 0, which stay for matches entered by who won; a win is bold as well
  as tinted. Screen readers say the score after how the match ended (won,
  3–2). A click on such a match opens what was reported for it.
- The command line reports scores, e.g. `report 3 21-17 19-21 21-15` (or
  `21–17`, as copied from its list of matches), and lists them with each
  finished match, a best of with its games won.
- In the browser, scores are typed in as well: after the number of the match,
  the left side's first (`3 21-17 19-21 21-15`), or after part of the winner's
  name, the winner's first (`ali 21-17`). A result typed against the scores is
  refused; that takes the dialog.

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

[Unreleased]: https://github.com/blacktemplar/tournamo/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/blacktemplar/tournamo/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/blacktemplar/tournamo/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/blacktemplar/tournamo/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/blacktemplar/tournamo/releases/tag/v0.1.0
