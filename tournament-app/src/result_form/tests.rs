use plugin_factory::{PluginType as _, config::PluginConfig};
use tournament_engine::{
    Competition, Match,
    plugins::{
        match_format::{BestOf, Hints, Scored, WinnerOnly},
        phase_format::RoundRobin,
    },
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{error::ReportError, session::Session};

const SCORE: FieldKey = FieldKey::new("score");

/// A started round robin of Alice and Bob whose results are entered by `results`, with its match.
fn started(results: PluginConfig) -> (Session, MatchId) {
    let mut session = Session::new("cup");
    for name in ["Alice", "Bob"] {
        session.add_entrant(name).unwrap();
    }
    let draft = session
        .phase_draft(RoundRobin::ID)
        .unwrap()
        .with("match_format", results)
        .and_then(|draft| draft.with_defaults())
        .unwrap();
    session.add_phase(&draft).unwrap();
    session.start().unwrap();
    let id = the_match(session.competition()).id();
    (session, id)
}

fn the_match(competition: &Competition) -> &Match {
    competition
        .phases()
        .values()
        .next()
        .unwrap()
        .data()
        .matches()
        .values()
        .next()
        .unwrap()
}

fn count(count: u32) -> Option<EnteredValue> {
    Some(EnteredValue::Count(count))
}

/// Enters `a` to `b` into the score of the part at `path`.
fn score(form: &mut ResultForm, path: &[usize], a: u32, b: u32) {
    form.set_value(path, &SCORE, Team::A, count(a));
    form.set_value(path, &SCORE, Team::B, count(b));
}

#[test]
fn a_score_suggests_its_winner_and_is_reported_with_it() {
    let (mut session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    assert_eq!(form.result(), None);

    score(&mut form, &[], 17, 21);

    assert_eq!(form.result(), Some(MatchResult::BWins));
    session.report_form(&form).unwrap();
    let contest = the_match(session.competition()).contest();
    assert_eq!(contest.decision(), Some(&Decision::new(MatchResult::BWins)));
    assert_eq!(
        contest.value("score"),
        Some(&Sides::new(count(17), count(21)))
    );
}

#[test]
fn a_result_chosen_against_the_score_is_taken_with_a_warning() {
    let (mut session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[], 17, 21);

    form.choose(&[], Some(MatchResult::AWins));

    let root = form.view().root;
    assert_eq!(
        (root.result, root.suggested, root.chosen),
        (Some(MatchResult::AWins), Some(MatchResult::BWins), true)
    );
    assert_eq!(root.warnings, [Text::new("warning-result-disagrees")]);
    session.report_form(&form).unwrap();
    assert_eq!(
        the_match(session.competition()).result(),
        Some(MatchResult::AWins)
    );
}

#[test]
fn following_the_suggestions_again_drops_every_result_chosen() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 17);
    form.choose(&[0], Some(MatchResult::BWins));
    form.choose(&[], Some(MatchResult::BWins));
    assert!(form.is_chosen());

    form.follow_suggestions();

    assert!(!form.is_chosen());
    let root = form.view().root;
    assert_eq!(root.parts[0].result, Some(MatchResult::AWins));
    assert_eq!(root.result, None);
}

#[test]
fn only_a_part_without_values_or_parts_needs_a_choice() {
    let (session, id) = started(PluginConfig::of::<WinnerOnly>());
    assert!(session.result_form(id).unwrap().view().root.needs_choice);

    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let root = session.result_form(id).unwrap().view().root;
    assert!(!root.needs_choice, "its games suggest a result");
    assert!(!root.parts[0].needs_choice, "its score suggests a result");
}

/// The warnings of the score `a` to `b` in a match to 21.
fn warnings_of(a: u32, b: u32) -> Vec<Text> {
    let (session, id) = started(PluginConfig::of::<Scored>().with("winning_score", 21));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[], a, b);
    form.view().root.fields[0].warnings.clone()
}

#[test]
fn a_score_a_winner_usually_has_is_taken_without_a_warning() {
    for (a, b) in [(21, 0), (19, 21), (22, 20), (30, 29)] {
        assert_eq!(warnings_of(a, b), [], "{a}–{b}");
    }
}

#[test]
fn a_score_where_neither_side_has_the_winning_score_is_taken_with_a_warning() {
    let (session, id) = started(PluginConfig::of::<Scored>().with("winning_score", 21));
    let mut form = session.result_form(id).unwrap();

    score(&mut form, &[], 12, 19);

    assert_eq!(
        form.view().root.fields[0].warnings,
        [Text::new("warning-below-winning-score").with("winning", 21_u32)]
    );
    assert_eq!(form.result(), Some(MatchResult::BWins));
}

#[test]
fn a_score_from_the_winning_score_on_without_a_lead_of_one_or_two_is_taken_with_a_warning() {
    for (a, b) in [(23, 19), (21, 21), (25, 25), (24, 21)] {
        assert_eq!(
            warnings_of(a, b),
            [Text::new("warning-winning-lead").with("winning", 21_u32)],
            "{a}–{b}"
        );
    }
}

#[test]
fn a_score_of_one_side_alone_is_not_warned_of() {
    let (session, id) = started(PluginConfig::of::<Scored>().with("winning_score", 21));
    let mut form = session.result_form(id).unwrap();

    form.set_value(&[], &SCORE, Team::A, count(30));

    assert_eq!(form.view().root.fields[0].warnings, []);
}

#[test]
fn a_best_of_shows_every_game_and_reports_those_played() {
    let (mut session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    let games = form.view().root.parts;
    assert_eq!(games.len(), 3);
    assert_eq!(games[2].path, [2]);

    score(&mut form, &[0], 21, 17);
    score(&mut form, &[1], 21, 9);

    let root = form.view().root;
    assert_eq!(root.parts[0].result, Some(MatchResult::AWins));
    assert_eq!(root.result, Some(MatchResult::AWins), "two games of three");
    session.report_form(&form).unwrap();
    assert_eq!(the_match(session.competition()).contest().parts().len(), 2);
}

#[test]
fn a_result_neither_entered_nor_suggested_is_not_reported() {
    let (mut session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 17);

    assert!(matches!(
        session.report_form(&form),
        Err(ReportError::Undecided)
    ));
}

#[test]
fn a_winner_alone_is_chosen() {
    let (mut session, id) = started(PluginConfig::of::<WinnerOnly>());
    let mut form = session.result_form(id).unwrap();
    assert!(form.view().root.fields.is_empty());

    form.choose(&[], Some(MatchResult::AWins));

    session.report_form(&form).unwrap();
    assert!(matches!(
        session.result_form(id),
        Err(ReportError::AlreadyFinished)
    ));
}

#[test]
fn a_value_cleared_on_both_sides_is_no_longer_entered() {
    let (session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[], 1, 0);

    form.set_value(&[], &SCORE, Team::A, None);
    form.set_value(&[], &SCORE, Team::B, None);

    assert_eq!(form.report().1, Values::new());
}

#[test]
fn a_match_shows_its_score_or_else_those_of_its_games() {
    let (mut session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[], 3, 1);
    let row = session.report_form(&form).unwrap().row;
    assert_eq!(row.scores, [Sides::new(count(3), count(1))]);
    assert!(row.details);

    let (mut session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 17);
    score(&mut form, &[1], 19, 21);
    score(&mut form, &[2], 21, 15);
    assert_eq!(
        session.report_form(&form).unwrap().row.scores,
        [
            Sides::new(count(21), count(17)),
            Sides::new(count(19), count(21)),
            Sides::new(count(21), count(15)),
        ]
    );
}

#[test]
fn a_match_with_a_winner_alone_shows_no_score() {
    let (mut session, id) = started(PluginConfig::of::<WinnerOnly>());

    let reported = session.report(id, MatchResult::AWins).unwrap();

    assert!(reported.row.scores.is_empty());
    assert!(
        !reported.row.details,
        "the winner is entered by the row itself"
    );
}

#[test]
fn a_score_is_entered_as_its_field_takes_it() {
    let half = |number| EnteredValue::Decimal(Score::new(number));

    assert_eq!(
        of_kind(EnteredValue::Count(3), ValueKind::Decimal),
        Some(half(3.0))
    );
    assert_eq!(
        of_kind(half(3.0), ValueKind::Count),
        Some(EnteredValue::Count(3))
    );
    assert_eq!(of_kind(half(2.5), ValueKind::Count), None);
    assert_eq!(of_kind(half(-1.0), ValueKind::Count), None);
    assert_eq!(of_kind(half(2.5), ValueKind::Decimal), Some(half(2.5)));
}

#[test]
fn a_score_is_entered_in_one_row() {
    let (session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[], 3, 1);

    let quick = form.quick_entry().unwrap();

    assert_eq!(quick.fields.len(), 1);
    assert_eq!(quick.fields[0].path, Vec::<usize>::new());
    assert_eq!(quick.fields[0].part, None);
    assert_eq!(quick.fields[0].field.values, Sides::new(count(3), count(1)));
    assert_eq!(quick.result, Some(MatchResult::AWins));
}

#[test]
fn the_games_of_a_best_of_are_entered_in_one_row() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 5));
    let mut form = session.result_form(id).unwrap();
    for game in 0..3 {
        score(&mut form, &[game], 21, 17);
    }

    let quick = form.quick_entry().unwrap();

    let parts: Vec<_> = quick
        .fields
        .iter()
        .map(|field| (field.path.clone(), field.part.as_ref().unwrap().1))
        .collect();
    assert_eq!(
        parts,
        [
            (vec![0], 1),
            (vec![1], 2),
            (vec![2], 3),
            (vec![3], 4),
            (vec![4], 5)
        ]
    );
    assert_eq!(
        quick.result,
        Some(MatchResult::AWins),
        "three games of five"
    );
}

#[test]
fn a_warning_of_several_fields_in_one_row_is_said_once() {
    let (session, id) = started(
        PluginConfig::of::<BestOf>()
            .with("games", 3)
            .with("winning_score", 21),
    );
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 11, 7);
    score(&mut form, &[1], 11, 9);

    let warnings = form.quick_entry().unwrap().warnings;

    assert_eq!(
        warnings,
        [Text::new("warning-below-winning-score").with("winning", 21_u32)]
    );
}

#[test]
fn a_winner_alone_is_not_entered_in_a_row() {
    let (session, id) = started(PluginConfig::of::<WinnerOnly>());

    assert_eq!(session.result_form(id).unwrap().quick_entry(), None);
}

/// A form of a match whose result looks like `root`.
fn form_of(root: Level) -> ResultForm {
    let (_, id) = started(PluginConfig::of::<WinnerOnly>());
    ResultForm::new(id, Box::new(WinnerOnly), Template { root })
}

/// A level called `name` with a score.
fn scored(name: &'static str) -> Level {
    Level {
        fields: vec![FieldSpec {
            key: SCORE,
            label: Phrase::new("score", "score"),
            kind: ValueKind::Count,
            hints: Hints::default(),
        }],
        ..Level::new(Phrase::new(name, name))
    }
}

#[test]
fn a_match_of_parts_of_several_levels_is_not_entered_in_a_row() {
    let fixture = Level {
        parts: Parts::Each(vec![scored("singles"), scored("doubles")]),
        ..Level::new(Phrase::new("fixture", "fixture"))
    };

    assert_eq!(form_of(fixture).quick_entry(), None);
}

#[test]
fn a_match_of_parts_with_parts_of_their_own_is_not_entered_in_a_row() {
    let set = Level {
        parts: Parts::Same {
            level: Box::new(scored("game")),
            max: 6,
        },
        ..scored("set")
    };
    let r#match = Level {
        parts: Parts::Same {
            level: Box::new(set),
            max: 3,
        },
        ..Level::new(Phrase::new("match", "match"))
    };

    assert_eq!(form_of(r#match).quick_entry(), None);
}

#[test]
fn a_match_with_a_field_of_its_own_and_parts_is_not_entered_in_a_row() {
    let r#match = Level {
        parts: Parts::Same {
            level: Box::new(scored("game")),
            max: 3,
        },
        ..scored("match")
    };

    assert_eq!(form_of(r#match).quick_entry(), None);
}
