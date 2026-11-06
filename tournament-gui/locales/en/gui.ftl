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
help-running-entrants = Enter the entrants by name, one after the other, or paste a list with a name on each line.
help-running-phase = Choose how they play: the format, and the order of the standings. Then add the phase.
help-running-start = Start. The format schedules the matches, and the entrants are fixed from then on. The phase can still be changed, as long as the change fits the results reported.
help-running-results = Report the results: choose the side that won or Draw at a match, or enter its score below it, type them in, e.g. “3 1” for a win of the left side of match 3, or choose the winner in the bracket. The standings follow every result.
help-running-mistakes = A result reported by mistake is taken back by reopening its match. Undo takes back what was done since the competition was opened, in the order it was done.
help-running-byes = Where a round leaves an entrant without an opponent, the entrant has a bye: it moves on without a match and scores as the phase says, e.g. the points for a bye.
help-formats = Formats and standings
help-standings = The order of the standings
help-standings-text = The standings compare the entrants by the criteria of their order in turn: the first decides, the second only between entrants the first leaves tied, and so on. A criterion compares a metric, a number calculated for each entrant from the results, such as the points; the metrics a criterion needs are calculated with it. Further metrics can be calculated and shown without ranking by them.
help-criteria = Criteria
help-metrics = Metrics
help-stored = Your data stays on your device
help-stored-here = Tournamo stores all competitions locally in your browser (in its IndexedDB). Nothing you enter leaves your device: no data is sent to a server, there is no tracking, and no account is required.
help-stored-hosting = The page itself is delivered by GitHub Pages, which, like any web server, receives your IP address when you load it.
help-stored-lost = Your competitions are therefore only available in this browser on this device. They are deleted when you clear your browser’s site data or close a private window.
help-stored-export = To create a backup or continue on another device, export the competition as a file and import it there.
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

entrants = Entrants
no-entrants = No entrants yet. Each one is added by its name, or several at once by pasting a list with a name on each line.
name = Name
add-entrant = Add entrant
# the name typed looks like that of an entrant, e.g. differing in case only, so that it is refused
entrant-like = “{ $name }” is entered already.
remove = Remove
remove-entrant = Remove { $name }
rename-entrant = Rename { $name }
# the field of the name an entrant is renamed to
entrant-new-name = New name of { $name }
# not `phase`, which is the app's message of the fields of a phase
phase-heading = Phase
# the heading once every match is played
finished = Finished
# the medals of the first three ranks in the standings, before the rank
medal-gold = Gold medal,
medal-silver = Silver medal,
medal-bronze = Bronze medal,
add-phase = Add phase
# how long the phase takes with the entrants entered, before the start; $rounds and $matches: what
# the messages below say
phase-scale = With { $entrants } entrants, the phase takes { $rounds } and { $matches }.
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
start = Start

## Configuring the phase

# what a choice of a plugin shows while none is chosen
choose-plugin = Choose…
# the choice of no plugin, and what the phase shows for a field without one
no-plugin = (none)
# next to a plugin the plugins chosen elsewhere recommend
recommended = (recommended)
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
yes = yes
no = no
edit-phase = Edit phase
edit-phase-title = Edit the phase
edit-phase-description = The results reported so far stay. A change that does not fit them is refused.
save-changes = Save changes
cancel = Cancel

## After the start

matches = Matches
# what a narrow screen shows: the matches or the standings (or a knockout's bracket); a wide screen
# shows both, or the bracket
view = View
matches-and-standings = Matches & standings
# the lists of matches to choose from: those playable now, and each round
rounds = Rounds
round = Round { $round }
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
# the column of what the results of a row add up to: 1 for a win, ½ for a draw
cross-table-total = Total
# how a match ended for the team of the row, said by screen readers for 1, ½ and 0
outcome-win = won
outcome-draw = drawn
outcome-loss = lost
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
announce-edited-phase = The phase is changed.
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
action-remove-entrant = removing { $name }
action-add-phase = adding the phase
action-edit-phase = editing the phase
action-start = the start
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
