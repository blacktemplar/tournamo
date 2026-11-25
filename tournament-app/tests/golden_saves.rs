//! Golden saves: competitions as released versions saved them, whose logs every later version has
//! to replay as they were played: the same matches with the same results, ranked alike. What else
//! the competition holds may differ, e.g. an annotation a format has since added to its matches.
//!
//! Each release adds a directory `saves/v<version>/`, written by `cargo test -p tournament-app
//! --test golden_saves -- --ignored write`, and lists its files in [`SAVES`]; a new format or
//! feature that is saved gets a script first. Each save is the file the version
//! exports (`<name>.json`) next to the competition it held, serialized (`<name>.competition.json`).
//! A save is never changed once committed: a test failing on it means that this version would
//! replay users' competitions differently than they were played or ranked. If that is intended, the
//! save goes, noted in `CHANGELOG.md`.

use std::{
    num::NonZeroU32,
    pin::pin,
    task::{Context, Poll, Waker},
};

use plugin_factory::{
    PluginType as _,
    config::{Path as ConfigPath, PluginConfig},
};
use serde::de::IgnoredAny;
use serde_json::Value;
use tournament_app::{
    file::{self, File},
    form::{Editable as _, End, PhaseForm},
    i18n::{Language, Translator},
    result_form::ResultForm,
    session::Session,
    storage::{MemoryStorage, Storage as _},
    view::{DivisionRef, MatchesOf},
};
use tournament_engine::{
    Competition, EnteredValue, MatchResult, PhaseId, Sides, Table,
    plugins::{
        draw::Blocks,
        match_format::{BestOf, Scored},
        metrics_calculator::{
            GamesBalanceCalculator, HigherGamesDifference, HigherPointsDifference, MetricId as _,
            MoreGamesWon, MorePointsScored, MoreWins, PointsBalanceCalculator, PointsPerMatch,
            WinsDrawsLossesCalculator,
        },
        phase_format::{
            DoubleElimination, MonsterDyp, RandomRounds, RoundRobin, SingleElimination,
        },
        teams::RandomPairs,
        template::{DoubleEliminationTemplate, GroupsAndKnockout, RoundsAndKnockout},
    },
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// What the generator plays: the first phase, the only one of each save.
trait FirstPhase {
    fn first(&self) -> PhaseId;

    fn all(&self) -> MatchesOf {
        MatchesOf::phase(self.first())
    }
}

impl FirstPhase for Session {
    fn first(&self) -> PhaseId {
        *self.competition().phases().keys().next().unwrap()
    }
}

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
    "v0.4.0/setup",
    "v0.4.0/round-robin",
    "v0.4.0/random-rounds",
    "v0.4.0/single-elimination",
    "v0.4.0/double-elimination",
    "v0.4.0/double-elimination-24",
    "v0.4.0/monster-dyp",
    "v0.4.0/scored",
    "v0.4.0/best-of",
    "v0.4.0/games",
    "v0.5.0/setup",
    "v0.5.0/round-robin",
    "v0.5.0/random-rounds",
    "v0.5.0/single-elimination",
    "v0.5.0/double-elimination",
    "v0.5.0/double-elimination-24",
    "v0.5.0/monster-dyp",
    "v0.5.0/scored",
    "v0.5.0/best-of",
    "v0.5.0/games",
    "v0.5.0/groups-then-knockout",
    "v0.5.0/qualification-then-knockouts",
    "v0.5.0/dyp",
    "v0.5.0/phases",
];

#[test]
fn every_golden_save_replays_as_it_was_played() {
    for (save, file, competition) in SAVES {
        let session = Session::import(file, "", &[])
            .unwrap_or_else(|error| panic!("{save} does not load: {error}"));

        let replayed = serde_json::to_value(session.competition()).unwrap();
        assert!(
            played_in(&replayed) == played_in(&as_read(file, competition)),
            "{save} replays into other matches or results than it was played with"
        );
        let saved: Value = serde_json::from_str(competition).unwrap();
        assert!(
            ranks_in(&replayed) == ranks_in(&converted(file, competition)),
            "{save} replays into other ranks than it was saved with"
        );
        let file: Value = serde_json::from_str(file).unwrap();
        assert!(
            file.get("competition").is_none_or(|held| *held == saved),
            "{save} holds another competition than it was saved as"
        );
    }
}

/// The competition saved with `file` as this version reads it and would save it: one of an older
/// format is converted first, as the competition in `file` would be.
fn as_read(file: &str, competition: &str) -> Value {
    let competition: Competition = serde_json::from_value(converted(file, competition)).unwrap();
    serde_json::to_value(competition).unwrap()
}

/// The competition saved with `file` in the layout of the current format, as it was written
/// otherwise: converted, not read.
fn converted(file: &str, competition: &str) -> Value {
    let format = serde_json::from_str::<File<IgnoredAny, IgnoredAny>>(file)
        .unwrap()
        .format;
    let mut competition = serde_json::from_str(competition).unwrap();
    file::upgrade_competition(&mut competition, format);
    competition
}

/// What was played in a serialized competition: per division its matches with their results and
/// its byes, each without the annotations its format attached.
fn played_in(competition: &Value) -> Vec<Value> {
    let without_annotations = |mut entity: Value, annotations: &str| {
        if let Some(Value::Array(attached)) = entity.pointer_mut(annotations) {
            attached.clear();
        }
        entity
    };
    competition["phases"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|phase| phase["divisions"].as_array().unwrap())
        .map(|division| {
            let data = &division["data"];
            let entities = |of: &str, annotations: &'static str| -> Vec<Value> {
                data[of]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entity| without_annotations(entity.clone(), annotations))
                    .collect()
            };
            serde_json::json!([
                entities("matches", "/contest/annotations"),
                entities("byes", "/annotations")
            ])
        })
        .collect()
}

/// What a serialized competition says of the ranks in the divisions of its phases: per division
/// the order of the rankings and the ranks of each.
fn ranks_in(competition: &Value) -> Vec<Value> {
    competition["phases"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|phase| phase["divisions"].as_array().unwrap())
        .map(|division| {
            let data = &division["data"];
            let rankings: Vec<_> = data["rankings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| (&r["id"], &r["rank"], &r["unique_rank"]))
                .collect();
            serde_json::json!([data["ranking_ids_ordered_by_unique_rank"], rankings])
        })
        .collect()
}

/// Saves of versions that did not store the competition get the one their log replays into when
/// they are stored.
#[test]
fn every_golden_save_is_stored_with_the_competition_it_replays_into() {
    for (save, file, _) in SAVES {
        let storage = MemoryStorage::default();
        let mut session = Session::import(file, "", &[]).unwrap();

        now(session.save(&storage)).unwrap();

        let stored = now(storage.read(session.key())).unwrap().unwrap();
        let stored: Value = serde_json::from_str(&stored.record).unwrap();
        assert!(
            stored["competition"] == serde_json::to_value(session.competition()).unwrap(),
            "{save} is stored with another competition than it replays into"
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
        session.ranking();
        for phase in session.phases() {
            let all = MatchesOf::phase(phase.id);
            let only = DivisionRef::first(phase.id);
            session.playable(all);
            session.standings(only);
            session.winners(only);
            session.bracket(only);
            session.cross_table(only);
            for summary in session.rounds(all) {
                assert!(
                    session.round(all, summary.round).is_some(),
                    "{save} lists round {} but does not show it",
                    summary.round
                );
            }
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
    ("double-elimination-24", double_elimination_24),
    ("monster-dyp", monster_dyp),
    ("scored", scored),
    ("best-of", best_of),
    ("games", games),
    ("groups-then-knockout", groups_then_knockout),
    ("qualification-then-knockouts", qualification_then_knockouts),
    ("dyp", dyp),
    ("phases", phases),
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
    let mut draft = session.phase_draft_to_edit(session.first()).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![PluginConfig::of::<MoreWins>()],
        )
        .unwrap();
    assert!(
        session
            .edit_phase(session.first(), &PhaseForm::new(draft))
            .unwrap()
    );
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
    let mut draft = session.phase_draft_to_edit(session.first()).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![PluginConfig::of::<MoreWins>()],
        )
        .unwrap();
    assert!(
        session
            .edit_phase(session.first(), &PhaseForm::new(draft))
            .unwrap()
    );
    play(
        session,
        &[MatchResult::BWins, MatchResult::Draw, MatchResult::AWins],
        usize::MAX,
    );
    let first = session.round(session.all(), round(1)).unwrap().matches[0].id;
    session.undo_report(session.first(), first).unwrap();
    session
        .report(session.first(), first, MatchResult::Draw)
        .unwrap();
    session.undo().unwrap();
    session.redo().unwrap();
}

/// More rounds added after the first results, some of the matches played.
fn random_rounds(session: &mut Session) {
    enter(session, &["A", "B", "C", "D", "E", "F", "G"]);
    add_phase(session, RandomRounds::ID);
    session.start().unwrap();
    play(session, &[MatchResult::AWins, MatchResult::Draw], 3);
    let mut draft = session.phase_draft_to_edit(session.first()).unwrap();
    draft
        .set(&ConfigPath::ROOT.field("format").field("rounds"), 7_u32)
        .unwrap();
    assert!(
        session
            .edit_phase(session.first(), &PhaseForm::new(draft))
            .unwrap()
    );
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
    let first = session.round(session.all(), round(1)).unwrap().matches[0].id;
    session.undo_report(session.first(), first).unwrap();
    session
        .report(session.first(), first, MatchResult::BWins)
        .unwrap();
    play(
        session,
        &[MatchResult::BWins, MatchResult::AWins, MatchResult::AWins],
        usize::MAX,
    );
}

/// 24 entrants, so with byes and every round of the loser bracket, played to the end. Written for
/// v0.4.0 after its release, to keep the loser bracket of its double elimination.
fn double_elimination_24(session: &mut Session) {
    let names: Vec<_> = (1..=24).map(|number| format!("P{number:02}")).collect();
    enter(
        session,
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    add_phase(session, DoubleElimination::ID);
    session.start().unwrap();
    play(
        session,
        &[MatchResult::AWins, MatchResult::BWins, MatchResult::BWins],
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
            PluginConfig::of::<Scored>().with("winning_score", 10),
        )
        .unwrap();
    session.add_phase(&PhaseForm::new(draft)).unwrap();
    session.start().unwrap();
    play_scores(session, &[&[(10, 7)], &[(3, 10)], &[(10, 10)]], 4);
    let open = session.playable(session.all())[0].id;
    let mut form = session.result_form(session.first(), open).unwrap();
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
    session
        .add_phase(&PhaseForm::new(draft.with_defaults().unwrap()))
        .unwrap();
    session.start().unwrap();
    play_scores(
        session,
        &[&[(21, 17), (21, 9)], &[(15, 21), (21, 19), (18, 21)]],
        2,
    );
    let first = session.round(session.all(), round(1)).unwrap().matches[0].id;
    session.undo_report(session.first(), first).unwrap();
    play_scores(session, &[&[(21, 23), (21, 15), (21, 19)]], 1);
    play_scores(session, &[&[(21, 11), (30, 29)]], usize::MAX);
}

/// The games of a best of five entered, ranked by the wins, then the games difference and the
/// games won, played to the end.
fn games(session: &mut Session) {
    enter(session, &["Alice", "Bob", "Carol", "Dave"]);
    let mut draft = session.phase_draft(RoundRobin::ID).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![
                PluginConfig::of::<MoreWins>(),
                PluginConfig::of::<HigherGamesDifference>(),
                PluginConfig::of::<MoreGamesWon>(),
            ],
        )
        .unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("metrics"),
            vec![
                PluginConfig::of::<WinsDrawsLossesCalculator>(),
                PluginConfig::of::<GamesBalanceCalculator>(),
            ],
        )
        .unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("match_format"),
            PluginConfig::of::<BestOf>()
                .with("games", 5)
                .with("winning_score", 11),
        )
        .unwrap();
    session.add_phase(&PhaseForm::new(draft)).unwrap();
    session.start().unwrap();
    play_scores(
        session,
        &[
            &[(11, 3), (11, 5), (11, 7)],
            &[(9, 11), (11, 8), (5, 11), (11, 9), (12, 14)],
            &[(11, 6), (8, 11), (11, 4), (11, 13), (11, 2)],
        ],
        usize::MAX,
    );
}

/// Entrants pasted with ratings, some without, then the template of groups and a knockout, in
/// three groups: played with draws, then the knockout of the best of them, played to the end, and
/// then a result of the groups corrected in place.
fn groups_then_knockout(session: &mut Session) {
    let pasted = session
        .add_entrants(
            "Alice;1800\nBob;1650\nCarol\t1650\nDave;1500\nErin\nFrank;1400\nGrace;1720\n\
             Heidi\nIvan;1300\nJudy;1550\nMallory",
        )
        .unwrap();
    assert!(pasted.refused.is_empty());
    add_template(session, GroupsAndKnockout::ID);
    let [groups, knockout] = phases_of(session);
    edit(session, groups, |form| {
        form.enter(&ConfigPath::ROOT.field("divisions"), "3")
    });
    session.start().unwrap();
    play_phase(
        session,
        groups,
        &[MatchResult::AWins, MatchResult::Draw, MatchResult::BWins],
        usize::MAX,
    );
    session.start_phase(knockout).unwrap();
    play_phase(
        session,
        knockout,
        &[MatchResult::BWins, MatchResult::AWins],
        usize::MAX,
    );
    let first = session
        .round(MatchesOf::phase(groups), round(1))
        .unwrap()
        .matches[0]
        .id;
    let mut form = session.edit_form(groups, first).unwrap();
    form.choose(&[], Some(MatchResult::BWins));
    session.correct_report(&form).unwrap();
    assert_eq!(session.competition().phases()[&groups].divisions().len(), 3);
    assert!(finished(session, knockout));
}

/// The template of rounds and a knockout, the knockout in two blocks: the qualification played to
/// the end, the knockouts partly.
fn qualification_then_knockouts(session: &mut Session) {
    let names: Vec<_> = (1..=12).map(|number| format!("P{number:02}")).collect();
    enter(
        session,
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    add_template(session, RoundsAndKnockout::ID);
    let [qualification, knockouts] = phases_of(session);
    edit(session, knockouts, |form| {
        form.enter(&ConfigPath::ROOT.field("divisions"), "2")?;
        form.choose(&ConfigPath::ROOT.field("draw"), Some(Blocks::ID))
    });
    session.start().unwrap();
    play_phase(
        session,
        qualification,
        &[MatchResult::AWins, MatchResult::BWins, MatchResult::Draw],
        usize::MAX,
    );
    session.start_phase(knockouts).unwrap();
    play_phase(session, knockouts, &[MatchResult::AWins], 5);
}

/// A DYP: the template of a double elimination, played by drawn pairs, played to the end.
fn dyp(session: &mut Session) {
    enter(
        session,
        &["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"],
    );
    add_template(session, DoubleEliminationTemplate::ID);
    let [pairs] = phases_of(session);
    edit(session, pairs, |form| {
        form.choose(&ConfigPath::ROOT.field("teams"), Some(RandomPairs::ID))
    });
    session.start().unwrap();
    play_phase(
        session,
        pairs,
        &[MatchResult::AWins, MatchResult::BWins, MatchResult::BWins],
        usize::MAX,
    );
    assert!(finished(session, pairs));
}

/// Phases set up by hand: a named qualification of the registration's places 3–8 showing a metric
/// it is not ranked by, and a final of its best two and the registration's places 1–2; a third
/// phase added and removed. A result of the qualification edited, the start of the final undone
/// and started again, both played to the end.
fn phases(session: &mut Session) {
    enter(
        session,
        &[
            "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi",
        ],
    );
    session
        .enter_rating(session.entrants()[0].id, "1900")
        .unwrap();
    session
        .enter_rating(session.entrants()[1].id, "1850")
        .unwrap();
    let mut qualification = session.phase_form().unwrap();
    qualification.set_name("Qualification");
    qualification.take_some_places();
    qualification.enter_place(0, End::From, "3");
    qualification.add_metric(PointsPerMatch::ID).unwrap();
    qualification.set_shown(PointsPerMatch::ID, Table::Division, true);
    session.add_phase(&qualification).unwrap();
    let mut final_ = session.phase_form().unwrap();
    final_
        .choose(
            &ConfigPath::ROOT.field("format"),
            Some(SingleElimination::ID),
        )
        .unwrap();
    final_.set_name("Final");
    final_.enter_place(0, End::To, "2");
    final_.add_source();
    final_.choose_source(1, None);
    final_.enter_place(1, End::From, "1");
    final_.enter_place(1, End::To, "2");
    session.add_phase(&final_).unwrap();
    session.add_phase(&session.phase_form().unwrap()).unwrap();
    let removed = session.phases()[2].id;
    session.remove_phase(removed).unwrap();
    session.start().unwrap();
    let [qualification, final_] = phases_of(session);
    play_phase(
        session,
        qualification,
        &[MatchResult::AWins, MatchResult::Draw, MatchResult::BWins],
        usize::MAX,
    );
    let first = session
        .round(MatchesOf::phase(qualification), round(1))
        .unwrap()
        .matches[0]
        .id;
    let mut form = session.edit_form(qualification, first).unwrap();
    form.choose(&[], Some(MatchResult::BWins));
    session.edit_report(&form).unwrap();
    session.start_phase(final_).unwrap();
    session.unstart_phase(final_).unwrap();
    session.start_phase(final_).unwrap();
    play_phase(session, final_, &[MatchResult::AWins], usize::MAX);
    assert!(finished(session, final_));
}

/// Whether every match of the phase `phase` has its result.
fn finished(session: &Session, phase: PhaseId) -> bool {
    session.competition().phases()[&phase].is_finished()
}

/// Adds the phases of the template `id`.
fn add_template(session: &mut Session, id: &str) {
    session
        .add_template(id, &Translator::new(Language::English, &[]))
        .unwrap();
}

/// Edits the phase `phase` by `change` to its form.
fn edit(
    session: &mut Session,
    phase: PhaseId,
    change: impl FnOnce(&mut PhaseForm) -> Result<(), plugin_factory::config::ConfigError>,
) {
    let mut form = session.phase_form_to_edit(phase).unwrap();
    change(&mut form).unwrap();
    assert!(session.edit_phase(phase, &form).unwrap());
}

/// The ids of the `N` phases of the competition, in order.
fn phases_of<const N: usize>(session: &Session) -> [PhaseId; N] {
    let ids: Vec<_> = session.phases().iter().map(|phase| phase.id).collect();
    ids.try_into().unwrap()
}

/// Reports up to `matches` of the playable matches of the phase `phase`, one at a time, with
/// `results` in turn.
fn play_phase(session: &mut Session, phase: PhaseId, results: &[MatchResult], matches: usize) {
    for result in results.iter().cycle().take(matches) {
        let Some(row) = session.playable(MatchesOf::phase(phase)).into_iter().next() else {
            return;
        };
        session.report(phase, row.id, *result).unwrap();
    }
}

/// Reports up to `matches` of the playable matches, one at a time, with the scores of `played` in
/// turn, each the scores of a match or of its games.
fn play_scores(session: &mut Session, played: &[&[(u16, u16)]], matches: usize) {
    for scores in played.iter().cycle().take(matches) {
        let Some(row) = session.playable(session.all()).into_iter().next() else {
            return;
        };
        let mut form = session.result_form(session.first(), row.id).unwrap();
        enter_scores(&mut form, scores);
        session.report_form(&form).unwrap();
    }
}

/// Enters `scores` into `form`: the match's, if it has a field, or else those of its games.
fn enter_scores(form: &mut ResultForm, scores: &[(u16, u16)]) {
    let scores: Vec<_> = scores
        .iter()
        .map(|&(a, b)| Sides::new(Some(EnteredValue::Count(a)), Some(EnteredValue::Count(b))))
        .collect();
    form.enter_scores(&scores).unwrap();
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
    session.add_phase(&PhaseForm::new(draft)).unwrap();
}

/// Reports up to `matches` of the playable matches, one at a time, with `results` in turn.
fn play(session: &mut Session, results: &[MatchResult], matches: usize) {
    for result in results.iter().cycle().take(matches) {
        let Some(row) = session.playable(session.all()).into_iter().next() else {
            return;
        };
        session.report(session.first(), row.id, *result).unwrap();
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
