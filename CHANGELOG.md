# Changelog

What changed for those running competitions with Tournamo, and for the data
they saved. The format follows [Keep a Changelog](https://keepachangelog.com),
and versions follow [Semantic Versioning](https://semver.org).

From 0.1.0 on, competitions saved by an earlier version keep opening in every
later one: a file whose `"format"` changes is migrated, not refused.

## [Unreleased]

### Changed

- **Settings by level.** The form of a phase shows the settings of the level
  chosen above it, advanced or expert, which the browser remembers. The draw,
  the seeding and how groups are compared show only to experts; below that,
  the form says how many of them differ from the defaults.

## [0.5.0] - 2026-11-21

Competitions saved with 0.4.0 and earlier open unchanged. Earlier versions
cannot open what this version saves: the files it writes, and the competitions
it changes in the browser.

### Added

- **Phases.** A competition is played in several phases, each taking its
  entrants from the registration or from places of the phases before it, e.g.
  a qualification, then knockouts for places 1–8 and 9–16. A phase is named,
  started by hand once the phases it takes entrants from are finished, and its
  start can be undone while it has no result. The standings draw a line below
  the places going on; where it goes through a tie, the lot decides who goes
  on (asked first), while the tied keep their shared rank. The ranking of the
  competition places everyone by the phase they reached. Match numbers are
  unique in the whole competition.
- **Groups.** A phase can be played in several groups of one configuration,
  dealt by a draw (back and forth, at random, or in blocks playing for places
  of their own) and seeded by rank, at random or keeping groups apart. The
  same place of different groups is compared by criteria of its own, by
  default the points per match. Matches, standings, bracket and cross table
  are shown per group.
- **Ratings.** Entrants can be rated (e.g. by Elo), also in a pasted list
  after a `;` or a tab. The start orders the registration by rating, which
  seeds brackets and deals groups; the best rated can go straight to a later
  phase ("Only some places"). Ratings are saved with the names, not in the
  competition's history.
- **Teams (DYP, #1).** A phase can be played in pairs, drawn by lot at its
  start or paired best with worst, which stay together in later phases unless
  one splits them up, e.g. for a knockout of singles after a DYP. A team is
  named by its members, and a result can be typed with either name.
- **Presets** set up all phases of a competition at once: "groups then
  knockout", "qualification then knockouts" and "DYP". They add ordinary
  phases, which one undo takes back.
- **Columns of the standings** are chosen per metric and table. By default a
  table shows what it is ranked by and what that is calculated from; "Add
  metric" adds others.
- **Editing a result.** A finished match is corrected with "Edit"; results
  reported after it stay as long as they still fit, otherwise the dialog names
  the matches in the way.
- **Printing and copying.** The page prints what it shows without its
  controls, and a table of standings or the ranking can be copied as text,
  which pastes into a spreadsheet or a message.
- Names can be corrected after the start ("Entrants" beside "Add phase").
- CLI: `add-phase --name --from`, `start-phase`, `phases`, `presets`,
  `add-preset`, `add-entrant --rating`, `rate`, and `--phase` and
  `--division` for the commands about a phase.

### Changed

- A competition is saved in format 5 (0.4.0 saved format 2): the log names
  each change, and the competition stored next to it holds its phases with
  their divisions, its lots and the ratings.
- A new round-robin phase scores no point for a bye, which with an odd number
  of entrants everyone gets; phases added before keep their points.
- Double elimination from 17 entrants on drops the losers of the winner
  bracket in an order keeping entrants who met apart for longer (as Challonge
  does). Phases added before keep their order until updated in their form
  ("Update to the latest version").
- Single elimination can show and rank by the round of elimination.
- A reopened match starts out empty: a result is corrected with "Edit"
  instead. Undoing a report still keeps its result in the form.
- Entering: a score in the row of a match shows below each side's name; a
  pasted list of entrants adds every line it can, lists the others with why,
  and is one step of undo. A refused reopening or change of a phase names
  every match in the way at once.
- The help covers what testers looked for (the views, how far undo reaches,
  typing in a draw, column headers, teams, exported files). German terms are
  used alike everywhere, and lists of options follow their German labels.
- Library users: the engine reads only the current layout, older files are
  converted by `tournament_app::file::File::read`; undoable-state-machine 0.3
  (`StateMachine::apply_new` and `SessionHistory::apply_new` replace
  `apply_transition` and `apply_in`); a started `Phase` is played by its
  `Division`s (`DivisionData`, `DivisionView`, `DivisionBuilder` replace the
  `Phase*` ones) and ranked by `PhaseStandings`, `SliceRanker::rank` takes a
  `RankingScope`; `PhasePlugins` holds `teams`, `divisions`, `draw`, `seeding`
  and `across`; `Points` holds the points of matches and byes apart. In the
  app, `Session` takes the phase every view and action is about.

### Fixed

- Typed results: Enter in a score completes it (7 becomes 7–21) and reports
  the match; "Type in results" takes the focus; what is wrong with a line is
  named (a match waiting for others, a number too large, no score) rather
  than looked up as a name, and what the result's form would warn of is said.
  A name only part of a longer one in the match typed is refused where an
  entrant of exactly that name plays another.
- Points with decimals (0.1 for a draw) add up exactly, so equal points tie.
  The cross table counts the points of the phase rather than 1 per win and ½
  per draw.
- Undoing or redoing in one tab what another has changed since says so
  rather than breaking the page; a tab takes the name another gave the
  competition.
- Settings that do not fit together say why in plain words and what fits
  instead, also a metric the match format cannot feed.
- Numbers are read as the page's language writes them (German `1.800,5`);
  names are tidied of double spaces and have at most 100 characters.
- A click inside a dialog no longer closes it, and dialogs editing or adding
  a phase ask before discarding changes.
- The bracket outlines the match to play now, keeps the focus on a pairing
  reported, scrolls with the page, leaves out a match for third place against
  nobody, and names the rounds of a double elimination by stage ("Winners
  semifinals, Losers round 3").
- Layout and accessibility: skip links (WCAG 2.4.1); a match's number above
  its pairing; lists of choices and long texts stay within a phone's screen,
  where help of a field shows at its button; on a low screen only the bar
  stays on top; the focus lands where it belongs, without taking it from a
  field typed into; the cross table has a caption.
- Odds and ends: an import says the name it gave; the form warns of a phase
  of more than 50 rounds and says the limits of its numbers (a winning score
  at most 65535, points at most 1000); "winner only" no longer says a match
  may be drawn where the format allows no draws; the games of a best of are
  labelled "best of".

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

[Unreleased]: https://github.com/blacktemplar/tournamo/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/blacktemplar/tournamo/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/blacktemplar/tournamo/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/blacktemplar/tournamo/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/blacktemplar/tournamo/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/blacktemplar/tournamo/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/blacktemplar/tournamo/releases/tag/v0.1.0
