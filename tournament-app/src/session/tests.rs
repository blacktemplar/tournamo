use std::{
    cell::Cell,
    cmp::Ordering,
    fmt,
    num::NonZeroU32,
    pin::pin,
    task::{Context, Poll, Waker},
};

use plugin_factory::{
    PluginType,
    config::{Path as ConfigPath, PluginConfig},
};
use tournament_engine::{
    EnteredValue, EntrantId, MatchId, MatchResult, PhasePlugins, Score, Sides,
    plugins::{
        match_format::{BestOf, Scored},
        metrics_calculator::{Cell as MetricCell, MoreWins, RoundOfFirstLossCalculator},
        phase_format::{
            DoubleElimination, MonsterDyp, RandomRounds, RoundRobin, SingleElimination,
        },
    },
    uuid::Uuid,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::{
        AddEntrantError, AddPhaseError, EditPhaseError, ImportError, LoadError, NoPhase,
        PhaseDraftError, RedoError, RemoveEntrantError, RenameEntrantError, ReportEnteredError,
        ReportError, StartError, UndoError, UndoReportError,
    },
    file::{self},
    storage::{CompetitionKey, Listing, MemoryStorage, Progress, Save, Storage, Stored},
    view::{
        Action, BracketColumn, BracketEntry, BracketLine, BracketPart, BracketSection,
        BracketSource, Column, Count, EntrantRow, MatchRow, Medal, NameSearch, Outcome, Plugin,
        RoundSummary, RoundsView, Scale, Shown, StageName, StandingCell, Unplayed,
    },
};

/// The output of `future`, which a [`MemoryStorage`] finishes right away.
#[track_caller]
fn now<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the storage of a test never waits"),
    }
}

/// Builds the session a test starts from: Alice, Bob, Carol and Dave entered in a round robin
/// with the defaults of its format, unless the test says otherwise.
struct Given {
    entrants: Vec<&'static str>,
    format: &'static str,
    /// The format's suggestion if `None`.
    match_format: Option<PluginConfig>,
}

impl Given {
    fn new() -> Self {
        Self {
            entrants: vec!["Alice", "Bob", "Carol", "Dave"],
            format: RoundRobin::ID,
            match_format: None,
        }
    }

    fn entrants(self, entrants: &[&'static str]) -> Self {
        Self {
            entrants: entrants.to_vec(),
            ..self
        }
    }

    fn format(self, format: &'static str) -> Self {
        Self { format, ..self }
    }

    fn match_format(self, match_format: PluginConfig) -> Self {
        Self {
            match_format: Some(match_format),
            ..self
        }
    }

    /// The session with the entrants and the phase added, not started.
    fn added(self) -> Session {
        let mut session = Session::new("cup");
        for name in self.entrants {
            session.add_entrant(name).unwrap();
        }
        let mut draft = session.phase_draft(self.format).unwrap();
        if let Some(match_format) = self.match_format {
            draft = draft
                .with(PhasePlugins::MATCH_FORMAT, match_format)
                .and_then(|draft| draft.with_defaults())
                .unwrap();
        }
        session.add_phase(&draft).unwrap();
        session
    }

    fn started(self) -> Session {
        let mut session = self.added();
        session.start().unwrap();
        session
    }
}

fn names(entrants: &[EntrantRow]) -> Vec<&str> {
    entrants
        .iter()
        .map(|entrant| entrant.name.as_deref().unwrap())
        .collect()
}

/// Every match of the phase, round by round.
fn matches(session: &Session) -> Vec<MatchRow> {
    session
        .rounds()
        .iter()
        .flat_map(|summary| session.round(summary.round).unwrap().matches)
        .collect()
}

/// The match listed at `number`.
fn numbered(session: &Session, number: usize) -> MatchId {
    matches(session)
        .into_iter()
        .find(|row| row.number == number)
        .unwrap()
        .id
}

fn round(number: u32) -> NonZeroU32 {
    NonZeroU32::new(number).unwrap()
}

/// What `stage` is named in English.
fn english(stage: Option<StageName>) -> Option<String> {
    stage.map(|stage| stage.stage.to_string())
}

#[test]
fn entrants_are_listed_by_name_in_the_order_they_were_added() {
    let mut session = Session::new("cup");

    session.add_entrant("Bob").unwrap();
    session.add_entrant("  Alice ").unwrap();

    assert_eq!(names(&session.entrants()), ["Bob", "Alice"]);
}

#[test]
fn an_empty_or_taken_name_adds_no_entrant() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    assert!(matches!(
        session.add_entrant(" "),
        Err(AddEntrantError::EmptyName)
    ));
    assert!(matches!(
        session.add_entrant("Alice "),
        Err(AddEntrantError::DuplicateName(name)) if name == "Alice"
    ));
    assert_eq!(names(&session.entrants()), ["Alice"]);
}

#[test]
fn a_batch_is_checked_before_any_entrant_is_added() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    let before = session.export();
    let undo = session.next_undo();

    for names in [
        ["Bob", "Alice"],
        ["Bob", "ALICE"],
        ["Bob", "Ａｌｉｃｅ"],
        ["Bob", "Bob"],
        ["Bob", "bob"],
        ["Bob", " "],
    ] {
        let names = names.map(str::to_owned);
        assert!(session.add_named_entrants(&names).is_err());
        assert_eq!(session.export(), before);
        assert_eq!(session.next_undo(), undo);
    }
}

#[test]
fn batch_names_are_kept_as_supplied_and_undone_one_at_a_time() {
    let mut session = Session::new("cup");
    session
        .add_named_entrants(&[" Alice ".to_owned(), "Bob".to_owned()])
        .unwrap();
    assert_eq!(names(&session.entrants()), [" Alice ", "Bob"]);

    session.undo().unwrap();
    assert_eq!(names(&session.entrants()), [" Alice "]);
    session.undo().unwrap();
    assert!(session.entrants().is_empty());
    session.redo().unwrap();
    session.redo().unwrap();
    assert_eq!(names(&session.entrants()), [" Alice ", "Bob"]);
}

#[test]
fn a_batch_is_not_added_after_the_start() {
    let mut session = Given::new().started();
    let before = session.export();

    assert!(matches!(
        session.add_named_entrants(&["Eve".to_owned(), "Frank".to_owned()]),
        Err(AddEntrantError::Started)
    ));
    assert_eq!(session.export(), before);
}

/// Names differing in case or in how Unicode composes their characters look alike, which the
/// operator is told of while typing, and are refused, by the name entered.
#[test]
fn a_name_like_one_entered_is_found_and_refused() {
    let mut session = Session::new("cup");
    // "Zoë" with its "ë" as one character
    session.add_entrant("Zo\u{eb}").unwrap();

    assert_eq!(session.entrant_like(" ZOË "), Some("Zoë".to_owned()));
    // with its "ë" as an "e" and a combining diaeresis
    assert_eq!(session.entrant_like("zoe\u{308}"), Some("Zoë".to_owned()));
    assert_eq!(session.entrant_like("Zoe"), None);
    assert_eq!(session.entrant_like(""), None);
    assert!(matches!(
        session.add_entrant("zoë"),
        Err(AddEntrantError::DuplicateName(name)) if name == "Zoë"
    ));
    assert_eq!(names(&session.entrants()), ["Zoë"]);
}

#[test]
fn a_removed_entrant_comes_back_with_its_name_by_undo() {
    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    session.add_entrant("Bob").unwrap();

    session.remove_entrant(alice).unwrap();
    assert_eq!(names(&session.entrants()), ["Bob"]);
    assert!(matches!(
        session.remove_entrant(alice),
        Err(RemoveEntrantError::UnknownEntrant)
    ));
    session.undo().unwrap();

    assert_eq!(names(&session.entrants()), ["Alice", "Bob"]);
}

/// A list adds an entrant for each line not blank, each an action of its own.
#[test]
fn a_list_adds_an_entrant_for_each_line() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    let added = session.add_entrants("Bob\r\n  Carol \n\n Dave\n").unwrap();

    assert_eq!(added.len(), 3);
    assert_eq!(
        names(&session.entrants()),
        ["Alice", "Bob", "Carol", "Dave"]
    );
    session.undo().unwrap();
    assert_eq!(names(&session.entrants()), ["Alice", "Bob", "Carol"]);
}

/// A list with a name taken, by an entrant or an earlier line, adds nobody.
#[test]
fn a_list_with_a_name_taken_adds_nobody() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    assert!(matches!(
        session.add_entrants("Bob\nALICE"),
        Err(AddEntrantError::DuplicateName(name)) if name == "Alice"
    ));
    assert!(matches!(
        session.add_entrants("Bob\nCarol\nbob"),
        Err(AddEntrantError::DuplicateName(name)) if name == "Bob"
    ));
    assert!(matches!(
        session.add_entrants(" \n"),
        Err(AddEntrantError::EmptyName)
    ));
    assert_eq!(names(&session.entrants()), ["Alice"]);
}

/// Renaming keeps the entrant's place, also after the start, and is saved; it appends nothing to
/// the log, so undo undoes the action before it.
#[test]
fn an_entrant_is_renamed_in_place() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    now(session.save(&storage)).unwrap();
    let bob = session.entrants()[1].id;
    let log = session.machine.log().len();

    session.rename_entrant(bob, " Robert ").unwrap();

    assert_eq!(
        names(&session.entrants()),
        ["Alice", "Robert", "Carol", "Dave"]
    );
    assert_eq!(session.machine.log().len(), log);
    assert!(session.has_unsaved_changes());
    now(session.save(&storage)).unwrap();
    let loaded = now(Session::load(&storage, session.key())).unwrap();
    assert_eq!(loaded.entrants(), session.entrants());
    assert_eq!(session.undo().unwrap(), Action::Start);
}

#[test]
fn a_rename_to_an_empty_or_taken_name_is_refused() {
    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    let bob = session.add_entrant("Bob").unwrap();
    session.remove_entrant(bob).unwrap();

    assert!(matches!(
        session.rename_entrant(alice, " "),
        Err(RenameEntrantError::EmptyName)
    ));
    assert!(matches!(
        session.rename_entrant(bob, "Robert"),
        Err(RenameEntrantError::UnknownEntrant)
    ));
    let carol = session.add_entrant("Carol").unwrap();
    assert!(matches!(
        session.rename_entrant(carol, "alice"),
        Err(RenameEntrantError::DuplicateName(name)) if name == "Alice"
    ));
    // differing from its own name in case only
    session.rename_entrant(alice, "ALICE").unwrap();
    assert_eq!(names(&session.entrants()), ["ALICE", "Carol"]);
}

#[test]
fn no_entrant_is_added_or_removed_after_the_start() {
    let mut session = Given::new().started();
    let alice = session.entrants()[0].id;

    assert!(matches!(
        session.add_entrant("Eve"),
        Err(AddEntrantError::Started)
    ));
    assert!(matches!(
        session.remove_entrant(alice),
        Err(RemoveEntrantError::Started)
    ));
}

/// What a GUI offers to pick from is every registered format, and each of them is played with
/// what it offers without anything else answered.
#[test]
fn every_format_adds_a_phase_with_the_answers_it_offers() {
    let formats = Session::formats();
    assert!(formats.iter().any(|format| format.id == RoundRobin::ID));

    for format in formats {
        let started = Given::new().format(format.id).started();

        assert!(!started.rounds().is_empty(), "{format:?}");
    }
}

/// A help explains every plugin, each by its label and description.
#[test]
fn the_glossary_has_every_format_criterion_and_metric() {
    let glossary = Session::glossary();

    let ids =
        |plugins: &[Plugin]| -> Vec<&str> { plugins.iter().map(|plugin| plugin.id).collect() };
    assert!(ids(&glossary.formats).contains(&RoundRobin::ID));
    assert!(ids(&glossary.criteria).contains(&<MoreWins as PluginType>::ID));
    assert!(ids(&glossary.metrics).contains(&RoundOfFirstLossCalculator::ID));
    let all = [glossary.formats, glossary.criteria, glossary.metrics].concat();
    assert!(all.iter().all(|plugin| plugin.description.is_some()));
}

/// How long a phase takes shows before the start: a round robin of 24 takes 23 rounds and 276
/// matches, a double elimination of 8 one match more if the grand final is played again, and a
/// phase that does not start with the entrants entered nothing.
#[test]
fn the_scale_of_a_phase_is_known_before_the_start() {
    let names: Vec<&'static str> = (1..=24)
        .map(|number| &*format!("Entrant {number}").leak())
        .collect();
    let scale =
        |session: &Session, format: &str| session.scale(&session.phase_draft(format).unwrap());
    let exactly = |count| Count {
        least: count,
        most: count,
    };

    assert_eq!(
        scale(&Session::new("cup"), RoundRobin::ID),
        None,
        "nobody entered"
    );
    let mut session = Session::new("cup");
    for name in &names {
        session.add_entrant(name).unwrap();
    }
    assert_eq!(
        scale(&session, RoundRobin::ID),
        Some(Scale {
            rounds: exactly(23),
            matches: exactly(276)
        })
    );
    let eight = Given::new()
        .entrants(&names[..8])
        .format(DoubleElimination::ID)
        .added();
    assert_eq!(
        scale(&eight, DoubleElimination::ID).map(|scale| scale.matches),
        Some(Count {
            least: 14,
            most: 15
        })
    );
}

#[test]
fn an_unknown_format_has_no_draft() {
    let session = Session::new("cup");

    assert!(matches!(
        session.phase_draft("swiss"),
        Err(PhaseDraftError::UnknownFormat(format)) if format == "swiss"
    ));
}

/// The points can be scored in many ways, so a phase ranked by them without a points calculator
/// is not added with one the operator was not asked about.
#[test]
fn a_phase_whose_plugins_do_not_fit_is_not_added() {
    let mut session = Session::new("cup");
    let mut draft = session.phase_draft(RoundRobin::ID).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("metrics"),
            Vec::<PluginConfig>::new(),
        )
        .unwrap();

    let result = session.add_phase(&draft);

    assert!(
        matches!(result, Err(AddPhaseError::Invalid(_))),
        "{result:?}"
    );
    assert!(session.competition().phases().is_empty());
}

#[test]
fn only_one_phase_is_added() {
    let mut session = Given::new().added();
    let draft = session.phase_draft(SingleElimination::ID).unwrap();

    assert!(matches!(
        session.add_phase(&draft),
        Err(AddPhaseError::PhaseAlreadyAdded)
    ));
}

/// The start says why it would be refused before it is tried, as it is refused.
#[test]
fn the_start_needs_a_phase_and_two_entrants() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    session.add_entrant("Bob").unwrap();
    assert!(matches!(
        session.start_refusal(),
        Some(StartError::NoPhase(_))
    ));
    assert!(matches!(session.start(), Err(StartError::NoPhase(_))));

    let mut session = Given::new().entrants(&["Alice"]).added();
    assert!(matches!(
        session.start_refusal(),
        Some(StartError::TooFewEntrants)
    ));
    assert!(matches!(session.start(), Err(StartError::TooFewEntrants)));

    let mut session = Given::new().added();
    assert!(session.start_refusal().is_none());
    session.start().unwrap();

    assert!(matches!(session.start_refusal(), Some(StartError::Started)));
    assert!(matches!(session.start(), Err(StartError::Started)));
}

/// A Monster DYP pairs teams of two, so it needs four entrants; with fewer it would schedule
/// nothing and be finished at once.
#[test]
fn the_start_needs_as_many_entrants_as_the_format_does() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol"])
        .format(MonsterDyp::ID)
        .added();

    assert!(matches!(
        session.start_refusal(),
        Some(StartError::TooFewForFormat(4))
    ));
    assert!(matches!(
        session.start(),
        Err(StartError::TooFewForFormat(4))
    ));
    session.add_entrant("Dave").unwrap();
    session.start().unwrap();
}

/// A file without a name, or with a blank one, is named after the file, or else by a name of its
/// own, never by nothing, which the list could not show.
#[test]
fn an_imported_competition_is_never_nameless() {
    let file = Given::new().added().export();
    let blank = file.replacen("\"name\": \"cup\"", "\"name\": \"  \"", 1);
    assert_ne!(blank, file, "the file names its competition");

    assert_eq!(
        Session::import(&blank, "league", &[]).unwrap().name(),
        "league"
    );
    assert_eq!(
        Session::import(&blank, " ", &[]).unwrap().name(),
        "Competition"
    );
}

#[test]
fn what_can_be_done_follows_the_competition() {
    let mut session = Session::new("cup");
    let actions = session.actions();
    assert!(actions.add_entrant && actions.add_phase);
    assert!(!actions.remove_entrant && !actions.edit_phase && !actions.start);
    assert!(!actions.report && !actions.undo && !actions.redo);

    session.add_entrant("Alice").unwrap();
    assert!(session.actions().remove_entrant && session.actions().undo);
    let draft = session.phase_draft(RoundRobin::ID).unwrap();
    session.add_phase(&draft).unwrap();
    assert!(!session.actions().add_phase && session.actions().edit_phase);
    assert!(!session.actions().start, "one entrant is too few");
    session.add_entrant("Bob").unwrap();
    assert!(session.actions().start);

    session.start().unwrap();
    let actions = session.actions();
    assert!(actions.report && actions.edit_phase);
    assert!(!actions.add_entrant && !actions.remove_entrant && !actions.start);

    session
        .report(numbered(&session, 1), MatchResult::Draw)
        .unwrap();
    assert!(!session.actions().report, "the only match has its result");
    session.undo().unwrap();
    assert!(session.actions().report && session.actions().redo);
}

#[test]
fn the_matches_are_listed_by_round_with_the_names_of_their_entrants() {
    let session = Given::new().started();

    let rounds = session.rounds();

    assert_eq!(
        rounds,
        (1..=3)
            .map(|number| RoundSummary {
                round: round(number),
                matches: 2,
                open: 2,
                stage: None,
            })
            .collect::<Vec<_>>(),
        "a round robin of four plays three rounds of two matches, none of a stage"
    );
    let first = session.round(round(1)).unwrap();
    assert_eq!(
        first
            .matches
            .iter()
            .map(|row| (row.number, row.round))
            .collect::<Vec<_>>(),
        [(1, round(1)), (2, round(1))]
    );
    let mut entrants: Vec<_> = first
        .matches
        .iter()
        .flat_map(|row| names(&row.a).into_iter().chain(names(&row.b)))
        .collect();
    entrants.sort_unstable();
    assert_eq!(entrants, ["Alice", "Bob", "Carol", "Dave"]);
    assert!(first.matches.iter().all(|row| row.stage.is_none()));
    assert!(first.byes.is_empty());
    assert!(first.draws, "a round robin takes draws");
    assert_eq!(session.round(round(4)), None);
}

#[test]
fn nothing_is_played_before_the_start() {
    let session = Given::new().added();

    assert!(session.rounds().is_empty());
    assert_eq!(session.round(round(1)), None);
    assert_eq!(session.standings(), None);
}

/// A round robin of three has one entrant sit out every round.
#[test]
fn a_round_lists_its_byes() {
    let session = Given::new().entrants(&["Alice", "Bob", "Carol"]).started();

    let second = session.round(round(2)).unwrap();

    assert_eq!(second.matches.len(), 1);
    assert_eq!(second.byes.len(), 1);
    assert_eq!(second.byes[0].entrants.len(), 1);
    assert_eq!(second.byes[0].stage, None, "a round robin names no stages");
}

/// Of six entrants in a double elimination, the best two sit out the first round of the winner
/// bracket, so the two that lose in it sit out the first round of the loser bracket, which is
/// played in the round of the second round of the winner bracket.
#[test]
fn a_bye_is_named_by_its_stage_where_its_round_has_several() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .format(DoubleElimination::ID)
        .started();
    let bye_stages = |session: &Session, number| -> Vec<Option<String>> {
        let round = session.round(round(number)).unwrap();
        round
            .byes
            .into_iter()
            .map(|bye| english(bye.stage))
            .collect()
    };
    assert_eq!(
        bye_stages(&session, 1),
        [None, None],
        "every match and bye of the first round is of the winner bracket"
    );

    for number in 1..=2 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let losers = Some("Losers round 1".to_owned());
    assert_eq!(bye_stages(&session, 2), [losers.clone(), losers]);
}

#[test]
fn a_knockout_names_the_stage_of_every_match() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    let stages = |session: &Session| -> Vec<Option<String>> {
        matches(session)
            .into_iter()
            .map(|row| english(row.stage))
            .collect()
    };
    assert_eq!(
        stages(&session),
        [Some("Semifinals".to_owned()), Some("Semifinals".to_owned())]
    );
    assert_eq!(
        matches(&session)[0].stage.as_ref().unwrap().format,
        SingleElimination::ID,
        "a stage is translated by its format"
    );

    let first = session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    let second = session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    assert_eq!((first.new_matches, first.winners), (0, None));
    assert_eq!(
        (second.new_matches, second.winners),
        (2, None),
        "the final and the match for third place"
    );
    assert_eq!(
        stages(&session)[2..],
        [Some("Final".to_owned()), Some("Third place".to_owned())]
    );
    session
        .report(numbered(&session, 3), MatchResult::AWins)
        .unwrap();
    let last = session
        .report(numbered(&session, 4), MatchResult::BWins)
        .unwrap();
    assert_eq!(last.new_matches, 0);
    assert_eq!(last.winners, session.winners());
    assert!(last.winners.is_some_and(|winners| winners.len() == 1));
}

/// A round whose matches are all of one stage is named by it, one of several stages is not.
#[test]
fn a_round_of_one_stage_is_named_by_it() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=2 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let stages: Vec<_> = session
        .rounds()
        .into_iter()
        .map(|summary| english(summary.stage))
        .collect();

    assert_eq!(stages, [Some("Semifinals".to_owned()), None]);
}

/// A round a cell of the standings names, e.g. that of the first loss, is named by the stage of
/// the entrant's match in it, also in a round of several stages.
#[test]
fn the_standings_name_the_round_of_a_cell_by_its_stage() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=4 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let standings = session.standings().unwrap();

    let column = standings
        .columns
        .iter()
        .position(|column| column.calculator == RoundOfFirstLossCalculator::ID)
        .unwrap();
    let lost_in: Vec<_> = standings
        .rows
        .iter()
        .map(|row| match &row.cells[column] {
            Some(StandingCell::Stage(stage)) => stage.stage.to_string(),
            Some(StandingCell::Metric(MetricCell::Text(phrase))) => phrase.to_string(),
            cell => panic!("{cell:?} is no round"),
        })
        .collect();
    assert_eq!(lost_in, ["-", "Final", "Semifinals", "Semifinals"]);
}

/// Who `entry` is, as a test reads it: the names of a team, `bye`, or where a side not decided
/// yet comes from, by the number of the match or else the stage.
fn entry(entry: &BracketEntry) -> String {
    let source = |source: &BracketSource| match (source.number, source.position) {
        (Some(number), _) => format!("#{number}"),
        (None, Some(position)) => {
            format!("{} ({position})", english(source.stage.clone()).unwrap())
        }
        (None, None) => english(source.stage.clone()).unwrap(),
    };
    match entry {
        BracketEntry::Team(team) => names(team).join(" & "),
        BracketEntry::Bye => "bye".to_owned(),
        BracketEntry::WinnerOf(from) => format!("winner of {}", source(from)),
        BracketEntry::LoserOf(from) => format!("loser of {}", source(from)),
    }
}

/// `column` as its stage in English and its pairings, each as its two sides.
fn column(column: &BracketColumn) -> (String, Vec<[String; 2]>) {
    (
        english(column.stage.clone()).unwrap(),
        column
            .pairings
            .iter()
            .map(|pairing| pairing.sides.each_ref().map(entry))
            .collect(),
    )
}

/// Every column of `section`, see [`column`].
fn columns(section: &BracketSection) -> Vec<(String, Vec<[String; 2]>)> {
    section.columns.iter().map(column).collect()
}

#[test]
fn only_a_started_knockout_has_a_bracket() {
    assert_eq!(Given::new().started().bracket(), None, "a round robin");
    assert_eq!(
        Given::new().format(SingleElimination::ID).added().bracket(),
        None
    );
    assert!(
        Given::new()
            .format(SingleElimination::ID)
            .started()
            .bracket()
            .is_some()
    );
}

/// The final and the match for third place, below it, are in the bracket before they are
/// scheduled, waiting for the semifinals, and take the entrants as those are decided.
#[test]
fn the_bracket_of_a_knockout_shows_who_meets_whom() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    let first = matches(&session)[0].clone();

    let bracket = session.bracket().unwrap();

    let parts = bracket.sections.iter().map(|section| section.part);
    assert_eq!(parts.collect::<Vec<_>>(), [BracketPart::Knockout]);
    let [semifinals, the_final] = &columns(&bracket.sections[0])[..] else {
        panic!("expected the semifinals and the final");
    };
    assert_eq!(semifinals.0, "Semifinals");
    assert_eq!(
        semifinals.1[0],
        [names(&first.a).join(""), names(&first.b).join("")]
    );
    assert_eq!(
        *the_final,
        (
            "Final".to_owned(),
            vec![["winner of #1".to_owned(), "winner of #2".to_owned()]]
        )
    );
    let knockout = &bracket.sections[0].columns;
    assert_eq!(knockout[0].below, None);
    assert_eq!(
        knockout[1].below.as_deref().map(column),
        Some((
            "Third place".to_owned(),
            vec![["loser of #2".to_owned(), "loser of #1".to_owned()]]
        ))
    );
    let lines = |column: &BracketColumn| -> Vec<_> {
        column
            .pairings
            .iter()
            .map(|pairing| (pairing.line_in, pairing.line_out))
            .collect()
    };
    assert_eq!(
        lines(&knockout[0]),
        [
            (false, Some(BracketLine::Down)),
            (false, Some(BracketLine::Up))
        ]
    );
    assert_eq!(lines(&knockout[1]), [(true, None)]);

    session.report(first.id, MatchResult::AWins).unwrap();

    let bracket = session.bracket().unwrap();
    let the_final = &bracket.sections[0].columns[1].pairings[0];
    assert_eq!(entry(&the_final.sides[0]), names(&first.a).join(""));
    assert_eq!(
        the_final.r#match, None,
        "it still waits for the other semifinal"
    );
    let first_row = bracket.sections[0].columns[0].pairings[0].r#match.as_ref();
    assert_eq!(
        first_row.map(|row| row.result),
        Some(Some(MatchResult::AWins))
    );
}

/// A knockout's matches have the numbers of their places in the bracket, whichever is scheduled
/// first, and a result is entered by them.
#[test]
fn a_knockout_numbers_its_matches_by_the_bracket() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .format(SingleElimination::ID)
        .started();

    session.report_entered("2 1").unwrap();

    let second_round = session.round(round(2)).unwrap();
    let scheduled: Vec<_> = second_round.matches.iter().map(|row| row.number).collect();
    assert_eq!(
        scheduled,
        [4],
        "the second semifinal, though scheduled first"
    );
    session.report_entered("4 1").unwrap();
    assert_eq!(
        session.round(round(2)).unwrap().matches[0].result,
        Some(MatchResult::AWins)
    );
}

/// Of six entrants, the best two have byes in the quarterfinals, so neither semifinal is scheduled
/// at the start; the final names them by the numbers their matches will have, which they show.
#[test]
fn a_side_waiting_for_a_pairing_not_scheduled_yet_names_the_number_of_its_match() {
    let session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .format(SingleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let semifinals = &bracket.sections[0].columns[1].pairings;
    assert_eq!(
        semifinals
            .iter()
            .map(|pairing| (pairing.number, pairing.r#match.is_some()))
            .collect::<Vec<_>>(),
        [(Some(3), false), (Some(4), false)]
    );

    let knockout = columns(&bracket.sections[0]);
    assert_eq!(
        knockout[1].1[0][1], "winner of #1",
        "a quarterfinal is scheduled"
    );
    assert_eq!(
        knockout[2],
        (
            "Final".to_owned(),
            vec![["winner of #3".to_owned(), "winner of #4".to_owned()]]
        )
    );
}

/// The losers of the winner bracket drop into the loser bracket, and the reset of the grand final
/// is only played if the champion of the loser bracket wins it.
#[test]
fn the_bracket_of_a_double_elimination_shows_where_the_losers_drop() {
    let session = Given::new().format(DoubleElimination::ID).started();

    let bracket = session.bracket().unwrap();

    let parts = bracket.sections.iter().map(|section| section.part);
    assert_eq!(
        parts.collect::<Vec<_>>(),
        [
            BracketPart::Winners,
            BracketPart::Losers,
            BracketPart::GrandFinal
        ]
    );
    let stages = |section: &BracketSection| -> Vec<String> {
        columns(section)
            .into_iter()
            .map(|(stage, _)| stage)
            .collect()
    };
    assert_eq!(
        stages(&bracket.sections[1]),
        ["Losers round 1", "Losers final"]
    );
    assert_eq!(
        columns(&bracket.sections[1])[1].1,
        [["winner of #4".to_owned(), "loser of #3".to_owned()]]
    );
    let losers = &bracket.sections[1].columns;
    assert_eq!(
        losers[0].pairings[0].line_out,
        Some(BracketLine::Straight),
        "the survivor meets one dropping down"
    );
    let grand_final = &bracket.sections[2].columns;
    assert_eq!(
        stages(&bracket.sections[2]),
        ["Grand final", "Grand final reset"]
    );
    assert!(!grand_final[0].pairings[0].only_if_needed);
    assert!(grand_final[1].pairings[0].only_if_needed);
}

/// Of five entrants, three have byes in the first round, whose pairings are left out: the first
/// column is drawn in the rows of the second, its one match level with the pairing it leads to,
/// which names who moved on by a bye.
#[test]
fn a_pairing_decided_by_a_bye_is_left_out_of_its_column() {
    let session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin"])
        .format(DoubleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let winners = &bracket.sections[0].columns;
    assert_eq!(winners[0].rows, 2, "the rows of the second round");
    let [first] = &winners[0].pairings[..] else {
        panic!("expected the one match of the first round");
    };
    assert_eq!(
        (first.number, first.row, first.line_out),
        (Some(1), 0, Some(BracketLine::Straight))
    );
    let second: Vec<_> = winners[1]
        .pairings
        .iter()
        .map(|pairing| (pairing.after_bye, pairing.line_in))
        .collect();
    assert_eq!(second, [([true, false], true), ([true, true], false)]);
    assert_eq!(
        winners[1].rows,
        winners[1].pairings.len(),
        "not folded, as a line leads into it"
    );
}

/// Of twenty entrants, twelve have byes in the round of 32, which is drawn in the eight rows of
/// the round of 16, half as high as with a row for each of its pairings.
#[test]
fn a_knockout_with_many_byes_is_half_as_high() {
    let names: Vec<&'static str> = (1..=20)
        .map(|number| &*format!("Entrant {number}").leak())
        .collect();
    let session = Given::new()
        .entrants(&names)
        .format(SingleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let first = &bracket.sections[0].columns[0];
    assert_eq!((first.rows, first.pairings.len()), (8, 4));
    assert!(
        first
            .pairings
            .iter()
            .all(|pairing| pairing.line_out == Some(BracketLine::Straight))
    );
}

/// Of five entrants, two byes meet in the first round of the loser bracket, which is left out
/// whole, and a side waiting for the winner of a pairing against a bye waits for whoever plays the
/// bye.
#[test]
fn a_bye_is_passed_through_to_whoever_plays_it() {
    let session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin"])
        .format(DoubleElimination::ID)
        .started();

    let bracket = session.bracket().unwrap();

    let losers = &bracket.sections[1].columns;
    let stages = losers
        .iter()
        .map(|column| english(column.stage.clone()).unwrap());
    assert_eq!(
        stages.collect::<Vec<_>>(),
        ["Losers round 2", "Losers round 3", "Losers final"]
    );
    assert_eq!(
        column(&losers[0]).1,
        [["loser of #1".to_owned(), "loser of #3".to_owned()]],
        "the losers of the winner bracket, rather than the winners of the byes they get first"
    );
    assert_eq!(losers[0].pairings[0].after_bye, [true, false]);
    assert_eq!(
        column(&losers[1]).1,
        [["winner of #4".to_owned(), "loser of #2".to_owned()]],
        "whoever gets the bye of the pairing of a bye and the loser of #2"
    );
    assert_eq!(losers[1].pairings[0].after_bye, [false, true]);
}

/// Reports side A as the winner of every match playable, as long as there is one, except
/// `exception`, a stage in English whose match side B wins.
fn play_out(session: &mut Session, exception: &str) {
    while let Some(row) = session.playable().into_iter().next() {
        let result = if english(row.stage.clone()).as_deref() == Some(exception) {
            MatchResult::BWins
        } else {
            MatchResult::AWins
        };
        session.report(row.id, result).unwrap();
    }
}

/// The reset of the grand final is "if needed" until it is scheduled, or until the champion of the
/// winner bracket won the grand final, which leaves it unplayed.
#[test]
fn the_reset_of_the_grand_final_is_only_if_needed_until_that_is_decided() {
    let reset = |session: &Session| {
        let bracket = session.bracket().unwrap();
        bracket.sections[2].columns[1].pairings[0].clone()
    };
    let mut session = Given::new().format(DoubleElimination::ID).started();
    assert!(reset(&session).only_if_needed);

    play_out(&mut session, "none");

    let not_needed = reset(&session);
    assert!(!not_needed.only_if_needed);
    assert_eq!(not_needed.unplayed, Some(Unplayed::NotNeeded));
    let grand_final = &session.bracket().unwrap().sections[2].columns[0].pairings[0];
    assert_eq!(grand_final.line_out, None);

    let mut session = Given::new().format(DoubleElimination::ID).started();
    play_out(&mut session, "Grand final");
    let played = reset(&session);
    assert!(!played.only_if_needed && played.unplayed.is_none());
    assert!(played.r#match.is_some());
}

#[test]
fn a_reported_result_comes_with_its_match() {
    let mut session = Given::new().started();
    let first = matches(&session).remove(0);

    let reported = session.report(first.id, MatchResult::BWins).unwrap();

    assert_eq!(
        reported.row,
        MatchRow {
            result: Some(MatchResult::BWins),
            ..first
        }
    );
}

#[test]
fn a_result_the_match_does_not_take_is_refused() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    let first = numbered(&session, 1);
    let elsewhere = numbered(&Given::new().started(), 1);

    assert!(!session.round(round(1)).unwrap().draws);
    assert!(matches!(
        session.report(first, MatchResult::Draw),
        Err(ReportError::DrawNotAllowed)
    ));
    session.report(first, MatchResult::AWins).unwrap();
    assert!(matches!(
        session.report(first, MatchResult::BWins),
        Err(ReportError::AlreadyFinished)
    ));
    assert!(matches!(
        session.report(elsewhere, MatchResult::AWins),
        Err(ReportError::UnknownMatch)
    ));
    assert!(matches!(
        Given::new().added().report(first, MatchResult::AWins),
        Err(ReportError::NotStarted)
    ));
}

/// The result of a match reported long ago is undone by its id, which leaves every result reported
/// after it alone, unlike `undo`, which would walk back through all of them.
#[test]
fn a_result_is_undone_by_its_match() {
    let mut session = Given::new().started();
    for number in 1..=3 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let reopened = session.undo_report(numbered(&session, 1)).unwrap();

    assert_eq!(reopened.unscheduled, 0);
    assert_eq!(
        (reopened.row.number, reopened.row.result),
        (1, Some(MatchResult::AWins)),
        "the match with the result it had"
    );
    let results: Vec<_> = matches(&session).iter().map(|row| row.result).collect();
    assert_eq!(
        results[..3],
        [None, Some(MatchResult::AWins), Some(MatchResult::AWins)]
    );
    assert!(matches!(
        session.undo_report(numbered(&session, 1)),
        Err(UndoReportError::NotReported)
    ));
}

/// Undoing the result of a semifinal unschedules the final it decided the participants of, so a
/// final that has a result itself has to be undone first.
#[test]
fn a_result_a_later_one_depends_on_is_undone_after_it() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=3 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }
    let semifinal = numbered(&session, 1);

    assert!(matches!(
        session.undo_report(semifinal),
        Err(UndoReportError::DependedOnBy(numbers)) if numbers == [3]
    ));
    session.undo_report(numbered(&session, 3)).unwrap();
    let reopened = session.undo_report(semifinal).unwrap();

    assert_eq!(
        reopened.unscheduled, 2,
        "the final and the match for third place go with it"
    );
}

/// The winner of a semifinal plays the final and its loser the match for third place, so undoing
/// it names both once they have results.
#[test]
fn every_result_depending_on_an_undone_one_is_named() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    for number in 1..=4 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let result = session.undo_report(numbered(&session, 1));

    assert!(
        matches!(&result, Err(UndoReportError::DependedOnBy(numbers)) if *numbers == [3, 4]),
        "{:?}",
        result.err()
    );
}

/// The columns of the metrics the phase is ranked by come first, in the order of the criteria,
/// so that what decides the rank is next to the names; the others follow in running order.
#[test]
fn the_standings_show_the_columns_of_the_metrics() {
    let mut session = Given::new().started();
    for number in 1..=6 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }

    let standings = session.standings().unwrap();

    assert_eq!(
        standings.columns,
        [
            Column {
                calculator: "tournament_engine::points",
                header: "points",
                label: "points",
                description: Some("scored for every win, every draw and every bye"),
                ranks: true,
            },
            Column {
                calculator: "tournament_engine::sonneborn_berger",
                header: "Sonneborn-Berger",
                label: "Sonneborn-Berger",
                description: Some(
                    "the points of the opponents that were beaten or played to a draw"
                ),
                ranks: true,
            },
            Column {
                calculator: "tournament_engine::wins_draws_losses",
                header: "W-D-L",
                label: "win-draw-loss record",
                description: Some("how many matches were won, drawn and lost"),
                ranks: false,
            },
        ]
    );
    let points: Vec<Score> = standings
        .rows
        .iter()
        .map(|row| match row.cells[0] {
            Some(StandingCell::Metric(MetricCell::Number(points))) => points,
            ref cell => panic!("{cell:?} is no number of points"),
        })
        .collect();
    assert_eq!(points.len(), 4);
    assert!(
        points.windows(2).all(|pair| pair[0] >= pair[1]),
        "{standings:?}"
    );
    assert!(points[0] > points[3], "{standings:?}");
}

/// A round robin's cross table has a row per team, in the order of the standings, and in each the
/// one match against each other team, seen from the row's team.
#[test]
fn the_cross_table_shows_the_match_of_every_two_teams() {
    let mut session = Given::new().started();
    assert!(
        Given::new().added().cross_table().is_none(),
        "before the start"
    );
    let first = numbered(&session, 1);
    session.report(first, MatchResult::AWins).unwrap();

    let table = session.cross_table().unwrap();

    let teams: Vec<_> = table
        .rows
        .iter()
        .map(|row| names(&row.entrants).join(""))
        .collect();
    let standings: Vec<_> = session
        .standings()
        .unwrap()
        .rows
        .iter()
        .map(|row| names(&row.entrants).join(""))
        .collect();
    assert_eq!(teams, standings);
    for (i, row) in table.rows.iter().enumerate() {
        assert_eq!(row.cells.len(), 4);
        assert!(row.cells[i].is_empty(), "no match against itself");
        for (j, cell) in row.cells.iter().enumerate().filter(|(j, _)| *j != i) {
            let [played] = cell.as_slice() else {
                panic!("one match of {i} against {j}: {cell:?}");
            };
            let [mirrored] = table.rows[j].cells[i].as_slice() else {
                panic!("one match of {j} against {i}");
            };
            assert_eq!(played.row, mirrored.row);
            let expected = match played.outcome {
                Some(Outcome::Win) => Some(Outcome::Loss),
                Some(Outcome::Loss) => Some(Outcome::Win),
                outcome => outcome,
            };
            assert_eq!(mirrored.outcome, expected);
        }
    }
    let outcomes: Vec<_> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|played| played.outcome.map(|outcome| (played.row.id, outcome)))
        .collect();
    assert_eq!(outcomes.len(), 2, "the match reported, from either side");
    assert!(outcomes.iter().all(|(id, _)| *id == first));
    assert!(
        table.rows[0]
            .cells
            .iter()
            .flatten()
            .any(|played| played.outcome == Some(Outcome::Win)),
        "the winner first: {table:?}"
    );
    let totals: Vec<_> = table.rows.iter().map(|row| row.total).collect();
    assert_eq!(
        totals,
        [Score::ONE, Score::ZERO, Score::ZERO, Score::ZERO],
        "the winner's one win"
    );
}

/// A draw adds a half to the total of each of its teams.
#[test]
fn a_draw_adds_a_half_to_the_cross_table() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::Draw)
        .unwrap();

    let table = session.cross_table().unwrap();

    let halves = table
        .rows
        .iter()
        .filter(|row| row.total == Score::new(0.5))
        .count();
    assert_eq!(halves, 2);
}

/// A cell of the cross table has the result at its highest level, seen from the row's team: the
/// games won of a best of, not their scores.
#[test]
fn the_cross_table_has_the_games_won_from_the_side_of_the_row() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 3))
        .started();
    session.report_entered("1 21-17 19-21 21-15").unwrap();

    let table = session.cross_table().unwrap();

    let mut played: Vec<_> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|played| Some((played.outcome?, played.score?)))
        .collect();
    played.sort_by_key(|(outcome, _)| *outcome != Outcome::Win);
    assert_eq!(
        played,
        [
            (Outcome::Win, scores([(2, 1)])[0]),
            (Outcome::Loss, scores([(1, 2)])[0])
        ]
    );
}

/// A match scored as a whole has its score in the cross table, seen from the row's team.
#[test]
fn the_cross_table_has_the_score_of_a_match_from_the_side_of_the_row() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<Scored>())
        .started();
    session.report_entered("1 21-17").unwrap();

    let table = session.cross_table().unwrap();

    let mut played: Vec<_> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|played| Some((played.outcome?, played.score?)))
        .collect();
    played.sort_by_key(|(outcome, _)| *outcome != Outcome::Win);
    assert_eq!(
        played,
        [
            (Outcome::Win, scores([(21, 17)])[0]),
            (Outcome::Loss, scores([(17, 21)])[0])
        ]
    );
    let mut unscored = Given::new().started();
    unscored
        .report(numbered(&unscored, 1), MatchResult::AWins)
        .unwrap();
    assert!(
        unscored
            .cross_table()
            .unwrap()
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .flatten()
            .all(|played| played.score.is_none()),
        "nothing but who won"
    );
}

/// A knockout has its bracket instead, and the teams of a Monster DYP change every round.
#[test]
fn a_cross_table_is_only_of_teams_that_stay_the_same_without_a_bracket() {
    for format in [SingleElimination::ID, DoubleElimination::ID, MonsterDyp::ID] {
        let session = Given::new().format(format).started();

        assert!(session.cross_table().is_none(), "{format}");
    }
    let session = Given::new().format(RandomRounds::ID).started();
    assert!(session.cross_table().is_some());
}

/// A ranking that lost every match is told apart from one that has not played yet.
#[test]
fn the_standings_count_the_matches_played() {
    let mut session = Given::new().started();

    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    let mut played: Vec<_> = session
        .standings()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.played)
        .collect();
    played.sort_unstable();
    assert_eq!(played, [0, 0, 1, 1]);
}

/// Once every match has a result, the first three ranks win medals; Carol and Dave, tied for
/// third, both win bronze.
#[test]
fn the_first_three_ranks_win_medals_once_every_match_has_a_result() {
    let mut session = Given::new().started();
    let medals = |session: &Session| -> Vec<_> {
        session
            .standings()
            .unwrap()
            .rows
            .iter()
            .map(|row| (names(&row.entrants).join(""), row.medal))
            .collect()
    };
    assert!(medals(&session).iter().all(|(_, medal)| medal.is_none()));

    // Alice beats everyone, Bob everyone else, Carol and Dave draw
    let strength = ["Alice", "Bob", "Carol"];
    while let Some(row) = session.playable().first().cloned() {
        let rank = |team: &[EntrantRow]| {
            strength
                .iter()
                .position(|name| names(team) == [*name])
                .unwrap_or(strength.len() - 1)
        };
        let result = match rank(&row.a).cmp(&rank(&row.b)) {
            Ordering::Less => MatchResult::AWins,
            Ordering::Greater => MatchResult::BWins,
            Ordering::Equal => MatchResult::Draw,
        };
        session.report(row.id, result).unwrap();
    }

    let mut medals = medals(&session);
    // tied, in no order of their own
    medals[2..].sort_by(|(a, _), (b, _)| a.cmp(b));
    assert_eq!(
        medals,
        [
            ("Alice".to_owned(), Some(Medal::Gold)),
            ("Bob".to_owned(), Some(Medal::Silver)),
            ("Carol".to_owned(), Some(Medal::Bronze)),
            ("Dave".to_owned(), Some(Medal::Bronze)),
        ]
    );
}

#[test]
fn undo_and_redo_walk_back_and_forth_through_the_actions() {
    let mut session = Session::new("cup");
    assert!(
        matches!(session.undo(), Err(UndoError::NothingToUndo)),
        "the seed is no action of the session"
    );
    session.add_entrant("Alice").unwrap();
    session.add_entrant("Bob").unwrap();

    session.undo().unwrap();
    session.undo().unwrap();
    assert!(session.entrants().is_empty());
    assert!(matches!(session.undo(), Err(UndoError::NothingToUndo)));
    session.redo().unwrap();
    assert_eq!(names(&session.entrants()), ["Alice"]);

    session.add_entrant("Carol").unwrap();
    assert!(
        matches!(session.redo(), Err(RedoError::NothingToRedo)),
        "a new action discards what could be redone"
    );
    assert_eq!(names(&session.entrants()), ["Alice", "Carol"]);
}

#[test]
fn the_action_an_undo_would_undo_is_told_before() {
    let mut session = Given::new().started();
    assert_eq!(session.next_undo(), Some(Action::Start));
    let first = matches(&session).remove(0);

    session.report(first.id, MatchResult::AWins).unwrap();

    assert!(matches!(session.next_undo(), Some(Action::Report(_))));
    session.undo().unwrap();
    assert_eq!(session.next_undo(), Some(Action::Start));
    assert_eq!(Session::new("cup").next_undo(), None);
}

#[test]
fn undo_and_redo_tell_the_action_they_walked_through() {
    let mut session = Given::new().started();
    let first = matches(&session).remove(0);
    session.report(first.id, MatchResult::AWins).unwrap();
    session.undo_report(first.id).unwrap();
    let reported = MatchRow {
        result: Some(MatchResult::AWins),
        ..first
    };

    assert_eq!(session.undo().unwrap(), Action::Reopen(reported.clone()));
    assert_eq!(session.undo().unwrap(), Action::Report(reported.clone()));
    assert_eq!(session.undo().unwrap(), Action::Start);
    assert_eq!(session.redo().unwrap(), Action::Start);
    assert_eq!(session.redo().unwrap(), Action::Report(reported));

    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    session.remove_entrant(alice).unwrap();
    let alice = EntrantRow {
        id: alice,
        name: Some("Alice".to_owned()),
    };
    assert_eq!(
        session.undo().unwrap(),
        Action::RemoveEntrant(alice.clone())
    );
    assert_eq!(session.undo().unwrap(), Action::AddEntrant(alice));
}

/// In a round robin of four, every match of the second round has an entrant of each match of the
/// first, so it can only be played once both of those have.
#[test]
fn a_match_can_be_played_once_its_entrants_played_the_rounds_before() {
    let mut session = Given::new().started();
    let numbers = |session: &Session| -> Vec<usize> {
        session.playable().iter().map(|row| row.number).collect()
    };
    assert_eq!(numbers(&session), [1, 2]);

    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    assert_eq!(numbers(&session), [2]);
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    assert_eq!(numbers(&session), [3, 4]);
    assert!(Given::new().added().playable().is_empty());
}

/// The numbers of the matches a list of rounds shows.
fn listed(view: &RoundsView) -> Vec<usize> {
    view.matches
        .iter()
        .map(|listed| listed.row.number)
        .collect()
}

#[test]
fn the_rounds_show_the_first_round_with_an_open_match_unless_another_is_chosen() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    let view = session.rounds_view(None, "").unwrap();
    assert_eq!(view.current, round(2));
    assert_eq!(view.shown, Shown::Round(round(2)));
    assert_eq!((view.previous, view.next), (Some(round(1)), Some(round(3))));
    assert_eq!(listed(&view), [3, 4]);
    assert!(
        !view.with_round,
        "a round shown names none of its matches by it"
    );

    let first = session
        .rounds_view(Some(Shown::Round(round(1))), "")
        .unwrap();
    assert_eq!((first.previous, first.next), (None, Some(round(2))));
    assert_eq!(listed(&first), [1, 2]);

    let gone = session
        .rounds_view(Some(Shown::Round(round(9))), "")
        .unwrap();
    assert_eq!(
        gone.shown,
        Shown::Round(round(2)),
        "a round that no longer exists falls back to the current one"
    );
}

#[test]
fn the_rounds_show_the_playable_matches_each_by_its_round() {
    let session = Given::new().started();

    let view = session.rounds_view(Some(Shown::Playable), "").unwrap();

    assert_eq!(listed(&view), [1, 2]);
    assert_eq!(view.playable, 2);
    assert_eq!((view.previous, view.next), (None, None));
    assert!(view.with_round);
}

/// After a result is reported, the operator goes on to the next match without one, wrapping around
/// to the first.
#[test]
fn each_open_match_listed_leads_on_to_the_next_open_one() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"])
        .started();
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();

    let view = session.rounds_view(None, "").unwrap();

    let next: Vec<_> = view
        .matches
        .iter()
        .map(|listed| (listed.row.number, listed.next_open))
        .collect();
    let id = |number| Some(numbered(&session, number));
    assert_eq!(next, [(1, id(3)), (2, None), (3, id(1))]);
}

#[test]
fn the_rounds_show_the_matches_a_typed_result_fits() {
    let mut session = Given::new().started();

    let view = session.rounds_view(None, " 3 1 ").unwrap();

    let entry = view.entry.as_ref().unwrap();
    assert_eq!(entry.text, "3 1");
    assert_eq!(entry.report.as_ref().unwrap().number, 3);
    assert_eq!(listed(&view), [3]);
    assert!(view.with_round);
    assert!(view.byes.is_empty());
    let blank = session.rounds_view(None, "  ").unwrap();
    assert!(blank.entry.is_none());
    assert_eq!(listed(&blank), [1, 2]);

    while let Some(row) = session.playable().first().cloned() {
        session.report(row.id, MatchResult::AWins).unwrap();
    }
    let finished = session.rounds_view(None, "3 1").unwrap();
    assert!(finished.finished);
    assert!(
        finished.entry.is_none(),
        "nothing is left to type once every match has a result"
    );
    assert_eq!(finished.current, round(3), "the last round");
}

#[test]
fn there_are_no_rounds_to_show_before_the_start() {
    assert!(Given::new().added().rounds_view(None, "1 1").is_none());
}

/// A round robin of four plays three rounds of two matches.
#[test]
fn the_progress_is_the_first_round_with_a_match_to_play() {
    let mut session = Given::new().added();
    assert_eq!(session.progress(), Progress::Setup { entrants: 4 });
    session.start().unwrap();
    assert_eq!(
        session.progress(),
        Progress::Playing {
            round: round(1),
            open: 2
        }
    );

    for number in 1..=2 {
        session
            .report(numbered(&session, number), MatchResult::Draw)
            .unwrap();
    }
    assert_eq!(
        session.progress(),
        Progress::Playing {
            round: round(2),
            open: 2
        }
    );
    for number in 3..=6 {
        session
            .report(numbered(&session, number), MatchResult::Draw)
            .unwrap();
    }

    let Progress::Won { winners } = session.progress() else {
        panic!("every match has a result");
    };
    // every match drawn, everyone is tied for first, listed by name
    assert_eq!(
        winners,
        ["Alice", "Bob", "Carol", "Dave"].map(|name| vec![Some(name.to_owned())])
    );
}

/// A listing stored before the winners were reads as it did.
#[test]
fn a_progress_stored_without_the_winners_still_reads() {
    assert_eq!(
        serde_json::from_str::<Progress>(r#""Finished""#).unwrap(),
        Progress::Finished
    );
}

/// The winner is known once every match has a result, not before.
#[test]
fn the_winner_is_the_team_ranked_first_once_every_match_is_played() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob"])
        .format(SingleElimination::ID)
        .started();
    assert_eq!(session.winners(), None);
    let final_ = &matches(&session)[0];
    let winner = final_.a.clone();

    session.report(final_.id, MatchResult::AWins).unwrap();

    assert_eq!(session.winners(), Some(vec![winner.clone()]));
    assert_eq!(
        session.progress(),
        Progress::Won {
            winners: vec![winner.into_iter().map(|entrant| entrant.name).collect()]
        }
    );
    session.undo().unwrap();
    assert_eq!(session.winners(), None);
}

#[test]
fn a_save_written_after_another_stores_over_it_whichever_finishes_first() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    let older = session.to_save();
    session.add_entrant("Bob").unwrap();
    let newer = session.to_save();

    let older = storage.write(&older);
    let newer = storage.write(&newer);
    now(newer).unwrap();
    now(older).unwrap();

    let listed = now(storage.list()).unwrap();
    assert_eq!(listed[0].progress, Some(Progress::Setup { entrants: 2 }));
    let loaded = now(Session::load(&storage, session.key())).unwrap();
    assert_eq!(names(&loaded.entrants()), ["Alice", "Bob"]);
}

#[test]
fn a_search_finds_an_entrant_without_a_name_only_when_nothing_is_typed() {
    let nameless = EntrantRow {
        id: EntrantId::new(Uuid::nil()),
        name: None,
    };

    assert!(NameSearch::new("").finds(&nameless));
    assert!(!NameSearch::new("a").finds(&nameless));
}

#[test]
fn a_result_is_entered_as_the_number_of_its_match_and_1_x_or_2() {
    let mut session = Given::new().started();

    session.report_entered("1 2").unwrap();
    let reported = session.report_entered("  2  x ").unwrap();
    session.report_entered("3-1").unwrap();
    session.report_entered("4X").unwrap();

    let results: Vec<_> = matches(&session).iter().map(|row| row.result).collect();
    assert_eq!(
        results[..5],
        [
            Some(MatchResult::BWins),
            Some(MatchResult::Draw),
            Some(MatchResult::AWins),
            Some(MatchResult::Draw),
            None
        ]
    );
    assert_eq!(reported.row.number, 2);
}

#[test]
fn a_result_entered_for_a_match_that_has_one_names_the_match() {
    let mut session = Given::new().started();
    session.report_entered("2 1").unwrap();

    assert!(matches!(
        session.report_entered("2 2"),
        Err(ReportEnteredError::AlreadyFinished(2))
    ));
}

#[test]
fn a_result_entered_is_refused_unless_it_reads_as_one_of_a_match() {
    let mut session = Given::new().format(SingleElimination::ID).started();

    for entry in ["", " "] {
        assert!(
            matches!(
                session.report_entered(entry),
                Err(ReportEnteredError::Unreadable)
            ),
            "{entry:?}"
        );
    }
    // "11" could be match 1 won by side A, or match 11, which is read
    let entered = session.read_entered("1");
    assert!(matches!(
        entered.report,
        Err(ReportEnteredError::MissingResult(1))
    ));
    assert_eq!(numbers(&entered.found), [1], "the match is shown");
    // read as part of a name, which nobody has
    for entry in ["1 3", "one 1", "1 1 1", "-1 1", "1x1", "1 x 1", "1:1"] {
        assert!(
            matches!(
                session.report_entered(entry),
                Err(ReportEnteredError::NoNameFound(name)) if name == entry
            ),
            "{entry:?}"
        );
    }
    for (entry, number) in [("0 1", 0), ("9 1", 9), ("11", 11)] {
        assert!(
            matches!(
                session.report_entered(entry),
                Err(ReportEnteredError::NoMatch(n)) if n == number
            ),
            "{entry:?}"
        );
    }
    assert!(matches!(
        session.report_entered("1 X"),
        Err(ReportEnteredError::Report(ReportError::DrawNotAllowed))
    ));
    assert!(matches(&session).iter().all(|row| row.result.is_none()));
}

fn count(count: u16) -> Option<EnteredValue> {
    Some(EnteredValue::Count(count))
}

/// The scores of `a` to `b`, each pair one.
fn scores<const N: usize>(scores: [(u16, u16); N]) -> Vec<Sides<Option<EnteredValue>>> {
    scores
        .into_iter()
        .map(|(a, b)| Sides::new(count(a), count(b)))
        .collect()
}

#[test]
fn scores_are_entered_after_the_number_of_a_match_and_say_its_result() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 3))
        .started();

    let entered = session
        .read_entered(" 1  21-17 19:21 21–15 ")
        .report
        .unwrap();
    assert_eq!(entered.result, Some(MatchResult::AWins));
    assert_eq!(entered.scores, scores([(21, 17), (19, 21), (21, 15)]));
    let reported = session.report_entered("1 21-17 19:21 21–15").unwrap();
    assert_eq!(reported.row.scores, entered.scores);
    assert_eq!(reported.row.result, Some(MatchResult::AWins));
    let reported = session.report_entered("2 17-21 21-17 2").unwrap();
    assert_eq!(
        reported.row.result,
        Some(MatchResult::BWins),
        "the result entered decides where the scores do not"
    );
    assert_eq!(reported.row.scores, scores([(17, 21), (21, 17)]));
}

#[test]
fn scores_entered_are_refused_unless_they_fit_the_result() {
    let mut session = Given::new()
        .match_format(PluginConfig::of::<BestOf>().with("games", 3))
        .started();

    assert!(matches!(
        session.report_entered("1 21-17"),
        Err(ReportEnteredError::Undecided(1))
    ));
    assert!(matches!(
        session.report_entered("1 21-17 21-19 2"),
        Err(ReportEnteredError::AgainstScores(1))
    ));
    assert!(matches!(
        session.report_entered("1 1-0 1-0 1-0 1-0"),
        Err(ReportEnteredError::ScoresNotTaken(1))
    ));
    assert!(matches(&session).iter().all(|row| row.result.is_none()));

    let mut session = Given::new()
        .match_format(PluginConfig::of::<Scored>())
        .started();
    assert!(matches!(
        session.report_entered("1 21.5-17"),
        Err(ReportEnteredError::ScoresNotTaken(1))
    ));
    assert_eq!(
        session.report_entered("1 21,0-17").unwrap().row.scores,
        scores([(21, 17)]),
        "a whole number in a score of counts"
    );

    let mut session = Given::new().started();
    assert!(matches!(
        session.report_entered("1 3-1"),
        Err(ReportEnteredError::ScoresNotTaken(1))
    ));
    assert!(
        session.report_entered("1-1").is_ok(),
        "a win of side A, as without scores"
    );
}

#[test]
fn scores_entered_after_a_name_are_the_named_sides() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob"])
        .match_format(PluginConfig::of::<Scored>())
        .started();
    let the_match = &matches(&session)[0];
    let alice = win_of(the_match, "Alice");
    let alices_side = |a, b| {
        if alice == MatchResult::AWins {
            scores([(a, b)])
        } else {
            scores([(b, a)])
        }
    };

    assert!(matches!(
        session.read_entered("ali 17-21").report,
        Err(ReportEnteredError::AgainstScores(1))
    ));
    let won = session.read_entered("ali 1 21-17").report.unwrap();
    assert_eq!(won.result, Some(alice));
    assert_eq!(won.scores, alices_side(21, 17));
    let reported = session.report_entered("bob 21-19").unwrap();
    assert_eq!(reported.row.scores, alices_side(19, 21));
    assert_ne!(reported.row.result, Some(alice));
}

#[test]
fn a_draw_entered_with_scores_needs_them_to_say_it() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob"])
        .match_format(PluginConfig::of::<Scored>())
        .started();

    assert!(matches!(
        session.read_entered("ali x 2-1").report,
        Err(ReportEnteredError::AgainstScores(1))
    ));
    let drawn = session.report_entered("Alice X 2:2").unwrap();
    assert_eq!(drawn.row.result, Some(MatchResult::Draw));
    assert_eq!(drawn.row.scores, scores([(2, 2)]));
}

#[test]
fn a_drawn_score_is_refused_where_there_are_no_draws() {
    let mut session = Given::new()
        .format(SingleElimination::ID)
        .match_format(PluginConfig::of::<Scored>())
        .started();

    assert!(matches!(
        session.report_entered("1 2-2"),
        Err(ReportEnteredError::Report(ReportError::DrawNotAllowed))
    ));
}

/// The numbers of `rows`, in order.
fn numbers(rows: &[MatchRow]) -> Vec<usize> {
    rows.iter().map(|row| row.number).collect()
}

/// The open matches of `name`, by number.
fn open_matches_of(session: &Session, name: &str) -> Vec<MatchRow> {
    let mut rows: Vec<_> = matches(session)
        .into_iter()
        .filter(|row| {
            row.result.is_none()
                && names(&row.a)
                    .iter()
                    .chain(&names(&row.b))
                    .any(|n| *n == name)
        })
        .collect();
    rows.sort_by_key(|row| row.number);
    rows
}

/// The win of the side of `name` in `row`.
fn win_of(row: &MatchRow, name: &str) -> MatchResult {
    if names(&row.a) == [name] {
        MatchResult::AWins
    } else {
        MatchResult::BWins
    }
}

/// Part of the winner's name fits every open match of the entrant, all of which are shown, and
/// reports nothing while they are several, even if just one is playable now: Alice plays a match
/// in every round of a round robin, so the number of the one meant is needed.
#[test]
fn a_win_is_entered_as_part_of_the_winners_name() {
    let mut session = Given::new().started();
    let alices = open_matches_of(&session, "Alice");

    let entered = session.read_entered(" ALI ");

    assert_eq!(numbers(&entered.found), numbers(&alices));
    assert!(matches!(
        entered.report,
        Err(ReportEnteredError::SeveralMatches { name, matches: 3, number })
            if name == "ALI" && number == alices[0].number
    ));
    assert!(session.report_entered("ali").is_err());
    let meant = &alices[1];
    let entry = format!(" ali {} ", meant.number);
    assert_eq!(numbers(&session.read_entered(&entry).found), [meant.number]);
    let reported = session.report_entered(&entry).unwrap();
    assert_eq!(reported.row.id, meant.id);
    assert_eq!(reported.row.result, Some(win_of(meant, "Alice")));
}

/// A draw is entered as part of a name and an `X`, before or after the number of the match.
#[test]
fn a_draw_is_entered_as_part_of_a_name_and_x() {
    let mut session = Given::new().started();
    let alices = open_matches_of(&session, "Alice");

    let first = session
        .report_entered(&format!("ali {} X", alices[0].number))
        .unwrap();
    let second = session
        .report_entered(&format!("Ali x {}", alices[1].number))
        .unwrap();
    let third = session.report_entered("ali x").unwrap();

    assert_eq!(
        [first.row.result, second.row.result, third.row.result],
        [Some(MatchResult::Draw); 3]
    );
    assert_eq!(
        numbers(&[first.row, second.row, third.row]),
        numbers(&alices),
        "the last one without a number, being the only one left"
    );
}

/// A number or an `X` at the end is part of the name as long as a name contains it.
#[test]
fn a_name_with_a_number_or_an_x_is_read_as_the_name_first() {
    let mut session = Given::new().entrants(&["Malcolm X", "Team 7"]).started();
    let the_match = &matches(&session)[0];

    let won = session.read_entered("malcolm x").report.unwrap();
    assert_eq!(won.result, Some(win_of(the_match, "Malcolm X")));
    let won = session.read_entered("team 7").report.unwrap();
    assert_eq!(won.result, Some(win_of(the_match, "Team 7")));
    let drawn = session.read_entered("Malcolm X X").report.unwrap();
    assert_eq!(drawn.result, Some(MatchResult::Draw));
    assert!(
        session.read_entered("team 7 1").report.is_ok(),
        "Team 7 in match 1"
    );
    assert!(matches!(
        session.read_entered("team 7 9").report,
        Err(ReportEnteredError::NotInMatch { name, number: 9 }) if name == "team 7"
    ));
    session.report_entered("team 7 1 x").unwrap();
    assert!(
        matches!(
            session.read_entered("team").report,
            Err(ReportEnteredError::NoNameFound(_))
        ),
        "a match with a result is not open"
    );
}

/// A name found on both sides of a match names no winner, but a draw of it; a name nobody has
/// reports nothing.
#[test]
fn a_name_entered_has_to_fit_one_side() {
    let mut session = Given::new().entrants(&["Anna", "Hannah"]).started();

    assert!(matches!(
        session.report_entered("ANN"),
        Err(ReportEnteredError::BothSides { name, number: 1 }) if name == "ANN"
    ));
    assert!(matches!(
        session.report_entered("Erin"),
        Err(ReportEnteredError::NoNameFound(name)) if name == "Erin"
    ));
    assert!(matches(&session).iter().all(|row| row.result.is_none()));
    let drawn = session.report_entered("ann x").unwrap();
    assert_eq!(drawn.row.result, Some(MatchResult::Draw));
}

/// The number of a match with a result fits it, so that it is shown, but reports nothing.
#[test]
fn a_match_entered_with_a_result_is_shown_but_not_reported() {
    let mut session = Given::new().started();
    session.report_entered("1 1").unwrap();

    let entered = session.read_entered("1 2");

    assert_eq!(numbers(&entered.found), [1]);
    assert!(matches!(
        entered.report,
        Err(ReportEnteredError::AlreadyFinished(1))
    ));
}

/// `undo_report` is an action like any other, so `undo` brings the result back rather than undoing
/// the action before it.
#[test]
fn undo_brings_back_a_result_undone_by_its_match() {
    let mut session = Given::new().started();
    let first = numbered(&session, 1);
    session.report(first, MatchResult::AWins).unwrap();
    session.undo_report(first).unwrap();

    session.undo().unwrap();

    assert_eq!(matches(&session)[0].result, Some(MatchResult::AWins));
}

/// An edit starts out at the configuration the phase is played with, so changing the ranking keeps
/// the metrics, and the results reported so far stay.
#[test]
fn a_phase_is_edited_starting_out_at_its_configuration() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    let mut draft = session.phase_draft_to_edit().unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![PluginConfig::of::<MoreWins>()],
        )
        .unwrap();

    assert!(session.edit_phase(&draft).unwrap());

    assert_eq!(matches(&session)[0].result, Some(MatchResult::AWins));
    let headers: Vec<_> = session
        .standings()
        .unwrap()
        .columns
        .iter()
        .map(|column| column.header)
        .collect();
    assert_eq!(headers, ["W-D-L", "points", "Sonneborn-Berger"]);
    session.undo().unwrap();
    assert_eq!(
        session.phase_draft_to_edit().unwrap().answers(),
        Given::new()
            .started()
            .phase_draft_to_edit()
            .unwrap()
            .answers()
    );
}

#[test]
fn a_phase_edited_without_a_change_is_left_as_it_is() {
    let mut session = Given::new().started();
    let draft = session.phase_draft_to_edit().unwrap();
    let undoable = session.actions().undo;

    assert!(!session.edit_phase(&draft).unwrap());
    session.undo().unwrap();

    assert!(undoable);
    assert!(
        !session.competition().is_started(),
        "the edit that changed nothing is no action, so undo undoes the start"
    );
}

/// More rounds keep those drawn already, so rounds are added after a result.
#[test]
fn rounds_are_added_to_random_rounds_after_a_result() {
    let mut session = Given::new()
        .entrants(&["A", "B", "C", "D", "E", "F", "G", "H"])
        .format(RandomRounds::ID)
        .started();
    let first = numbered(&session, 1);
    session.report(first, MatchResult::AWins).unwrap();
    let mut draft = session.phase_draft_to_edit().unwrap();
    draft
        .set(&ConfigPath::ROOT.field("format").field("rounds"), 7_u32)
        .unwrap();

    assert!(session.edit_phase(&draft).unwrap());

    assert_eq!(session.rounds().len(), 7);
    assert_eq!(numbered(&session, 1), first);
    assert_eq!(
        matches(&session)[0].result,
        Some(MatchResult::AWins),
        "the result stays"
    );
}

/// The teams of a Monster DYP are pairs, so it never schedules the match of single entrants whose
/// result was reported, which the refusal names.
#[test]
fn an_edit_a_reported_result_does_not_fit_is_refused() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    let draft = session.phase_draft(MonsterDyp::ID).unwrap();

    let result = session.edit_phase(&draft);

    assert!(
        matches!(&result, Err(EditPhaseError::ResultDoesNotFit(numbers)) if *numbers == [1]),
        "{result:?}"
    );
    assert_eq!(matches(&session)[0].result, Some(MatchResult::AWins));
    assert!(matches!(
        Session::new("cup").phase_draft_to_edit(),
        Err(NoPhase)
    ));
}

/// A refused edit names every result that does not fit it, not only the first.
#[test]
fn an_edit_names_every_reported_result_it_does_not_fit() {
    let mut session = Given::new().started();
    for number in 1..=2 {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }
    let draft = session.phase_draft(MonsterDyp::ID).unwrap();

    let result = session.edit_phase(&draft);

    assert!(
        matches!(&result, Err(EditPhaseError::ResultDoesNotFit(numbers)) if *numbers == [1, 2]),
        "{result:?}"
    );
}

/// The matches of the phase that have a result, with that result.
fn reported(session: &Session) -> Vec<(MatchId, MatchResult)> {
    matches(session)
        .into_iter()
        .filter_map(|row| Some((row.id, row.result?)))
        .collect()
}

/// A round robin pairs its first round as a knockout does, so the results of a knockout's first
/// round stay when it is changed to a round robin.
#[test]
fn a_knockout_changed_to_a_round_robin_keeps_its_first_results() {
    let mut session = Given::new().format(SingleElimination::ID).started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();
    let before = reported(&session);

    assert!(
        session
            .edit_phase(&session.phase_draft(RoundRobin::ID).unwrap())
            .unwrap()
    );

    assert_eq!(reported(&session), before);
}

/// A round robin plays the final of a knockout in another round, if at all, so a knockout played
/// to its end is not changed to a round robin, which keeps its results.
#[test]
fn a_finished_knockout_is_not_changed_to_a_round_robin() {
    let mut session = Given::new()
        .entrants(&["A", "B", "C", "D", "E", "F", "G", "H"])
        .format(SingleElimination::ID)
        .started();
    while let Some(open) = matches(&session)
        .into_iter()
        .find(|row| row.result.is_none())
    {
        session.report(open.id, MatchResult::AWins).unwrap();
    }
    let before = reported(&session);

    let result = session.edit_phase(&session.phase_draft(RoundRobin::ID).unwrap());

    assert!(
        matches!(result, Err(EditPhaseError::ResultDoesNotFit(_))),
        "{result:?}"
    );
    assert_eq!(reported(&session), before);
}

/// What can be undone is what a session did itself, so a loaded competition has the state it was
/// saved with but nothing to undo.
#[test]
fn a_saved_competition_loads_as_it_was_saved_without_anything_to_undo() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    now(session.save(&storage)).unwrap();
    let mut loaded = now(Session::load(&storage, session.key())).unwrap();

    assert_eq!(loaded.name(), "cup");
    assert!(!loaded.has_unsaved_changes());
    assert_eq!(loaded.entrants(), session.entrants());
    assert_eq!(matches(&loaded), matches(&session));
    assert_eq!(loaded.standings(), session.standings());
    assert!(matches!(loaded.undo(), Err(UndoError::NothingToUndo)));
    // a single result can still be undone, as it is found in the log
    loaded.undo_report(numbered(&loaded, 1)).unwrap();
    assert_eq!(
        now(storage.list()).unwrap(),
        [Listing {
            key: session.key(),
            name: "cup".to_owned(),
            progress: Some(session.progress()),
        }]
    );
}

#[test]
fn every_change_is_unsaved_until_the_next_save() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    assert!(session.has_unsaved_changes(), "the seed is not saved yet");
    now(session.save(&storage)).unwrap();
    assert!(!session.has_unsaved_changes());

    session.add_entrant("Alice").unwrap();
    assert!(session.has_unsaved_changes());
    now(session.save(&storage)).unwrap();
    assert!(!session.has_unsaved_changes());
    session.undo().unwrap();
    assert!(
        session.has_unsaved_changes(),
        "an undo is an entry of the log too"
    );
}

/// The record of the competition `key` in `storage`, as JSON.
fn record(storage: &MemoryStorage, key: CompetitionKey) -> serde_json::Value {
    let stored = now(storage.read(key)).unwrap().unwrap();
    serde_json::from_str(&stored.record).unwrap()
}

/// Leaves the competition `key` as a version that did not store the competition its log replays
/// into would have.
fn forget_competition(storage: &MemoryStorage, key: CompetitionKey) {
    let mut record = record(storage, key);
    record.as_object_mut().unwrap().remove("competition");
    storage.set_record(key, record.to_string());
}

/// A competition is stored as its file, so that it is exported as it is.
#[test]
fn a_save_stores_the_file_without_the_name() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    now(session.save(&storage)).unwrap();

    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    file.as_object_mut().unwrap().remove("name");
    assert_eq!(record(&storage, session.key()), file);
}

#[test]
fn a_competition_saved_by_a_newer_version_is_refused_as_such() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    now(session.save(&storage)).unwrap();
    let mut record = record(&storage, session.key());
    record["format"] = serde_json::json!(1000);
    storage.set_record(session.key(), record.to_string());

    assert!(matches!(
        now(Session::load(&storage, session.key())),
        Err(LoadError::Newer(_))
    ));
}

/// Opening a competition an older version saved is enough to store what its log replays into,
/// while that version's log still replays.
#[test]
fn a_competition_stored_without_what_its_log_replays_into_gets_it_on_the_next_save() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    now(session.save(&storage)).unwrap();
    forget_competition(&storage, session.key());

    let mut loaded = now(Session::load(&storage, session.key())).unwrap();

    assert!(loaded.has_unsaved_changes());
    now(loaded.save(&storage)).unwrap();
    assert!(!loaded.has_unsaved_changes());
    assert_eq!(
        record(&storage, session.key())["competition"],
        serde_json::to_value(session.competition()).unwrap()
    );
    let reloaded = now(Session::load(&storage, session.key())).unwrap();
    assert!(!reloaded.has_unsaved_changes());
}

/// A tab that opened such a competition read-only follows the one changing it.
#[test]
fn a_competition_stored_without_what_its_log_replays_into_is_caught_up_with() {
    let storage = MemoryStorage::default();
    let mut changing = Given::new().started();
    now(changing.save(&storage)).unwrap();
    forget_competition(&storage, changing.key());
    let mut following = now(Session::load(&storage, changing.key())).unwrap();

    changing
        .report(numbered(&changing, 1), MatchResult::AWins)
        .unwrap();
    now(changing.save(&storage)).unwrap();

    assert_eq!(now(following.catch_up(&storage)).unwrap(), 1);
    assert!(
        !following.has_unsaved_changes(),
        "the storage holds it again"
    );
}

/// A GUI writes changes while the operator goes on, so a write may finish after a later one.
#[test]
fn saves_written_out_of_order_leave_nothing_unsaved() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    let first = session.to_save();
    session.add_entrant("Bob").unwrap();
    let second = session.to_save();

    session.mark_saved(&second);
    session.mark_saved(&first);

    assert!(!session.has_unsaved_changes());
}

/// A save taken before a person changed leaves that change unsaved once it is written.
#[test]
fn a_person_changed_after_the_save_was_taken_stays_unsaved() {
    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    let save = session.to_save();
    session.rename_entrant(alice, "Alicia").unwrap();

    session.mark_saved(&save);

    assert!(session.has_unsaved_changes());
}

/// Another tab, say, shows what the session changing the competition saved, without replaying the
/// log each time.
#[test]
fn a_session_catches_up_with_what_another_one_saved() {
    let storage = MemoryStorage::default();
    let mut changing = Given::new().started();
    now(changing.save(&storage)).unwrap();
    let mut following = now(Session::load(&storage, changing.key())).unwrap();

    changing
        .report(numbered(&changing, 1), MatchResult::AWins)
        .unwrap();
    changing
        .report(numbered(&changing, 2), MatchResult::BWins)
        .unwrap();
    // entries that undo an earlier one, which replay the log after it
    changing.undo_report(numbered(&changing, 1)).unwrap();
    changing.undo().unwrap();
    now(changing.save(&storage)).unwrap();

    assert_eq!(now(following.catch_up(&storage)).unwrap(), 4);
    assert_eq!(matches(&following), matches(&changing));
    assert_eq!(following.standings(), changing.standings());
    assert!(!following.has_unsaved_changes());
    assert!(matches!(following.undo(), Err(UndoError::NothingToUndo)));
    assert_eq!(now(following.catch_up(&storage)).unwrap(), 0);
}

#[test]
fn catching_up_takes_the_people_as_stored() {
    let storage = MemoryStorage::default();
    let mut changing = Session::new("cup");
    changing.add_entrant("Alice").unwrap();
    now(changing.save(&storage)).unwrap();
    let mut following = now(Session::load(&storage, changing.key())).unwrap();

    changing.add_entrant("Bob").unwrap();
    now(changing.save(&storage)).unwrap();
    now(following.catch_up(&storage)).unwrap();

    assert_eq!(names(&following.entrants()), ["Alice", "Bob"]);
}

/// What another session saved may have reported a match this one reopened, and reopened it again,
/// so after catching up, the form of the match no longer starts out at the result this one took
/// back.
#[test]
fn catching_up_drops_the_results_taken_back() {
    let storage = MemoryStorage::default();
    let mut session = Given::new()
        .match_format(PluginConfig::of::<Scored>())
        .started();
    session.report_entered("1 21-17").unwrap();
    session.undo_report(numbered(&session, 1)).unwrap();
    now(session.save(&storage)).unwrap();
    let mut other = now(Session::load(&storage, session.key())).unwrap();
    other.report_entered("1 21-19").unwrap();
    other.undo_report(numbered(&other, 1)).unwrap();
    now(other.save(&storage)).unwrap();

    now(session.catch_up(&storage)).unwrap();

    let form = session.result_form(numbered(&session, 1)).unwrap();
    assert_eq!(form.view().root.fields[0].values, Sides::new(None, None));
}

/// The started competition of `session` saved to `storage`, and a session of another tab that
/// loaded it.
fn shared(session: &mut Session, storage: &MemoryStorage) -> Session {
    now(session.save(storage)).unwrap();
    now(Session::load(storage, session.key())).unwrap()
}

/// A result another session reported stays when this one undoes its own.
#[test]
fn undo_takes_back_the_session_s_own_result_and_not_one_caught_up_with() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 2), MatchResult::BWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    let undone = session.undo().unwrap();

    assert!(matches!(undone, Action::Report(row) if row.number == 1));
    let results: Vec<_> = matches(&session).iter().map(|row| row.result).collect();
    assert_eq!(results[..2], [None, Some(MatchResult::BWins)]);
}

#[test]
fn redo_brings_back_what_the_session_undid_after_catching_up() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    session.undo().unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 2), MatchResult::BWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    assert!(session.actions().redo);
    assert!(matches!(session.redo().unwrap(), Action::Report(row) if row.number == 1));
}

/// Undoing a result another session reopened already would undo an older action while the result
/// could still come back, so the session undoes nothing more.
#[test]
fn a_result_another_session_reopened_leaves_nothing_to_undo() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other.undo_report(numbered(&other, 1)).unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    assert!(!session.actions().undo);
    assert_eq!(session.next_undo(), None);
    assert!(matches!(session.undo(), Err(UndoError::ChangedElsewhere)));
    assert!(matches!(session.undo(), Err(UndoError::NothingToUndo)));
    assert!(
        session.competition().is_started(),
        "the start is not undone"
    );
}

/// The final another session reported depends on the semifinal this one reported last.
#[test]
fn a_result_another_session_s_result_depends_on_is_not_undone_and_ends_undoing() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().format(SingleElimination::ID).started();
    let mut other = shared(&mut session, &storage);
    for number in [1, 2] {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 3), MatchResult::AWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();
    let before = session.export();

    assert!(matches!(session.next_undo(), Some(Action::Report(row)) if row.number == 2));
    assert!(matches!(session.undo(), Err(UndoError::ChangedElsewhere)));

    assert_eq!(session.export(), before);
    assert!(!session.actions().undo);
}

/// A result this session undid and another one reported again cannot be redone, while what this
/// session did before stays undoable.
#[test]
fn a_result_another_session_reported_again_leaves_nothing_to_redo() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();
    session.undo().unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 2), MatchResult::BWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    assert!(matches!(session.redo(), Err(RedoError::ChangedElsewhere)));

    assert!(!session.actions().redo);
    assert!(matches!(session.undo().unwrap(), Action::Report(row) if row.number == 1));
}

#[test]
fn an_entry_stored_that_does_not_apply_is_not_caught_up_with() {
    let storage = MemoryStorage::default();
    let mut changing = Session::new("cup");
    now(changing.save(&storage)).unwrap();
    let mut following = now(Session::load(&storage, changing.key())).unwrap();
    changing.add_entrant("Alice").unwrap();
    now(changing.save(&storage)).unwrap();
    let mut record = record(&storage, changing.key());
    record["log"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "unknown": true }));
    storage.set_record(changing.key(), record.to_string());

    assert!(matches!(
        now(following.catch_up(&storage)),
        Err(LoadError::Damaged(_))
    ));
    assert!(following.entrants().is_empty(), "nothing is appended");
}

#[test]
#[should_panic = "a session catches up only with nothing unsaved"]
#[cfg_attr(target_arch = "wasm32", ignore = "WebAssembly aborts on a panic")]
fn a_session_with_unsaved_changes_does_not_catch_up() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    now(session.save(&storage)).unwrap();
    session.add_entrant("Alice").unwrap();

    _ = now(session.catch_up(&storage));
}

/// A storage whose writes fail as long as it is told to.
#[derive(Default)]
struct Flaky {
    storage: MemoryStorage,
    failing: Cell<bool>,
}

#[derive(Debug)]
struct WriteFailed;

impl fmt::Display for WriteFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the write failed")
    }
}

impl std::error::Error for WriteFailed {}

impl Storage for Flaky {
    type Error = WriteFailed;

    async fn list(&self) -> Result<Vec<Listing>, WriteFailed> {
        Ok(now(self.storage.list()).unwrap())
    }

    async fn read(&self, key: CompetitionKey) -> Result<Option<Stored>, WriteFailed> {
        Ok(now(self.storage.read(key)).unwrap())
    }

    fn write(&self, save: &Save) -> impl Future<Output = Result<(), WriteFailed>> + use<> {
        let written = if self.failing.get() {
            Err(WriteFailed)
        } else {
            now(self.storage.write(save)).unwrap();
            Ok(())
        };
        std::future::ready(written)
    }

    async fn rename(&self, key: CompetitionKey, name: &str) -> Result<(), WriteFailed> {
        now(self.storage.rename(key, name)).unwrap();
        Ok(())
    }

    async fn delete(&self, key: CompetitionKey) -> Result<(), WriteFailed> {
        now(self.storage.delete(key)).unwrap();
        Ok(())
    }
}

/// A competition renamed keeps its name through the saves of a session holding the old one, and
/// one deleted is gone.
#[test]
fn a_competition_stored_is_renamed_and_deleted() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    let mut other = Session::new("league");
    now(session.save(&storage)).unwrap();
    now(other.save(&storage)).unwrap();

    now(storage.rename(session.key(), "Cup 2026")).unwrap();
    session.add_entrant("Alice").unwrap();
    now(session.save(&storage)).unwrap();
    now(storage.delete(other.key())).unwrap();

    let listings = now(storage.list()).unwrap();
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].name, "Cup 2026");
    assert_eq!(
        now(Session::load(&storage, session.key())).unwrap().name(),
        "Cup 2026"
    );
    assert!(now(storage.read(other.key())).unwrap().is_none());
}

#[test]
fn a_failed_save_leaves_the_changes_to_the_next_one() {
    let storage = Flaky::default();
    let mut session = Session::new("cup");
    now(session.save(&storage)).unwrap();
    session.add_entrant("Alice").unwrap();

    storage.failing.set(true);
    assert!(now(session.save(&storage)).is_err());
    assert!(session.has_unsaved_changes());
    storage.failing.set(false);
    session.add_entrant("Bob").unwrap();
    now(session.save(&storage)).unwrap();

    let loaded = now(Session::load(&storage, session.key())).unwrap();
    assert_eq!(names(&loaded.entrants()), ["Alice", "Bob"]);
}

/// The log knows the entrants by id only, so that their names can be deleted without touching the
/// log.
#[test]
fn names_are_stored_apart_from_the_log() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    now(session.save(&storage)).unwrap();

    let record = record(&storage, session.key());
    assert!(!record["log"].to_string().contains("Alice"));
    assert!(record["people"].to_string().contains("Alice"));
}

#[test]
fn an_exported_competition_is_imported_as_a_new_one_as_it_was() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    let mut imported = Session::import(&session.export(), "other", &[]).unwrap();

    assert_eq!(imported.name(), "cup", "the file names the competition");
    assert_ne!(imported.key(), session.key());
    assert_eq!(imported.entrants(), session.entrants());
    assert_eq!(matches(&imported), matches(&session));
    assert_eq!(imported.standings(), session.standings());
    assert!(matches!(imported.undo(), Err(UndoError::NothingToUndo)));
    assert!(imported.has_unsaved_changes(), "nothing of it is saved yet");
    now(imported.save(&storage)).unwrap();
    let loaded = now(Session::load(&storage, imported.key())).unwrap();
    assert_eq!(loaded.entrants(), session.entrants());
    assert_eq!(matches(&loaded), matches(&session));
}

/// For a version that can no longer replay its log.
#[test]
fn an_exported_competition_holds_what_its_log_replays_into() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    let file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();

    assert_eq!(
        file["competition"],
        serde_json::to_value(session.competition()).unwrap()
    );
}

/// The log is what is imported: the competition is only read by a version that cannot replay it.
#[test]
fn a_file_is_imported_from_its_log_whatever_its_competition_holds() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();

    for competition in [
        None,
        Some(serde_json::json!({ "of a later version": true })),
    ] {
        match competition {
            None => file.as_object_mut().unwrap().remove("competition"),
            Some(competition) => file
                .as_object_mut()
                .unwrap()
                .insert("competition".to_owned(), competition),
        };

        let imported = Session::import(&file.to_string(), "cup", &[]).unwrap();

        assert_eq!(imported.entrants(), session.entrants());
    }
}

/// The CLI saves the same file without the name.
#[test]
fn a_file_without_a_name_is_imported_under_the_name_given() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    file.as_object_mut().unwrap().remove("name");

    let imported = Session::import(&file.to_string(), "cup.json", &[]).unwrap();

    assert_eq!(imported.name(), "cup.json");
    assert_eq!(imported.entrants(), session.entrants());
}

/// A copy of a competition stored already is told apart from it in the list.
#[test]
fn a_competition_imported_under_a_name_stored_already_is_numbered() {
    let session = Given::new().added();
    let stored = |name: &str| Listing {
        key: CompetitionKey::new(Uuid::new_v4()),
        name: name.to_owned(),
        progress: None,
    };

    let imported =
        Session::import(&session.export(), "", &[stored("cup"), stored("cup (2)")]).unwrap();

    assert_eq!(imported.name(), "cup (3)");
}

/// Files were written without their format before it was added.
#[test]
fn a_file_without_a_format_is_imported_as_of_the_first() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    assert_eq!(
        file["format"],
        serde_json::to_value(file::Format::CURRENT).unwrap()
    );
    file.as_object_mut().unwrap().remove("format");

    let imported = Session::import(&file.to_string(), "cup", &[]).unwrap();

    assert_eq!(imported.entrants(), session.entrants());
}

#[test]
fn a_file_of_a_newer_format_is_not_imported_for_its_format() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    let newer = file["format"].as_u64().unwrap() + 1;
    file["format"] = newer.into();
    // what a newer format might hold instead of a log
    file["log"] = serde_json::json!({ "entries": [] });

    let result = Session::import(&file.to_string(), "cup", &[]);

    assert!(
        matches!(
            result,
            Err(ImportError::Newer(format)) if format.to_string() == newer.to_string()
        ),
        "{:?}",
        result.err()
    );
}

/// A file that is no JSON, or no object with a log and people, is no competition at all.
#[test]
fn a_file_that_is_no_competition_is_not_imported() {
    for file in [
        "",
        "<html>",
        "[1, 2]",
        r#"{"people": []}"#,
        r#"{"format": "x"}"#,
    ] {
        let result = Session::import(file, "cup", &[]);

        assert!(
            matches!(result, Err(ImportError::NoCompetition(_))),
            "{file}: {:?}",
            result.err()
        );
    }
}

/// A file with a log and people that do not load is a competition, but damaged.
#[test]
fn a_damaged_competition_is_not_imported() {
    let session = Given::new().started();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    file["log"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({}));

    for file in [
        file.to_string(),
        r#"{"log": [{}], "people": []}"#.to_owned(),
    ] {
        let result = Session::import(&file, "cup", &[]);

        assert!(
            matches!(result, Err(ImportError::Damaged(_))),
            "{:?}",
            result.err()
        );
    }
}

/// A log written by a version with a plugin this one does not have, say, cannot be loaded, but the
/// operator can still keep a copy of it.
#[test]
fn what_is_stored_is_exported_whether_it_replays_or_not() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    now(session.save(&storage)).unwrap();
    let mut record = record(&storage, session.key());
    record["log"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "unknown": true }));
    storage.set_record(session.key(), record.to_string());
    assert!(now(Session::load(&storage, session.key())).is_err());

    let file = now(storage.read(session.key())).unwrap().unwrap().export();

    let exported: serde_json::Value = serde_json::from_str(&file).unwrap();
    assert_eq!(exported["name"], "cup");
    assert_eq!(exported["log"].as_array().unwrap().len(), 3);
    assert_eq!(exported["log"][2], serde_json::json!({ "unknown": true }));
    assert_eq!(exported["people"][0]["name"], "Alice");
    assert_eq!(
        exported["competition"],
        serde_json::to_value(session.competition()).unwrap(),
        "what the log replayed into in the version that wrote it"
    );
    assert!(Session::import(&file, "cup", &[]).is_err());
}

#[test]
fn what_does_not_replay_into_a_competition_is_not_loaded() {
    let storage = MemoryStorage::default();
    let session = Session::new("cup");
    let key = session.key();
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::NotFound)
    ));
    now(storage.write(&session.to_save())).unwrap();
    let write = |log: serde_json::Value| {
        let mut record = record(&storage, key);
        record["log"] = log;
        storage.set_record(key, record.to_string());
    };

    write(serde_json::json!([{}]));
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::Damaged(_))
    ));
    let seed = serde_json::to_value(&session.machine.log()[0]).unwrap();
    let undo_of_nothing = serde_json::json!({
        "id": Uuid::from_u128(2),
        "kind": { "Undo": Uuid::from_u128(3) },
    });
    write(serde_json::json!([seed, undo_of_nothing]));
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::Replay(_))
    ));
    storage.set_record(key, "no file".to_owned());
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::Damaged(_))
    ));
}
