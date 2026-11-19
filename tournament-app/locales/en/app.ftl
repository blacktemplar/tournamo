# What tournament-app says in English: why an action was refused. The Display of each error says
# its message, without the marks around its variables.

error-empty-name = an entrant name must not be empty
error-duplicate-name = there already is an entrant named “{ $name }”
# $max: the most characters a name has
error-name-too-long = a name has at most { $max } characters
# $number: the number of a line pasted, $line: the line, $why: why it was not added
error-line-refused = line { $number }, “{ $line }”: { $why }
error-entrant-added-after-start = the competition has started, so no entrant can be added anymore
error-unknown-entrant = there is no such entrant
error-entrant-removed-after-start = the competition has started, so no entrant can be removed anymore
error-no-rating = “{ $rating }” is no rating: a rating is a number, such as 1500
error-rating-after-start = the competition has started, so the ratings can no longer change
error-unknown-format = there is no format “{ $format }”
# $reason: in English, from the format's code
error-format-misconfigured = { $reason }
error-unknown-preset = there is no preset “{ $preset }”
error-preset-misconfigured = { $reason }
error-preset-does-not-expand = the preset does not work: { $reason }
error-has-phases = the competition has phases already; a preset sets up a competition without any
error-invalid-phase = the phase cannot be played this way; the form shows what to change
error-no-sources = choose where the phase takes its entrants from
error-source-empty = a range of places ends before it starts
error-sources-overlap = two ranges of places of the phase overlap
# $phase: the phase that takes some of the same places already
error-source-taken = { $phase } takes some of these places already
error-registration-closed = the competition has started, so a phase cannot take entrants from the registration alone anymore
error-mixed-team-sizes = the places taken are of teams of different sizes, e.g. single entrants and pairs, which a phase takes only with “everyone on their own”
error-place-not-taken = enter a place: a whole number from 1
error-no-such-phase = there is no such phase
error-phase-started-already = the phase has started already
error-phase-starts-with-competition = the phase takes its entrants from the registration and starts with the competition
# $phases: each phase not finished with its progress, see phase-progress
error-phase-waiting = the phase can start once its entrants are known: { $phases }
error-phase-removed-after-start = the phase has started, so it cannot be removed anymore
# $phases: the phases taking entrants from the one to remove
error-phase-in-use = { $count ->
        [one] { $phases } takes entrants from this phase; remove that first
       *[other] { $phases } take entrants from this phase; remove those first
    }
# $phase: a later phase, started, taking entrants from the one changed
error-later-phase-started = { $phase } has started with the entrants of this phase; undo its start first
# a tie that a cut between places goes through; $favoured and $others: the names of the teams
# before and after the cut
warning-cut-through-tie = in { $phase }, more entrants share place { $rank } than go on: the lot takes { $favoured }, not { $others }
warning-cut-through-tie-in-group = in group { $group } of { $phase }, more entrants share place { $rank } than go on: the lot takes { $favoured }, not { $others }
# a phase without a name: its number and the label of its format
phase-unnamed = { $number } · { $format }
# a phase that is not finished: e.g. "Group phase: 12 of 18 matches played"
phase-progress = { $phase }: { $played } of { $matches } matches played
error-no-phase = the competition has no phase yet
# $count: how many matches have a result the changed phase does not fit; $matches: a list of
# them, of the phase itself and of later phases, see matches-of-edited-phase
error-result-does-not-fit = changed this way, the phase does not fit the { $count ->
        [one] result
       *[other] results
    } of { $matches }; reopen { $count ->
        [one] that match
       *[other] those matches
    } first
# $numbers: the numbers of matches, e.g. "5, 7, 9"
matches-of-edited-phase = { $count ->
        [one] its match { $numbers }
       *[other] its matches { $numbers }
    }
matches-of-later-phase = { $count ->
        [one] match { $numbers }
       *[other] matches { $numbers }
    } of { $phase }, which it sends entrants to
# $reason: in English, from the engine
error-phase-change-does-not-fit = the phase cannot be changed this way, since a later change no longer fits it: { $reason }
error-started-already = the competition has started already
error-too-few-entrants = a competition needs at least two entrants
error-too-few-for-format = the format needs at least { $count } entrants
error-too-few-for-divisions = { $divisions } groups need at least { $count } entrants: lower the number of groups
error-field-does-not-fit = { $phase } cannot form its teams ({ $teams }) of { $count } entrants, as the number does not come out even
# of a phase being configured
warning-many-rounds = { $count } rounds are more than a competition usually plays, and one this big is slow to save: is the phase meant this way?
warning-too-many-groups = { $divisions } groups need at least { $count } entrants; { $teams } entrants make at most { $most ->
        [one] one group
       *[other] { $most } groups
    }
warning-teams-unfit = its teams ({ $teams }) cannot be formed of { $count } entrants, as the number does not come out even
error-not-started = the competition has not started yet
error-unknown-match = there is no such match
error-match-finished-already = the match has a result already
error-draw-not-allowed = a match of this format cannot end in a draw
error-undecided = the result does not say who won
error-value-not-taken = a value entered is not taken; correct the one marked
error-result-unreadable = enter the number of a match and its result, 1, X or 2, e.g. “3 1”, or part of the winner's name
error-no-match-numbered = there is no match { $number }
error-match-numbered-finished-already = match { $number } has a result already
error-result-missing = enter the result of match { $number } as well: 1, X or 2
# $entered: part of the winner's name, as entered
error-no-name-found = no open match has an entrant whose name contains “{ $entered }”
error-name-elsewhere = “{ $name }” is only part of a name in match { $number }, while an entrant of that name plays in match { $elsewhere }: enter more of the name, or the other match's number
error-name-not-in-match = no open match { $number } has an entrant whose name contains “{ $name }”
# $matches: how many open matches have an entrant whose name contains $name; $number: one of them
error-name-in-several-matches = “{ $name }” fits { $matches } open matches; add the number of one, e.g. “{ $name } { $number }”
error-name-on-both-sides = “{ $name }” fits both sides of match { $number }; enter more of the name
# $parts: how many scores the match takes, e.g. one per game of a best of
error-scores-too-many = { $parts ->
        [0] match { $number } takes no scores: enter its result, 1, X or 2
        [one] match { $number } takes a single score
       *[other] match { $number } takes at most { $parts } scores, one per game
    }
error-scores-not-whole = match { $number } takes whole numbers as scores
# $count: how many matches it waits for; $waiting: their numbers, e.g. "1, 2"
error-match-not-playable-yet = { $count ->
        [0] match { $number } cannot be played yet
        [one] match { $number } cannot be played yet: it waits for match { $waiting }
       *[other] match { $number } cannot be played yet: it waits for matches { $waiting }
    }
# $value: a number entered, $max: the largest score
error-score-too-large = { $value } is too large for a score of match { $number }: at most { $max }
# $word: what was entered after the number of the match
error-not-a-score = “{ $word }” is no score of match { $number }: a score is two numbers, e.g. 21-17
error-not-a-result = “{ $word }” is no result of match { $number }: enter 1, X or 2, or its score, e.g. 21-17
error-scores-undecided = the scores entered do not say who won match { $number }; add the result: 1, X or 2
error-result-against-scores = the result entered for match { $number } is not the one its scores say
error-match-not-reported = the match has no result that could be undone
error-result-depended-on = a later result depends on this one; undo that one first
# $count: how many matches have a result depending on the one to reopen; $numbers: their numbers,
# e.g. "5, 7, 9"
error-result-depended-on-by = { $count ->
        [one] the result of match { $numbers } depends on this one; reopen that match first
       *[other] the results of matches { $numbers } depend on this one; reopen those matches first
    }
error-nothing-to-undo = there is nothing to undo
error-nothing-to-redo = there is nothing to redo
error-undo-changed-elsewhere = a change made elsewhere since keeps this from being undone, so nothing more can be undone
error-redo-changed-elsewhere = a change made elsewhere since keeps this from being redone, so nothing more can be redone
error-competition-not-found = there is no such competition
# the messages below have a detail, in English, saying why: from the storage, from reading JSON or
# from the engine
error-competition-unreadable = the competition cannot be read
error-competition-newer = the competition was saved by a newer version of Tournamo, which this one cannot open; reload the page for the newest
error-competition-damaged = the competition is stored damaged
error-log-does-not-replay = the log does not replay
error-file-no-competition = the file is no competition of Tournamo
error-file-newer = the file is of a newer version of Tournamo, which this one cannot open; reload the page for the newest
error-file-damaged = the file is a competition of Tournamo, but damaged
# $reason: in English, why the engine refused an action
error-rejected = { $reason }

# a round of the phase in a column of the standings, e.g. the one an entrant was knocked out in
cell-round = round { $round }

## The form of a phase

# the numbers a field takes; $whole: yes or no, $bounds: both, min, max or none, $multiple: 0 for
# any number
number-range = { $whole ->
        [yes] a whole number
       *[no] a number
    }{ $bounds ->
        [both] {" "}from { $min } to { $max }
        [min] {" "}of at least { $min }
        [max] {" "}of at most { $max }
       *[none] {""}
    }{ $multiple ->
        [0] {""}
       *[other] {" "}that is a multiple of { $multiple }
    }
# what an answer leads to that the operator may not want, although the phase can be played so
warning-unranked = ranked by nothing, every entrant is tied in the standings
warning-lot-decides-cut = the entrants on both sides of the cut after place { $place } of the registration have equal ratings or none, so the lot decides who is on which side
# where the places a source takes of a phase played in groups side by side lie in its groups,
# e.g. "with 3 groups side by side, places 1–8 are places 1–2 of each group and the best 2 of
# those in place 3 of their group, those of one place compared across the groups"; $parts: a list
# of the group-place messages below; $compared: "yes" where a part takes some of one place
places-in-groups = with { $groups } groups side by side, { $count ->
        [one] place { $from } is
       *[other] places { $from }–{ $to } are
    } { $parts }{ $compared ->
        [yes] , those of one place compared across the groups
       *[no] {""}
    }
places-on-in-groups = with { $groups } groups side by side, the places from { $from } on are { $parts }{ $compared ->
        [yes] , those of one place compared across the groups
       *[no] {""}
    }
group-place = place { $rank } of each group
group-places = places { $first }–{ $last } of each group
group-places-on = the places from { $rank } on of each group
group-places-best = { $count ->
        [one] the best
       *[other] the best { $count }
    } of those in place { $rank } of their group
group-places-worst = { $count ->
        [one] the worst
       *[other] the worst { $count }
    } of those in place { $rank } of their group
group-place-between = place { $place } of those in place { $rank } of their group
group-places-between = places { $first }–{ $last } of those in place { $rank } of their group
# the last two items of a list, e.g. "a, b and c" with $first "a, b"
list = { $first } and { $last }
# what looks like a typo in a result entered, though it is taken
# $winning: the score a winner usually has
# $name: an entrant of the match entered, $earlier: a match of theirs of an earlier round, open
warning-earlier-round = { $name } still has match { $earlier } of an earlier round to play
warning-below-winning-score = neither side has { $winning }, which a winner usually has
warning-winning-lead = at { $winning } or more, a winner usually leads by one or two
warning-result-disagrees = this is not the result the score suggests
warning-more-than-decided = more is entered than it took to decide the match
# $a, $b: the parts, e.g. games, each side had won when the match was decided, side A's first
warning-more-than-decided-at = the match was decided at { $a }–{ $b } already, yet more is entered
# what is wrong with the answer of a field, or with a plugin chosen
issue-preset-incomplete = the preset is not complete; its fields show what to change
issue-unanswered = this needs an answer
# the variables of number-range
issue-out-of-range = this takes { number-range }
# $entered: what the operator typed
issue-not-a-number = “{ $entered }” is no number
issue-duplicate = this is chosen twice
# $reason: in English, from the plugin
issue-rejected = this is not taken: { $reason }
# $reason: in English
issue-invalid = { $reason }
# $reason: in English
issue-incompatible = this does not fit next to the others: { $reason }
# $plugin: a plugin, $fact: in English, what it needs
issue-unprovided = { $plugin } needs { $fact }, which nothing provides
# $plugin: a plugin, $providers: the plugins of which it needs one
issue-unprovided-by = { $plugin } needs one of these: { $providers }
# $lacking: what each of the providers needs in turn, e.g. what a match format keeps
issue-unprovided-lacking = { $plugin } needs { $count ->
        [one] { $providers }, which needs
       *[other] one of these: { $providers }, which need
    } { $lacking }, and nothing chosen provides that
# $plugin: a plugin, $condition: what it needs, $provider: issue-provider, $provided: what that
# gives instead
issue-unmet = { $plugin } needs { $condition }, but { $provider } gives { $provided }
# as issue-unmet, where what it gives says nothing more
issue-unmet-unsaid = { $plugin } needs { $condition }, which { $provider } does not give
# as issue-unmet, where what it gets comes into the phase from its sources
issue-unmet-coming-in = { $plugin } needs { $condition }, but { $provided } come in
# as issue-unmet-coming-in, where what comes in says nothing more
issue-unmet-context = { $plugin } needs { $condition }, which does not come in
# $plugin: the plugin giving what another one gets, $field: the field it is chosen in
issue-provider = { $plugin } ({ $field })
# $issue: issue-unmet or one like it, $field: a field, $count: how many fit, $choices: the
# plugins that fit as the field's answer
issue-fitting = { $issue }; as { $field }, { $count ->
        [one] this fits
       *[other] these fit
    }: { $choices }
