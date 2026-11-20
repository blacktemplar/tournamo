# What the GUI says in English. Why an action was refused, and the plugins, are translated by
# tournament-app.

app-title = Tournamo
# the title of the page while a competition is open
competition-title = { $name } – Tournamo
language = Language
# whether the page is light or dark
theme = Colours
theme-system = Like the system
theme-light = Light
theme-dark = Dark
undo = Undo
redo = Redo
# under the pointer on Undo while there is nothing to undo, e.g. after a reload
undo-since-opened = Undo covers what was done here since this competition was opened. To change a result reported before, reopen its match.
# undoing the start leaves play, so it is asked about first
undo-start-title = Undo the start?
undo-start-description = The competition goes back to its setup, where the entrants and the phase can be changed. Redo starts it again.
undo-start = Undo the start
# $phase: what a phase is called, its name or else its number and format
undo-start-named = Undo the start: { $phase }
# undoing the start of a phase that started with the competition, asked about first
unstart-description = The competition goes back to its setup, where the entrants and the phases can be changed. Starting it again draws the same lots.
# the browser refused to keep what is stored, which a private window does as well; the first line,
# which stays shown when the warning is folded, then what it unfolds to
not-persisted-summary = This browser may not keep the competitions stored here.
not-persisted = For example in a private window, or when it runs short of space. Export them to keep a copy.
# shown while the browser offers to install the page
not-persisted-install = Installed as an app, the page is usually allowed to keep them.
export-this-competition = Export this competition
install = Install the app
# hides the warning for good
dont-show-again = Don’t show again
# the errors below with a reason from the browser or the storage show it, in English, folded away
# below this
error-detail = Details
error-no-storage = the browser does not let the page store competitions
error-not-saved = the changes could not be saved
error-not-exported = the file could not be made
# a link to the repository, shown with GitHub's icon
source-code = Source code

## The help

help = Help
# the title of the page while the help is shown
help-title = Help – Tournamo
help-running = Running a competition
help-running-steps = A competition is run in four steps:
help-running-entrants = Enter the entrants by name, one after the other, or paste a list with a name on each line. A rating may follow a name after a “;”, e.g. “Alice;1500”. Of a list, the lines that cannot be added are listed with why; the others are added.
help-running-phase = Choose how they play: a preset, which sets up all phases at once, e.g. groups then a knockout, or a phase of your own: the format, and the order of the standings. Then add the phases.
help-running-start = Start. The format schedules the matches, and the entrants are fixed from then on, though their names can still be corrected under “Entrants”. The phase can still be changed, as long as the change fits the results reported.
help-running-results = Report the results: at a match, choose the side that won, or Draw, or enter its score in its row; or type a result in under “Type in results”, e.g. “3 1” for a win of the left side of match 3, “3 x” for a draw, or part of the winner's name; or choose the winner in the bracket. The standings follow every result.
help-running-views = A phase shows its matches, those playable now or round by round, and its standings, with a line below the places going on to a later phase. A knockout also shows its bracket; a phase whose entrants meet in pairs, such as a round robin, a cross table of every result of each entrant against each other. The ranking places everyone by the phase they reached.
help-running-mistakes = A result reported by mistake is taken back by reopening its match. Undo takes back what was done since the competition was opened, in the order it was done. Either way, the score entered stays, so that only what was wrong needs changing.
help-running-undo = Undo goes back only as far as the page was open: reloading the page or leaving the competition forgets what could be undone, though not what was done. Reopening a match works at any time, and so does undoing the start of a phase once its matches are reopened (under Phase). Undoing the start of a phase that started with the competition, or the start itself with Undo, takes the competition back to its setup (it asks first), where the entrants and phases can be changed. Starting again draws the same lots, groups, pairs and brackets: what is drawn is fixed when the competition is created, so that a draw cannot be repeated until it suits someone, and changes only with what it is drawn of, e.g. the entrants or the phase.
help-running-byes = Where a round leaves an entrant without an opponent, the entrant has a bye: it moves on without a match and scores as the phase says, e.g. the points for a bye.
help-formats = Formats and standings
help-presets = Presets
help-presets-text = A preset sets up a whole competition at once, while it has no phase: choose it under “How is it played?”, answer its few questions, and add the phases it shows. They are ordinary phases, each changed or removed as any phase; one undo takes all of them back.
help-standings = The order of the standings
help-standings-text = The standings compare the entrants by the criteria of their order in turn: the first decides, the second only between entrants the first leaves tied, and so on, e.g. more points, then more Sonneborn-Berger, a tie break favouring those who scored against strong opponents. A criterion compares a metric, a number calculated for each entrant from the results, such as the points; the metrics a criterion needs are calculated with it. Further metrics can be calculated and shown without ranking by them. The “i” next to a short column header, such as SB, says what the column counts; every criterion and metric is explained below.
help-columns-text = A table of standings shows the metrics it is ranked by and those they are calculated from, e.g. ranked by points also the wins, draws and losses the points are scored for. Where a phase compares several groups, the rows compared show what they are compared by. The metrics of a phase are those it is ranked by, those these need, and those added under “Add metric”; a metric needed comes and goes with what needs it, one added stays until it is removed. A box with an outlined tick is shown by this rule; tick or untick it to show or hide the metric anyway, in the group tables or in the comparison. This can be changed at any time, also while the phase is played.
help-groups = Groups
help-groups-text = A phase can be played in several groups, each on its own in the same format; the draw deals the entrants to them, and the seeding orders those of each, which a knockout seeds its bracket by: “groups kept apart” lets entrants of one group meet as late as possible. Groups dealt in blocks play for places of their own, e.g. knockouts A and B for places 1–4 and 5–8. Otherwise the groups stand side by side: the firsts of all groups come first, then the seconds, and so on (who is tied in their group placed by lot), and where a next phase takes only some of one place, e.g. the best two of three thirds, they are compared by the criteria across the groups, suggested for formats ranking by points: the points per match, which suit groups of different sizes, then the points.
help-seeding = Seeding
help-seeding-ratings = Entrants can be rated, by any number where higher is better, e.g. an Elo rating, written as the page’s language writes numbers (1,800.5): switch on “Ratings” above the entrants. The start orders the registration by rating, best first; entrants of equal ratings, and those without one after the rated, are ordered by lot. A knockout of the registration seeds its bracket in this order, so that the best rated meet as late as possible.
help-seeding-places = A phase can take only some places of the registration: the best rated can go straight to a main phase while the others play a qualification for the remaining places. Where such a cut goes through entrants of equal ratings or none, the lot decides who is on which side, which the form of the phase says.
help-seeding-draw = Played in groups, the draw deals the registration to them in its order, e.g. back and forth, so that the best rated end up in different groups; the seeding then orders the entrants of each group.
help-seeding-log = The ratings are kept with the names, not in the competition's history: it holds only the order the start gave the registration. Once started, the ratings no longer change.
help-teams = Teams (DYP)
help-teams-text = A phase can be played between teams: with “drawn pairs” the entrants are drawn into pairs by lot at its start, which play together throughout, as in a DYP (draw your partner); with “best with worst” the first is paired with the last, the second with the second last, and so on, by rating from the registration, by place from a phase. Either needs an even number of entrants, else the phase does not start. “Unchanged”, the default, plays everyone as they come in: on their own from the registration, in their teams from a phase played in teams. The standings, the matches and the ranking name a team by its members, and a result can be typed with the name of either.
help-teams-later = A team stays together in the phases after it, unless a later phase splits it up with “everyone on their own”: e.g. a DYP, then a knockout played singles for the one winner. Only a phase splitting teams up takes teams of different sizes, e.g. the pairs of a DYP and the best of the registration. A Monster DYP draws new partners for every match, so it takes single entrants only.
help-criteria = Criteria
help-metrics = Metrics
help-stored = Your data stays on your device
help-stored-here = Tournamo stores all competitions locally in your browser (in its IndexedDB). Nothing you enter leaves your device: no data is sent to a server, there is no tracking, and no account is required.
help-stored-hosting = The page itself is delivered by GitHub Pages, which, like any web server, receives your IP address when you load it.
help-stored-lost = Your competitions are therefore only available in this browser on this device. They are deleted when you clear your browser’s site data or close a private window.
help-stored-export = To create a backup or continue on another device, export the competition as a file and import it there, with “Import from a file” in the list of competitions; one named like a competition stored already gets a number added, e.g. “Cup (2)”. The file holds everything that was done, which the import plays through again, and the names; it is not meant to be read.
help-offline = Offline use and installation
help-offline-text = Once loaded, Tournamo also works without an internet connection. You can install it as an app from your browser’s menu; installed apps are usually allowed to keep their data permanently.
help-tabs = If a competition is open in several tabs, it can only be edited in one of them at a time. The other tabs show it read-only and update automatically.
help-open-source = Open source
help-media-owner = Media owner (disclosure under § 25 MedienG): { $owner }, Austria.
help-open-source-text = Tournamo is free, open-source software, licensed under MIT or Apache 2.0. The source code is available on GitHub, where you can also report bugs and suggest improvements.
help-version = This is version { $version }.

## The competitions stored

competitions = Competitions
no-competitions = No competition is stored in this browser yet.
# how far a competition of the list is
progress-setup = Not started · { $entrants ->
        [one] one entrant
       *[other] { $entrants } entrants
    }
# $round: the first round with a match to play; $open: its matches to play
progress-playing = Round { $round } · { $open ->
        [one] one match to play
       *[other] { $open } matches to play
    }
progress-finished = Every match played
# $winners: what the message winners says
progress-won = Finished · { $winners }
export = Export
export-competition = Export { $name }
rename = Rename
rename-competition = Rename { $name }
rename-title = Rename { $name }
# said while a name is typed that another competition has; it is taken all the same
competition-name-taken = Another competition is named “{ $name }” already.
delete = Delete
delete-competition = Delete { $name }
delete-title = Delete { $name }?
delete-description = Everything stored of it in this browser is removed. Export it first to keep a copy.
error-competition-open-elsewhere = the competition is open in another tab; close it there first
error-not-deleted = the competition could not be deleted
new-competition = New competition
# what the page is for, above the new competition
intro = Tournamo runs a tournament in your browser: enter the entrants, choose a format, start, and report the results.
# a link to the help on running a competition
how-it-works = How it works
# a link to the help on the formats and the order of the standings
formats-explained = Formats and standings explained
create = Create
import = Import from a file
error-empty-competition-name = a competition needs a name
error-file-not-read = the file could not be read
error-competitions-unreadable = the competitions stored cannot be read

## The competition open

opening = Opening the competition…
all-competitions = All competitions
# what can be done with the competition as a whole
competition-actions = Competition
read-only = This competition is open in another tab, so it is only shown here. It follows what is done there, and can be changed here once that tab leaves it.

## Before the start

# the dialog listing the entrants after the start, to correct a name
entrants-unnamed = { $count ->
    [one] An entrant has
   *[other] { $count } entrants have
} no name; give them one under “Entrants”.
entrants-after-start = The competition has started, so its entrants are fixed; their names can still be corrected.
entrants = Entrants
no-entrants = No entrants yet. Each one is added by its name, or several at once by pasting a list with a name on each line, followed by a rating after a semicolon or a tab where there is one.
name = Name
add-entrant = Add entrant
# the name typed looks like that of an entrant, e.g. differing in case only, so that it is refused
entrant-like = “{ $name }” is entered already.
# $count: how many lines of a list pasted were not added, each listed below with why
lines-not-added = { $count ->
        [one] This line was not added; the others were:
       *[other] These { $count } lines were not added; the others were:
    }
remove = Remove
remove-entrant = Remove { $name }
rename-entrant = Rename { $name }
ratings = Ratings
ratings-locked = On while an entrant has a rating.
rating-of = Rating of { $name }
seed = seed { $seed }
ratings-note = Entrants are seeded by rating, higher first; those without one follow by lot.
seeding-explained = Seeding explained
# the field of the name an entrant is renamed to
entrant-new-name = New name of { $name }
# not `phase`, which is the app's message of the fields of a phase
phase-heading = Phase
how-played = How is it played?
own-phases = Own phases, one by one
preset-preview = Phases added
add-phases = Add phases
# the heading once every match is played
finished = Finished
# the medals of the first three ranks in the standings, before the rank
medal-gold = Gold medal,
medal-silver = Silver medal,
medal-bronze = Bronze medal,
add-phase = Add phase
add-phase-description = The phase takes its entrants from the registration or from places of the phases before it.
new-phase = New phase
# the button discarding the form of a new phase, before it is added
discard-phase = Discard
# the heading of the card once a competition has several phases
phases = Phases
remove-phase = Remove phase
# $phase: what a phase is called, its name or else its number and format
edit-phase-named = Edit phase: { $phase }
remove-phase-named = Remove phase: { $phase }
# where a phase takes its entrants from
everyone-registered = everyone registered
only-some-places = Only some places
registration = the registration
every-place-of = every place of { $phase }
places-of = places { $from }–{ $to } of { $phase }
places-from-of = places from { $from } of { $phase }
registration-places = places { $from }–{ $to } of the registration
registration-places-from = places from { $from } of the registration
# how long the phase takes with the entrants entered, before the start; $rounds and $matches: what
# the messages below say
phase-scale = With { $entrants } entrants, the phase takes { $rounds } and { $matches }.
phase-scale-teams = With { $entrants } entrants in { $teams } teams, the phase takes { $rounds } and { $matches }.
scale-rounds = { $count ->
        [one] one round
       *[other] { $count } rounds
    }
# a number depending on the results, e.g. whether a grand final is played again
scale-rounds-range = { $least } to { $most } rounds
scale-matches = { $count ->
        [one] one match
       *[other] { $count } matches
    }
scale-matches-range = { $least } to { $most } matches
# too many to play the phase out beforehand
scale-rounds-at-least = at least { $count } rounds
scale-matches-at-least = at least { $count } matches
start = Start
# why the start waits while the form of a further phase is open
start-new-phase-open = Add the new phase, or discard it, first.

## Configuring the phase

# the name of a phase, optional; without one, a phase is called by its number and format
phase-name = Name
entrants-from = Entrants from
# a row taking a range of places; $number counts the rows from 1
source-of = Source { $number }
from-place = from place
to-place = to place
# what the last place of a range shows while it is empty
to-last-place = last
remove-source = Remove source { $number }
add-source = Add source

# what a choice of a plugin shows while none is chosen
choose-plugin = Choose…
# the choice of no plugin, and what the phase shows for a field without one
no-plugin = (none)
# next to a plugin the plugins chosen elsewhere recommend
recommended = (recommended)
# next to a plugin of which a later version has come out, which a phase added before keeps
earlier-version = (earlier version)
# $plugins: the plugins of a phase of which a later version has come out
outdated-plugins = This phase is played with an earlier version of { $plugins }. Updated, it is played with the latest version, if its results fit that.
update-plugins = Update to the latest version
following-suggestions = Ranking and scoring follow suggestions for the format until you edit them.
customized-settings = Your ranking and scoring settings are kept when the format changes. Applying suggestions replaces them and resumes automatic updates.
apply-suggested-ranking = Apply suggested ranking
# next to the choice of a plugin added to a list
add-plugin = Add
# the button showing what is said about $subject, a plugin or a field
about = About { $subject }
# $plugin: a plugin in a list
move-up = Move { $plugin } up
move-down = Move { $plugin } down
remove-plugin = Remove { $plugin }
# $plugin: a plugin added to a list since another one needs it
implied-plugin = { $plugin }, since another one needs it
# $needed_by: the plugins needing it, as a list
implied-plugin-needed-by = { $plugin }, needed by { $needed_by }
# the tables of standings a metric is shown in, heading a column of checkboxes
metrics-name = Metric
shown-in-divisions = Group tables
shown-in-comparison = Comparison
# $metric: a metric, the name of its checkbox where there is a column per table
show-in-divisions = Show { $metric } in the group tables
show-in-comparison = Show { $metric } in the comparison
# $needed_by: the plugins needing a metric, as a list
metric-needed-by = needed by { $needed_by }
# the choice of a metric to add to a phase
add-metric = Add metric
# the choice of the calculator of a metric several can calculate
metric-provided-by = Calculated by
# a metric added that is shown nowhere and needed by nothing
metric-unused = not shown and not used
# why a metric is shown by rule, said of its checkbox
shown-ranked-in-divisions = shown since the standings are ranked by it
shown-ranked-in-comparison = shown since the rows compared are ranked by it
# $metric: a metric shown, calculated from the metric of the checkbox
shown-calculated-from = shown since it goes into { $metric }
# the summary of a phase: the metrics shown or hidden against the rule
columns-also-shows = Also shows
columns-hides = Hides
columns-compared-also-shows = Comparison also shows
columns-compared-hides = Comparison hides
yes = yes
no = no
edit-phase = Edit phase
edit-phase-title = Edit the phase
edit-phase-description = The results reported so far stay. A change that does not fit them is refused.
save-changes = Save changes
cancel = Cancel

# a dialog with the form of a phase asking, on Escape or a click beside it, before it loses what was changed
discard-changes-title = Discard the changes?
discard-changes-description = What was changed in the form is lost.
discard-changes = Discard changes
keep-editing = Keep editing

## After the start

# the state of a phase, next to its link, for screen readers
phase-not-started = not started:
phase-running = running:
phase-done = finished:
start-phase = Start { $phase }
phase-finished = { $phase } is finished
# a cut between the places taken and those not taken goes through a tie
tie-at-cut-title = Start with a tie decided by lot?
tie-at-cut-description = To decide it otherwise, cancel and add a criterion to the ranking of the phase first.
start-anyway = Start anyway
# the ranking of the competition: every entrant's place
ranking = Ranking
ranking-provisional = As things stand: not every phase is finished.
place = Place
phase-reached = Phase reached
# the line below the rows of the standings going on to another phase: by their place, or the
# place going on as things stand by the comparison of the groups; and in that comparison
cut-by-place = { $count ->
    [one] Place { $from } goes
   *[other] Places { $from }–{ $through } go
} on to { $phase }
cut-compared = Place { $place } goes on to { $phase } as things stand, compared across the groups
cut-comparison = Go on to { $phase } as things stand
# $names: the teams right below the line that share the rank of the last above it
cut-below-by-lot = { $names }: tied, below the line by lot

matches = Matches
# what a narrow screen shows: the matches or the standings (or a knockout's bracket); a wide screen
# shows both, or the bracket
view = View
matches-and-standings = Matches & standings
# the lists of matches to choose from: those playable now, and each round
rounds = Rounds
round = Round { $round }
# a round playing several stages side by side, e.g. "Round 2: winners round 2, losers round 1"
round-of-stages = Round { $round }: { $stages }
# $round: the name of the round, as `round` says it or the stage it is of
round-with-open = { $round } ({ $open } open)
# the matches whose entrants have played every round before
playable-now = Playable now
playable-now-count = Playable now ({ $count })
# shows the matches of one round at a time
by-round = By round
# the group of the two buttons above, for screen readers
matches-shown = Matches shown
previous-round = Previous round
next-round = Next round
nothing-playable = No match can be played now.
# said by screen readers before the number of a match, and between its sides
match = Match
versus = against
# what a screen reader says for the button reporting that a side won
wins = { $team } wins
# what that button shows, below the side's name; it must be a part of `wins`
wins-short = Wins
draw = Draw
# the button reporting a draw, for screen readers
draw-of = { $a } – { $b }: draw
won = { $team } won
# a result and the scores it was reported with; $result: what `won` or `draw` say
result-with-scores = { $result }, { $scores }
reopen = Reopen
reopen-match = Reopen match { $number }
# the result of a match whose match format enters more than who won, e.g. a score, in a dialog
open-result = Enter result
open-result-of = Enter result of match { $number }
# the button opening the dialog of a result entered in the row of its match, for more than the row enters
result-details = Details
# what screen readers say of it
result-details-of = Details of the result of match { $number }
# the button reporting the result entered in the row of a match, for screen readers
report-match = Report match { $number }
# a value entered in the row of a match, for screen readers; $value: what it is, e.g. Score
quick-value-of = { $value } of { $team }, match { $number }
# the same of a part of the match; $part: e.g. Game 2
quick-value-of-in = { $value } of { $team } in { $part }, match { $number }
result-title = Result of match { $number }
# the dialog showing the result of a finished match as it was reported
result-details-description = { $a } against { $b }, as reported.
# the header of the column saying who won each part, e.g. each game, where it has no values
result-column = Result
# a warning of a part, e.g. of game 2; $warning: a sentence
part-warning = { $part }: { $warning }
# a value of the match itself; $value: e.g. Score; $score: e.g. 21–17, $a's first
reported-value = { $value }: { $a } { $score } { $b }
close = Close
result-description = { $a } against { $b }. Each value is taken as entered; what looks like a typo is pointed out.
part-numbered = { $name } { $number }
value-of = { $value } of { $team }
value-of-in = { $value } of { $team } in { $part }
result-of = Result: { $part }
result-as-entered = As entered: { $result }
result-open = Not decided yet
result-is = { $part }: { $result }
# a result, or that there is none yet, with the parts each side won in the order of the sides, e.g. 2–1 in games; $result: what `wins`, `draw` or `result-open` say
result-with-parts-won = { $result } ({ $won })
# a button, rarely used: choosing results other than those the values suggest
result-override = Override result
# a button, while overriding: dropping the results chosen
result-follow-values = Use result as entered
bye = Bye: { $team }
bye-in-stage = Bye: { $team } ({ $stage })
standings = Standings
rank = Rank
# the header of how many matches an entrant has a result of
played = Played
# a table with a row and a column per team, each cell the matches of the row's team against the
# column's
cross-table = Cross table
# the column of the points of a row, those of its results and of its byes, as the standings show
# them
cross-table-total = Points
# the column of the points of the byes of a row, where a bye scored points
cross-table-byes = Byes
# said to screen readers for the cells that show nothing: of a team against itself, of two that
# do not meet, of a match without a result
cross-table-self = same
cross-table-no-match = no match
cross-table-open = to be played
# how a match ended for the team of the row, said by screen readers for what it shows: 1, ½ and 0,
# or the points of the result
outcome-win = won
outcome-draw = drawn
outcome-loss = lost
# how a match ended for the team of the row, followed by the score shown for it, said by screen
# readers; $outcome: what `outcome-win`, `outcome-draw` or `outcome-loss` say; $score: e.g. 2–1, the
# row's first
outcome-with-score = { $outcome }, { $score }
# how a match ended for the team of the row, followed by its points for it; $points: e.g. 3
outcome-with-points = { $outcome }, { $points ->
        [one] { $points } point
       *[other] { $points } points
    }
# the bracket of a knockout, every match in it from the start
bracket = Bracket
bracket-winners = Winners bracket
bracket-losers = Losers bracket
bracket-grand-final = Grand final
# above the bracket, while a match in it has no result: the names of its sides are buttons
bracket-report-hint = Choosing the name of the side that won reports the result.
# a side of a match in the bracket that nobody takes
bracket-bye = Bye
# the tag of a side that moved on without a match, in place of a pairing against nobody
bracket-after-bye = bye
# a side of a match in the bracket not decided yet, by the match or else the stage it comes from
winner-of-match = Winner of { $number }
loser-of-match = Loser of { $number }
winner-of-stage = Winner of { $stage }
loser-of-stage = Loser of { $stage }
# a match of a stage of several, by its place from the top of the stage's column
stage-position = { $stage } ({ $position })
# a match of the bracket only played if the one before ends a certain way, e.g. a grand final reset
if-needed = if needed
# a pairing only played if needed, once it is not
bracket-not-needed = not needed
# reporting a result by typing it
# what opens the field reporting results typed in
type-results = Type in results
enter-result = Enter a result
# what the toggletip holding the hint below is about, once a result typed in was reported
typing-results = typing in results
# $draws: yes if a match may end in a draw, else no
enter-result-hint = Type the number of the match, then 1 if the side on the left won{ $draws ->
        [yes] , X for a draw,
       *[no] {""}
    } or 2 if the side on the right won, e.g. “3 1”. Or type part of the winner’s name, with the number of the match if it fits several{ $draws ->
        [yes] {" "}and X for a draw, e.g. “ali”, “ali 5” or “ali X”.
       *[no] , e.g. “ali” or “ali 5”.
    }
# what the entry reports if submitted now; $result: what the message won or draw says
enter-scores-hint = Scores follow the number, the left side’s first, or the name, the winner’s first, e.g. “3 21-17 19-21” or “ali 21-17”.
result-entry-preview = Reports match { $number }: { $result }.
# the heading of the matches while the entry of results has text; $text: the text
entry-found = Matches fitting “{ $text }”
entry-nothing-found = No match fits “{ $text }”.
report = Report

## What was done, as the page announces it

# $result: as `won` or `draw` say it
announce-reported = Match { $number }: { $result }.
announce-new-matches = { $count ->
        [one] One more match is scheduled.
       *[other] { $count } more matches are scheduled.
    }
announce-all-finished = Every match has a result.
# $winners: as `winners` says it, capitalized
announce-winners = { $winners }.
announce-reopened = Match { $number } is open again.
announce-unscheduled = { $count ->
        [one] The match scheduled because of its result is gone.
       *[other] The { $count } matches scheduled because of its result are gone.
    }
# $action: one of the action-… messages
announce-undone = Undone: { $action }.
announce-redone = Redone: { $action }.
announce-added-entrant = Added { $name }.
# entrants added at once, from a list pasted with a name on each line
announce-added-entrants = Added { $count } entrants.
announce-removed-entrant = Removed { $name }.
announce-renamed-entrant = Renamed { $name } to { $new_name }.
announce-added-phase = The phase is added.
announce-added-phases = { $count ->
        [one] The phase is added.
       *[other] { $count } phases are added.
    }
announce-edited-phase = The phase is changed.
announce-removed-phase = The phase is removed.
announce-unstarted = The start is undone: the competition is back at its setup.
announce-unstarted-phase = The start of { $phase } is undone.
announce-phase-finished = Every match of the phase has a result.
# $count: the matches without a result
announce-started-phase = Started { $phase }: { $count ->
        [one] one match
       *[other] { $count } matches
    } to play.
# $count: the matches with a result, at least one
announce-results-stay = { $count ->
        [one] The result reported stays.
       *[other] The { $count } results reported stay.
    }
# $count: the matches without a result
announce-started = Started: { $count ->
        [one] one match
       *[other] { $count } matches
    } to play.
action-add-entrant = adding { $name }
# $count: at least two
action-add-entrants = adding { $count } entrants
action-remove-entrant = removing { $name }
action-add-phase = adding { $phase }
# $count: at least two
action-add-phases = adding { $count } phases
action-edit-phase = editing { $phase }
action-remove-phase = removing { $phase }
action-start-phase = the start of { $phase }
action-start = the start
action-unstart = undoing the start
action-unstart-phase = undoing the start of { $phase }
# $result: as `won` or `draw` say it
action-report = the result of match { $number } ({ $result })
action-reopen = reopening match { $number }

## Names

# an entrant without a name, e.g. of a competition entered elsewhere
no-name = (no name)
# between the names of the members of a team
team-separator = {" & "}
# between the teams of a list, e.g. those tied for first
list-separator = {", "}
# $count: how many teams are ranked first; $winners: their names
winners = { $count ->
        [one] winner: { $winners }
       *[other] tied for first: { $winners }
    }

## Divisions of a phase

division-group = Group { $letter }
division-of-format = { $format } { $letter }
division = Group
all-divisions = All
divisions-shown = Groups shown
compared-heading = Place { $rank } compared across the groups
compared-hint = The same place of every group is ranked by the comparison across the groups, in the end by lot.
# buttons shown only while they have the keyboard's focus: past the bars at the top of the page,
# and past the list of entrants
skip-to-content = Skip to content
skip-to-phases = Skip to the phases
# in the ranking, the phase entrants wait for; $phase: its name
phase-not-started-yet = { $phase } (not started)
# a button below a table, copying it as text to paste into a spreadsheet or a message
copy = Copy
# $table: what the table is called, e.g. "Standings"
copy-table = Copy { $table } as text
announce-copied = Copied as text, to paste into a spreadsheet or a message.
announce-imported = Imported as a new competition, “{ $name }”.
