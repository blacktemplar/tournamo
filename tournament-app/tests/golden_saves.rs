//! Golden saves: competitions as released versions saved them, which every later version has to
//! load and replay into the very competition they were saved as.
//!
//! Each release adds a directory `saves/v<version>/`, written by `cargo test -p tournament-app
//! --test golden_saves -- --ignored write`, and lists its files in [`SAVES`]; a new format or
//! feature that is saved gets a script first. Each save is the file the version
//! exports (`<name>.json`) next to the competition it held, serialized (`<name>.competition.json`).
//! A save is never changed once committed: a test failing on it means that this version would show
//! users' competitions differently than they were saved. If that is intended, the save goes, noted
//! in `CHANGELOG.md`.

use std::{
    num::NonZeroU32,
    pin::pin,
    task::{Context, Poll, Waker},
};

use plugin_factory::{
    PluginType as _,
    config::{Path as ConfigPath, PluginConfig},
};
use serde_json::Value;
use tournament_app::{
    result_form::ResultForm,
    session::Session,
    storage::{MemoryStorage, Storage as _},
};
use tournament_engine::{
    Competition, EnteredValue, FieldKey, MatchResult, Team,
    plugins::{
        match_format::{BestOf, Scored},
        metrics_calculator::{
            HigherPointsDifference, MorePointsScored, MoreWins, PointsBalanceCalculator,
        },
        phase_format::{
            DoubleElimination, MonsterDyp, RandomRounds, RoundRobin, SingleElimination,
        },
    },
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Every golden save, as `(<version>/<name>, file, competition)`.
macro_rules! saves {
    ($($save:literal),* $(,)?) => {
        &[$((
            $save,
            include_str!(concat!("saves/", $save, ".json")),
            include_str!(concat!("saves/", $save, ".competition.json")),
        )),*]
    };
}

const SAVES: &[(&str, &str, &str)] = saves![
    "v0.1.1/setup",
    "v0.1.1/round-robin",
    "v0.1.1/random-rounds",
    "v0.1.1/single-elimination",
    "v0.1.1/double-elimination",
    "v0.1.1/monster-dyp",
    "v0.2.0/setup",
    "v0.2.0/round-robin",
    "v0.2.0/random-rounds",
    "v0.2.0/single-elimination",
    "v0.2.0/double-elimination",
    "v0.2.0/monster-dyp",
    "v0.3.0/setup",
    "v0.3.0/round-robin",
    "v0.3.0/random-rounds",
    "v0.3.0/single-elimination",
    "v0.3.0/double-elimination",
    "v0.3.0/monster-dyp",
];

#[test]
fn every_golden_save_replays_into_the_competition_it_was_saved_as() {
    for (save, file, competition) in SAVES {
        let session = Session::import(file, "", &[])
            .unwrap_or_else(|error| panic!("{save} does not load: {error}"));

        let replayed = serde_json::to_value(session.competition()).unwrap();
        assert!(
            replayed == as_read(competition),
            "{save} replays into another competition than it was saved as"
        );
        let file: Value = serde_json::from_str(file).unwrap();
        let saved: Value = serde_json::from_str(competition).unwrap();
        assert!(
            file.get("competition").is_none_or(|held| *held == saved),
            "{save} holds another competition than it was saved as"
        );
    }
}

/// A saved competition as this version reads it and would save it: where the serialized form of a
/// competition changed, an older one is read by a converter.
fn as_read(competition: &str) -> Value {
    let competition: Competition = serde_json::from_str(competition).unwrap();
    serde_json::to_value(competition).unwrap()
}

/// Saves of versions that did not store the competition get it when they are stored.
#[test]
fn every_golden_save_is_stored_with_the_competition_it_was_saved_as() {
    for (save, file, competition) in SAVES {
        let storage = MemoryStorage::default();
        let mut session = Session::import(file, "", &[]).unwrap();

        now(session.save(&storage)).unwrap();

        let stored = now(storage.read(session.key())).unwrap().unwrap();
        let stored: Value = serde_json::from_str(&stored.record).unwrap();
        assert!(
            stored["competition"] == as_read(competition),
            "{save} is stored with another competition than it was saved as"
        );
    }
}

/// Every screen still shows a golden save.
#[test]
fn every_golden_save_shows() {
    for (save, file, _) in SAVES {
        let session = Session::import(file, "", &[]).unwrap();

        session.entrants();
        session.actions();
        session.progress();
        session.playable();
        session.standings();
        session.winners();
        session.bracket();
        session.cross_table();
        for summary in session.rounds() {
            assert!(
                session.round(summary.round).is_some(),
                "{save} lists round {} but does not show it",
                summary.round
            );
        }
    }
}

/// A save written but not listed in [`SAVES`] would never be checked.
#[test]
#[cfg_attr(target_arch = "wasm32", ignore = "reads the directory of the saves")]
fn every_golden_save_is_listed() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/saves");
    let mut written = Vec::new();
    for version in std::fs::read_dir(&directory).unwrap() {
        let version = version.unwrap();
        for file in std::fs::read_dir(version.path()).unwrap() {
            let file = file.unwrap().file_name().into_string().unwrap();
            if let Some(name) = file.strip_suffix(".json")
                && !name.ends_with(".competition")
            {
                written.push(format!(
                    "{}/{name}",
                    version.file_name().into_string().unwrap()
                ));
            }
        }
    }
    let mut listed: Vec<_> = SAVES.iter().map(|(save, ..)| save.to_string()).collect();
    written.sort();
    listed.sort();

    assert_eq!(written, listed);
}

/// The scripts play until a release writes its saves, so a change that breaks one shows now.
#[test]
fn every_script_plays() {
    for &(name, script) in SCRIPTS {
        script(&mut Session::new(name));
    }
}

/// Writes the golden saves of this version, see the module docs.
#[test]
#[ignore = "writes the golden saves of this version, once before its release"]
fn write_the_golden_saves_of_this_version() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/saves")
        .join(format!("v{}", env!("CARGO_PKG_VERSION")));
    assert!(
        !directory.exists(),
        "{} exists: the saves of a version are never written again",
        directory.display()
    );
    std::fs::create_dir_all(&directory).unwrap();
    for &(name, script) in SCRIPTS {
        let mut session = Session::new(name);
        script(&mut session);
        std::fs::write(directory.join(format!("{name}.json")), session.export()).unwrap();
        std::fs::write(
            directory.join(format!("{name}.competition.json")),
            serde_json::to_string_pretty(session.competition()).unwrap(),
        )
        .unwrap();
    }
}

/// Plays a competition to save.
type Script = fn(&mut Session);

/// The competitions each version saves, by name.
const SCRIPTS: &[(&str, Script)] = &[
    ("setup", setup),
    ("round-robin", round_robin),
    ("random-rounds", random_rounds),
    ("single-elimination", single_elimination),
    ("double-elimination", double_elimination),
    ("monster-dyp", monster_dyp),
    ("scored", scored),
    ("best-of", best_of),
];

/// Entrants renamed and removed, a phase added, undone, added again and edited, not started.
fn setup(session: &mut Session) {
    let alice = enter(session, &["Alice", "Bob", "Carol", "Dave", "Eve"])[0];
    session.rename_entrant(alice, "Alicia").unwrap();
    let eve = session.entrants().last().unwrap().id;
    session.remove_entrant(eve).unwrap();
    session.undo().unwrap();
    session.redo().unwrap();
    add_phase(session, RoundRobin::ID);
    session.undo().unwrap();
    add_phase(session, SingleElimination::ID);
    let mut draft = session.phase_draft_to_edit().unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![PluginConfig::of::<MoreWins>()],
        )
        .unwrap();
    assert!(session.edit_phase(&draft).unwrap());
}

/// An odd field, so with byes, all played with draws, a result reopened and reported anew, an edit
/// of the ranking during play, and an undo and redo.
fn round_robin(session: &mut Session) {
    enter(session, &["Alice", "Bob", "Carol", "Dave", "Eve"]);
    add_phase(session, RoundRobin::ID);
    session.start().unwrap();
    play(
        session,
        &[MatchResult::AWins, MatchResult::Draw, MatchResult::BWins],
        4,
    );
    let mut draft = session.phase_draft_to_edit().unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![PluginConfig::of::<MoreWins>()],
        )
        .unwrap();
    assert!(session.edit_phase(&draft).unwrap());
    play(
        session,
        &[MatchResult::BWins, MatchResult::Draw, MatchResult::AWins],
        usize::MAX,
    );
    let first = session.round(round(1)).unwrap().matches[0].id;
    session.undo_report(first).unwrap();
    session.report(first, MatchResult::Draw).unwrap();
    session.undo().unwrap();
    session.redo().unwrap();
}

/// More rounds added after the first results, some of the matches played.
fn random_rounds(session: &mut Session) {
    enter(session, &["A", "B", "C", "D", "E", "F", "G"]);
    add_phase(session, RandomRounds::ID);
    session.start().unwrap();
    play(session, &[MatchResult::AWins, MatchResult::Draw], 3);
    let mut draft = session.phase_draft_to_edit().unwrap();
    draft
        .set(&ConfigPath::ROOT.field("format").field("rounds"), 7_u32)
        .unwrap();
    assert!(session.edit_phase(&draft).unwrap());
    play(session, &[MatchResult::BWins, MatchResult::AWins], 6);
}

/// A field that is no power of two, so with byes, played to the end.
fn single_elimination(session: &mut Session) {
    enter(session, &["A", "B", "C", "D", "E", "F"]);
    add_phase(session, SingleElimination::ID);
    session.start().unwrap();
    play(
        session,
        &[MatchResult::AWins, MatchResult::BWins],
        usize::MAX,
    );
}

/// Played to the end, with a result of the first round reopened and reported anew.
fn double_elimination(session: &mut Session) {
    enter(session, &["A", "B", "C", "D", "E", "F", "G", "H"]);
    add_phase(session, DoubleElimination::ID);
    session.start().unwrap();
    play(session, &[MatchResult::AWins], 2);
    let first = session.round(round(1)).unwrap().matches[0].id;
    session.undo_report(first).unwrap();
    session.report(first, MatchResult::BWins).unwrap();
    play(
        session,
        &[MatchResult::BWins, MatchResult::AWins, MatchResult::AWins],
        usize::MAX,
    );
}

/// Teams drawn anew every round, partly played.
fn monster_dyp(session: &mut Session) {
    enter(session, &["A", "B", "C", "D", "E", "F", "G", "H", "I"]);
    add_phase(session, MonsterDyp::ID);
    session.start().unwrap();
    play(session, &[MatchResult::AWins, MatchResult::BWins], 5);
}

/// Scores entered, ranked by the points difference, one result chosen against its score, played to
/// the end.
fn scored(session: &mut Session) {
    enter(session, &["Alice", "Bob", "Carol", "Dave"]);
    let mut draft = session.phase_draft(RoundRobin::ID).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![
                PluginConfig::of::<HigherPointsDifference>(),
                PluginConfig::of::<MorePointsScored>(),
            ],
        )
        .unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("metrics"),
            vec![PluginConfig::of::<PointsBalanceCalculator>()],
        )
        .unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("match_format"),
            PluginConfig::of::<Scored>().with("usual_top_score", 10),
        )
        .unwrap();
    session.add_phase(&draft).unwrap();
    session.start().unwrap();
    play_scores(session, &[&[(10, 7)], &[(3, 10)], &[(10, 10)]], 4);
    let open = session.playable()[0].id;
    let mut form = session.result_form(open).unwrap();
    enter_scores(&mut form, &[(2, 10)]);
    form.choose(&[], Some(MatchResult::AWins));
    session.report_form(&form).unwrap();
    play_scores(session, &[&[(12, 10)]], usize::MAX);
}

/// The games of a best of three entered, some matches in two games, a result reopened and entered
/// anew, played to the end.
fn best_of(session: &mut Session) {
    enter(session, &["A", "B", "C", "D", "E", "F", "G", "H"]);
    let mut draft = session.phase_draft(SingleElimination::ID).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("match_format"),
            PluginConfig::of::<BestOf>().with("games", 3),
        )
        .unwrap();
    session.add_phase(&draft.with_defaults().unwrap()).unwrap();
    session.start().unwrap();
    play_scores(
        session,
        &[&[(21, 17), (21, 9)], &[(15, 21), (21, 19), (18, 21)]],
        2,
    );
    let first = session.round(round(1)).unwrap().matches[0].id;
    session.undo_report(first).unwrap();
    play_scores(session, &[&[(21, 23), (21, 15), (21, 19)]], 1);
    play_scores(session, &[&[(21, 11), (30, 29)]], usize::MAX);
}

/// Reports up to `matches` of the playable matches, one at a time, with the scores of `played` in
/// turn, each the scores of a match or of its games.
fn play_scores(session: &mut Session, played: &[&[(u32, u32)]], matches: usize) {
    for scores in played.iter().cycle().take(matches) {
        let Some(row) = session.playable().into_iter().next() else {
            return;
        };
        let mut form = session.result_form(row.id).unwrap();
        enter_scores(&mut form, scores);
        session.report_form(&form).unwrap();
    }
}

/// Enters `scores` into `form`: the match's, if it has a field, or else those of its games.
fn enter_scores(form: &mut ResultForm, scores: &[(u32, u32)]) {
    let root = form.view().root;
    let places: Vec<_> = if root.fields.is_empty() {
        root.parts.iter().map(|part| part.path.clone()).collect()
    } else {
        vec![Vec::new()]
    };
    let score = FieldKey::new("score");
    for (path, (a, b)) in places.iter().zip(scores) {
        form.set_value(path, &score, Team::A, Some(EnteredValue::Count(*a)));
        form.set_value(path, &score, Team::B, Some(EnteredValue::Count(*b)));
    }
}

/// Enters entrants named `names`, returning their ids.
fn enter(session: &mut Session, names: &[&str]) -> Vec<tournament_engine::EntrantId> {
    names
        .iter()
        .map(|name| session.add_entrant(name).unwrap())
        .collect()
}

/// Adds a phase of `format` with the defaults of its form.
fn add_phase(session: &mut Session, format: &str) {
    let draft = session.phase_draft(format).unwrap();
    session.add_phase(&draft).unwrap();
}

/// Reports up to `matches` of the playable matches, one at a time, with `results` in turn.
fn play(session: &mut Session, results: &[MatchResult], matches: usize) {
    for result in results.iter().cycle().take(matches) {
        let Some(row) = session.playable().into_iter().next() else {
            return;
        };
        session.report(row.id, *result).unwrap();
    }
}

fn round(number: u32) -> NonZeroU32 {
    NonZeroU32::new(number).unwrap()
}

/// The output of `future`, which a [`MemoryStorage`] finishes right away.
#[track_caller]
fn now<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("a memory storage never waits"),
    }
}
