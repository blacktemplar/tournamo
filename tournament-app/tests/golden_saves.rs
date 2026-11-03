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

use std::num::NonZeroU32;

use plugin_factory::{
    PluginType as _,
    config::{Path as ConfigPath, PluginConfig},
};
use serde_json::Value;
use tournament_app::session::Session;
use tournament_engine::{
    MatchResult,
    plugins::{
        metrics_calculator::MoreWins,
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
];

#[test]
fn every_golden_save_replays_into_the_competition_it_was_saved_as() {
    for (save, file, competition) in SAVES {
        let session = Session::import(file, "", &[])
            .unwrap_or_else(|error| panic!("{save} does not load: {error}"));

        let replayed = serde_json::to_value(session.competition()).unwrap();
        let saved: Value = serde_json::from_str(competition).unwrap();
        assert!(
            replayed == saved,
            "{save} replays into another competition than it was saved as"
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
