# Changelog

What changed for those running competitions with Tournamo, and for the data
they saved. The format follows [Keep a Changelog](https://keepachangelog.com),
and versions follow [Semantic Versioning](https://semver.org).

From 0.1.0 on, competitions saved by an earlier version keep opening in every
later one: a file whose `"format"` changes is migrated, not refused.

## [Unreleased]

### Added

- A competition is played in several phases: each takes its entrants from the
  registration or from places of the standings of phases before it, e.g. a
  qualification, then knockouts of its places 1–8 and 9–16. A phase can be
  named, and removed while no other phase takes entrants from it. A phase is
  started by hand once the phases it takes entrants from are finished; a tie
  through the cut is decided by lot, which the page asks about first. The
  standings draw a line below the places going on, and the ranking of the
  competition places everyone by the phase they reached. Match numbers are
  unique in the whole competition.
- A phase can be played in several groups (divisions) of one configuration,
  dealt by a draw: back and forth, at random, or in blocks (knockouts A/B/C
  playing for places of their own). A seeding orders the entrants of each
  group, which a knockout seeds its bracket by: by rank, at random, or
  keeping groups apart (for a knockout of the best of several groups, so that
  entrants of one group meet as late as possible); it is not asked for random
  rounds and Monster DYP, which draw their pairings by lot. Each place of a group is a place of the phase, a tie in a group decided by
  its lot: the best eight of three groups are the firsts and seconds of every
  group and two of the thirds. Entrants of the same place in different groups
  are compared by criteria of their own, by default (for round robin, random
  rounds and Monster DYP) the new points per match, which compares groups of
  different sizes, then the points, rather than by lot. A phase with too few entrants for its groups is not started until
  the groups are lowered. The page filters the matches by group, shows the
  standings of each group and, where the cut of the next phase goes through a
  place, how that place compares across the groups, a dashed line below the
  place in a group going on as things stand; a bracket or cross table per
  group. The form of a phase taking places of groups says where they lie in
  the groups, e.g. places 1–6 of three groups are places 1–2 of each group.
  Medals go by the places of the competition, so the group tables show none.
  Who won the competition is shown above the phases, not in the phase
  shown.
- What a table of standings shows is chosen per metric: the metrics of a
  phase are a checkbox per table, the group tables and, where groups are
  compared, the comparison. By default a table shows what it is ranked by and
  what that is calculated from (ranked by points also the wins, draws and
  losses), marked by an outlined tick; ticking or unticking shows or hides a
  metric anyway, also while the phase is played, and a metric ticked is
  calculated. The summary of a phase names what is shown or hidden so.
- Entrants can be rated (e.g. by Elo), higher is better: a switch “Ratings”
  above the entrants shows a field per entrant and their seed, and a list
  pasted gives a rating after a `;` or a tab. The start orders the
  registration by rating, equal ratings and the unrated by lot, so a
  knockout seeds its bracket by rating and groups are dealt by it. Only that
  order is saved in the competition's history; the ratings are saved with
  the names, and no longer change once the competition has started. The
  first phase can take only some places of the registration (“Only some
  places”), so the best rated can go straight to a later phase while the
  others play a qualification; the form warns where the lot decides who of
  the registration is in a phase. CLI: `add-entrant --rating`, `rate`.
- Teams (DYP, #1): a phase can be played between teams, its entrants drawn
  into pairs by lot at its start or paired best with worst; a pair plays
  together throughout and in the phases after it, unless a later phase
  splits it up (“everyone on their own”), e.g. for a knockout played singles
  after a DYP, which may take single entrants next to the pairs. Pairs are
  seeded by the places their members came in with.
  Standings, matches and the ranking name a team by its members, and a
  result can be typed with the name of either. In the ranking of the
  competition a pair is one row: a pair out before a phase of singles is
  placed after all of its players, and gets no medal for a place counted in
  pairs. An odd number of entrants is
  not drawn into pairs: the form of the phase and the start say so. The form
  says how many teams the phase forms of its entrants, and what each choice
  of teams does (“unchanged” keeps them as they come in). A Monster DYP takes single
  entrants only. CLI: `teams=drawn-pairs`.
- Presets set up the phases of a whole competition at once, while it has
  none: “groups then knockout” (groups, then a knockout of the best of each,
  groups kept apart), “qualification then knockouts” (random rounds, then
  knockouts by places in blocks) and “DYP” (drawn pairs, double
  elimination). The phases card asks how the competition is played: a
  preset, set up by its fields with a preview of its phases, or own phases.
  The phases added are ordinary phases, each edited and removed as any
  phase; one undo takes all of them back. CLI: `presets`, `add-preset`.
- CLI: `add-phase --name --from`, `start-phase`, `phases`, `ranking
  --competition`, and `--phase` for `matches`, `report`, `undo-report`,
  `ranking` and `edit-phase`; `--division <letter>` for `matches` and
  `ranking`; `add-phase` answers the phase's own fields too, e.g.
  `divisions=3 draw=in-blocks`.

### Changed

- A list of entrants pasted at once, or added by one `add-entrant` of several
  names, is one step of undo and redo: one undo takes all of them back, which
  the page announces as "adding 12 entrants".
- A competition is saved in format 3: each change in its log is written under
  its name, `{"AddEntrant": {...}}`, rather than as its `"type"`
  (`"tournament_engine::add_entrant"`) and `"value"`. Competitions of formats 1
  and 2 open unchanged; earlier versions refuse format 3 as a file of a newer
  version, and cannot open the competitions this version changes in the
  browser.
- A competition is saved in format 4: the competition stored next to the log
  holds each phase's plugins as one `"plugins"` (`"format"`,
  `"match_format"`, `"ranking"`, `"metrics"`), as a phase is added with them,
  and every pairing of a knockout's bracket holds the number of its match.
  Competitions of formats 1 to 3 open unchanged; earlier versions refuse
  format 4.
- A competition is saved in format 5: in the competition stored next to the
  log, a phase holds its matches and standings in its divisions
  (`"divisions": [{"plugins", "data"}]`, one once it has started, none before),
  next to its `"plugins"` as configured, its `"seed"` and whether it has
  `"started"`. A phase and the change adding it hold their `"name"` and
  `"sources"`, the competition its `"lots"`, `"registration"` and
  `"highest_number"`, a division its `"number_base"`, a ranking its `"lot"`, a
  person their `"rating"`; the start holds its `"order"` (`{"order": []}` where
  it was `null`), and the log can hold the new change `"StartPhase"`. The plugins of a phase, of a division and of the change adding
  a phase hold their `"divisions"`, `"draw"` and `"across"` (one division,
  dealt in order, compared by nothing in a competition of an earlier format).
  Competitions of formats 1 to 4 open unchanged; earlier versions refuse
  format 5.
- The group tables show fewer columns by default, also in competitions saved
  before: a metric no criterion of the table ranks by and nothing ranked by is
  calculated from (e.g. the points per match the groups are only compared by,
  or a Sonneborn-Berger score nothing ranks by) is no longer shown. To get one
  back, edit the phase and tick it for the group tables. The rows compared
  across groups also show what their criteria's metrics are calculated from,
  and the CLI's `ranking` lists the columns of the criteria first. In format
  5, a phase and the change adding it hold their `"columns"`.
- Ties in the standings left by every criterion are broken by lot, which an
  existing competition gets as the order it started in, so nothing changes in
  it.
- Library users: the engine reads only the current layout of a competition and
  its log; a file of an older format is converted when read, by
  `tournament_app::file::File::read` (`file::upgrade_competition` for the
  competition alone).
- Library users: undoable-state-machine 0.3. `StateMachine::apply_new` and
  `SessionHistory::apply_new` replace
  `CompetitionStateMachineExt::apply_transition` and `apply_in`; they take a
  `CompetitionTransition`, so a transition goes in with `.into()`.
  `Transition::apply` returns nothing.
- Library users: `PhaseSuggestions` has a third list, `across`, the criteria
  comparing groups, which `PhasePlugins::default_value` and `with_suggestions`
  offer only where the draft answers several divisions; `PhasePlugins::SUGGESTED`
  names `across` too.
- Library users: `Phase::plugins` replaces `Phase::metrics_calculators` and
  `Phase::slice_rankers`, and a `HierarchicalRanker` borrows its slice
  rankers.
- Library users: a `Phase` is played by its divisions once it has started
  (`Phase::divisions`, `Phase::division_of`, `Phase::is_finished`). A
  `Division` has what a phase had: `data`, `standings`, `format`,
  `match_format`, and its `plugins` with the state of its format;
  `Phase::plugins` are the plugins as configured. `DivisionData`,
  `DivisionView` and `DivisionBuilder` replace `PhaseData`, `PhaseView` and
  `PhaseBuilder`.
- Library users: `AddPhase::with_name` and `with_sources`, `StartPhase`,
  `Competition::{lots, field_of, ranking, first_place}`, `Ranking::lot`;
  `reconfigure_phase` takes an `AddPhase`, `remove_phase` is new, and
  `AddPhaseError::PhaseAlreadyAdded` is gone. In the app, `Session` takes the
  phase (`PhaseId`, `MatchesOf`, `DivisionRef`) every view and action is
  about, `add_phase` and `edit_phase` take a `PhaseForm`.
- Library users: `Phase::standings` returns the overall `PhaseStandings`;
  `SliceRanker::rank` takes a `RankingScope` instead of the `DivisionData`;
  `PhasePlugins` has `divisions`, `draw` and `across`, and the plugin axis
  `plugins::draw::Draw` is new. In the app, `Session::compared`,
  `StandingRow::division` and `PhaseOverview::{configured_divisions, order}`.
- Library users: `plugin_factory::downcast_ref` reaches a plugin behind a
  `dyn` of its axis as its type; `PointsCalculator` tells its points
  (`points_for_win`, `points_for_draw`, `points_for_bye`). In the app,
  `CrossRow::total` is the row's points, `None` without them, next to
  `CrossRow::byes` and `CrossMatch::points`.
- Reopening a match that later results depend on, or changing the phase in a
  way that results already reported do not fit, says which matches are in the
  way: all of them at once, rather than one at a time, and of which phase,
  e.g. those of a later phase that other entrants would go on to. The page
  shows why an edit was refused above the buttons of the dialog, rather than
  below its form, out of sight.

### Fixed

- The keyboard can skip what it would otherwise go through stop by stop: the
  first stop of the page skips the bars to the content, and one before the
  list of entrants skips it to the phases (WCAG 2.4.1). Both show only while
  they have the focus.
- A match to play shows its number above the pairing ("Match 3"), where it
  sat between the names and read like a score, and each name centred above
  its button; a played match shows its number first, a dash between the
  names.
- The cross table counts the points of the phase: a result shows its points
  where it shows no score (e.g. 3, 1 or 0), and the last column is the points
  of the standings, byes included, which a column of their own shows where a
  bye scored any. It used to add up 1 for a win and ½ for a draw whatever the
  points of the phase. A phase counting no points shows neither column.
- Where a cut goes through a tie, the line below the places going on names
  who the lot put below it (“Eve: tied, below the line by lot”), so that a
  rank 1 below the line no longer reads as a mistake; the question before
  starting the next phase says who of the tie the lot takes and who not.
- A metric needing what the match format does not keep, e.g. the points
  difference in a phase of “winner only”, says what it needs in turn (“needs
  points scored and conceded, which needs the points scored”), where it named
  only another metric to tick, which would not have done on its own.
- In the browser, undoing or redoing what another tab has undone, redone or
  built on since no longer breaks the page: it says that a change made
  elsewhere keeps it from being undone (or redone), and nothing more can be
  undone (or redone); what this tab did stays undoable as long as no other tab
  touched it.
- Pressing Enter in a score completes it as leaving it does (7 becomes 7–21
  where 21 is the winning score), so the match is reported, as by the button.
- Points of decimals (e.g. 0.1 for a draw) add up to what they are worth,
  whatever the order of the matches: the standings show 0.3, not
  0.30000000000000004, and entrants with equal points or tiebreaks are tied
  rather than ranked apart. Scores are counted to nine decimal places, also
  those of competitions saved before.
- Opening "Type in results" moves the focus into its field, so the first
  result typed right after is no longer lost.
- Start no longer throws away the form of a further phase left open: it waits,
  saying so, until the phase is added or the form discarded by its new button
  "Discard".
- A result reported in the bracket keeps the focus on its pairing, rather than
  moving it to the bracket's heading, so keyboard users keep their place. What
  the last action did is no longer said once another view or phase is shown.
- A result typed in with scores says what the form of the result would warn
  of, e.g. a third game of a best of 3 after 2–0, or a score unlike a game's
  winning score, with what Enter reports; it is still reported, as the form
  reports it. The number of games of a best of is labelled "best of" (German
  "Best of (Sätze)", the match format "nach Sätzen" rather than
  "Gewinnsätze", which read as the games needed to win).
- A name can be corrected after the start: “Entrants” beside “Add phase”
  lists the entrants, each renamed in place, which changes nothing else since
  names are not part of the competition's history. Adding, removing and
  rating stay closed once it has started.

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
