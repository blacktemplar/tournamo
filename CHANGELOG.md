# Changelog

What changed for those running competitions with Tournamo, and for the data
they saved. The format follows [Keep a Changelog](https://keepachangelog.com),
and versions follow [Semantic Versioning](https://semver.org).

From 0.1.0 on, competitions saved by an earlier version keep opening in every
later one: the tags in a saved log (`tournament_engine::...`) never change, and
a file whose `"format"` changes is migrated, not refused.

## [Unreleased]

### Changed

- Renamed from Matchwright to Tournamo, now hosted at
  <https://blacktemplar.github.io/tournamo/>. Competitions stored in the browser
  under the old name are not carried over; export them there and import them
  here.

### Fixed

- A number a field of the phase does not take keeps the phase from being added
  or saved, rather than the number before it being kept unnoticed; why shows
  while it is typed, and the focus goes to the field.
- Escape in the list of a choice closes only the list, no longer the dialog
  around it with the edits made there, and the focus is back at the choice. The
  option highlighted in such a list stands out clearly.
- A double tap reporting a result no longer reopens the match at once: the
  button reopening it takes no tap for a moment after it shows.
- The views (matches, standings, bracket, phase) stay right below the bar, with
  nothing showing through between them, and fit on one line on the narrowest
  phones; the bar no longer shifts between screens that scroll and those that
  do not.
- The focus no longer falls to the page: a new competition opens at the field of
  the first entrant, the start moves to the matches, removing an entrant moves
  to the next one, and a result reported in the bracket moves to its heading
  rather than to the next match, where one more Enter reported it. The edit of
  the phase opens at its format, and the question whether to delete a
  competition at Cancel, as an alert dialog.
- Adding or removing an entrant, adding or changing the phase and the start are
  announced, and the announcement of the last result names who won.
- Screen readers hear a draw button by its match ("Alice – Bob: draw"), the
  preview of a result typed in as it changes, the standings by their caption
  with each row by its name, and the switch between the matches playable now
  and those by round by its name. Buttons showing only their icon show what
  they do under the pointer.

## [0.1.0] - 2026-10-28

The first release, hosted at <https://blacktemplar.github.io/matchwright/>.

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

[Unreleased]: https://github.com/blacktemplar/tournamo/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/blacktemplar/tournamo/releases/tag/v0.1.0
