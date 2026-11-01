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

- A Monster DYP takes at most 100 rounds, all of which it schedules at once; a
  phase saved with more still loads, and editing it asks for fewer.
- A Monster DYP starts with four entrants at least, rather than finishing at
  once with fewer.

### Fixed

- An imported file naming its competition by nothing but spaces is named after
  the file, or else "Competition", rather than shown as an empty entry of the
  list.

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
- On a phone, a match stacks its sides, each name on a line of its own beside
  its button, so that names break only between words. A match played keeps a
  solid border, the dashed one being for placeholders, and its loser recedes
  behind its winner.
- The standings show the metrics the phase is ranked by right after the names,
  in the order of its criteria, and the matches played last; on a phone the
  rank and the name stay in sight while the rest scrolls. The metrics of an
  entrant yet to play recede.
- The warning that the browser may not keep the competitions takes one line
  until unfolded, with buttons big enough to tap. What an action did shows in a
  toast at the bottom for a while, rather than in a line that pushed the page
  down, and the banner of the winners shows above the standings, so that the
  matches no longer move when the last result is reported.
- Why an action was refused is gone once the next action is done, also on the
  list of competitions and in a dialog, and no longer said again when another
  language is chosen.
- A change of the phase refused for a result it does not fit names the match of
  that result and says that reopening it allows the change. A change that keeps
  results says how many, and the dialog says in plain words that results stay.
- What is said about a plugin or a field, and the hint below the entry of
  results, reads as sentences. Each criterion of the ranking says when to rank
  by it, rather than what its metric is, and a metric calculated since a
  criterion ranks by it names that criterion.
- A round of a knockout whose matches are all of one stage is named by it (the
  semifinals, the final), in the list of rounds, above its matches and in the
  standings, which name the round an entrant lost in by the stage of its match.
- A file that cannot be imported says whether it is no competition at all, one
  of a newer version, or a damaged one.
- The question whether to delete a competition offers to export it first.
- An empty or taken name of an entrant, and an empty name of a new competition,
  are refused at their field, in one message instead of two.
- German: "Sieger" for the winner, "höhere Sonneborn-Berger-Wertung" (and so
  for Buchholz), "Vorherige Runde", and a clearer hint below the entry of
  results.

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
