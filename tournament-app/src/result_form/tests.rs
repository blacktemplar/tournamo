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
use crate::{
    error::ReportError,
    i18n::{Language, Translator},
    session::Session,
};

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

fn count(count: u16) -> Option<EnteredValue> {
    Some(EnteredValue::Count(count))
}

/// Enters `a` to `b` into the score of the part at `path`.
fn score(form: &mut ResultForm, path: &[usize], a: u16, b: u16) {
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
fn warnings_of(a: u16, b: u16) -> Vec<Text> {
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
fn a_best_of_shows_the_games_that_may_decide_it_and_reports_those_played() {
    let (mut session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    let games = form.view().root.parts;
    assert_eq!(games.len(), 2);
    assert_eq!(games[1].path, [1]);

    score(&mut form, &[0], 21, 17);
    score(&mut form, &[1], 21, 9);

    let root = form.view().root;
    assert_eq!(root.parts[0].result, Some(MatchResult::AWins));
    assert_eq!(root.result, Some(MatchResult::AWins), "two games of three");
    assert_eq!(root.parts.len(), 2, "no third game");
    session.report_form(&form).unwrap();
    assert_eq!(the_match(session.competition()).contest().parts().len(), 2);
}

/// The number of games the form of a best of five shows once each of `games` is entered.
fn games_shown(games: &[(u16, u16)]) -> usize {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 5));
    let mut form = session.result_form(id).unwrap();
    for (index, &(a, b)) in games.iter().enumerate() {
        score(&mut form, &[index], a, b);
    }
    form.view().root.parts.len()
}

#[test]
fn a_game_is_shown_once_every_game_before_it_is_decided_but_the_match_is_not() {
    assert_eq!(games_shown(&[]), 3);
    assert_eq!(games_shown(&[(21, 3), (21, 3), (21, 3)]), 3, "3–0");
    assert_eq!(games_shown(&[(21, 3), (21, 3), (3, 21)]), 4, "2–1");
    assert_eq!(games_shown(&[(21, 3), (21, 3), (3, 21), (3, 21)]), 5, "2–2");
}

#[test]
fn a_game_entered_on_one_side_alone_shows_no_further_game() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 5));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 3);
    score(&mut form, &[1], 3, 21);
    form.set_value(&[2], &SCORE, Team::A, count(21));

    assert_eq!(form.view().root.parts.len(), 3);
}

#[test]
fn a_match_decided_by_hand_shows_no_further_game() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 3);
    score(&mut form, &[1], 3, 21);

    form.choose(&[], Some(MatchResult::AWins));

    assert_eq!(form.view().root.parts.len(), 2);
}

#[test]
fn a_game_entered_after_the_match_was_decided_stays_with_a_warning() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 5));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 3);
    score(&mut form, &[1], 21, 3);
    score(&mut form, &[2], 3, 21);
    score(&mut form, &[3], 21, 3);
    assert_eq!(form.view().root.warnings, []);

    score(&mut form, &[2], 21, 3);

    let root = form.view().root;
    assert_eq!(root.parts.len(), 4, "the fourth game stays");
    let warning = Text::new("warning-more-than-decided-at")
        .with("a", 3_u32)
        .with("b", 0_u32);
    assert_eq!(root.warnings, std::slice::from_ref(&warning));
    assert_eq!(form.quick_entry().unwrap().warnings, [warning]);
}

#[test]
fn the_games_each_side_won_are_counted_once_one_is_decided() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 5));
    let mut form = session.result_form(id).unwrap();
    assert_eq!(form.view().root.parts_won, None);

    score(&mut form, &[0], 21, 3);
    score(&mut form, &[1], 3, 21);
    score(&mut form, &[2], 21, 3);

    assert_eq!(form.view().root.parts_won, Some(Sides::new(2, 1)));
    assert_eq!(
        form.quick_entry().unwrap().parts_won,
        Some(Sides::new(2, 1))
    );
    assert_eq!(
        form.view().root.parts[0].parts_won,
        None,
        "a game is not played in parts"
    );
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
    assert_eq!(row.parts_won, None, "a score is no games");

    let (mut session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 17);
    score(&mut form, &[1], 19, 21);
    score(&mut form, &[2], 21, 15);
    let row = session.report_form(&form).unwrap().row;
    assert_eq!(
        row.scores,
        [
            Sides::new(count(21), count(17)),
            Sides::new(count(19), count(21)),
            Sides::new(count(21), count(15)),
        ]
    );
    assert_eq!(row.parts_won, Some(Sides::new(2, 1)), "with the games won");
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
    assert_eq!(of_kind(half(65535.0), ValueKind::Count), count(u16::MAX));
    assert_eq!(of_kind(half(65536.0), ValueKind::Count), None);
    assert_eq!(of_kind(half(2.5), ValueKind::Decimal), Some(half(2.5)));
}

/// `text` in English, without the marks isolating its variables.
fn english(text: &Text) -> String {
    Translator::new(Language::English, &[])
        .text(text)
        .replace(['\u{2068}', '\u{2069}'], "")
}

/// The field of the score of a match once `typed` is typed for the first side.
fn after_typing(typed: &str) -> FieldView {
    let (session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    form.enter(&[], &SCORE, Team::A, typed);
    form.view().root.fields.remove(0)
}

#[test]
fn a_whole_number_typed_is_entered_as_a_count() {
    for (typed, value) in [
        ("17", 17),
        (" 3 ", 3),
        ("2.0", 2),
        ("1e3", 1000),
        ("65535", u16::MAX),
    ] {
        let field = after_typing(typed);

        assert_eq!(field.values.a, count(value), "{typed}");
        assert_eq!(field.entered.a, None, "{typed}");
        assert!(field.issues.is_empty(), "{typed}");
    }
}

#[test]
fn what_is_typed_as_no_count_stays_shown_with_why() {
    let range = "this takes a whole number from 0 to 65535";
    for (typed, why) in [
        ("65536", range),
        ("4294967295", range),
        ("-1", range),
        ("2.5", range),
        ("many", "“many” is no number"),
    ] {
        let field = after_typing(typed);

        assert_eq!(field.values.a, None, "{typed}");
        assert_eq!(field.entered.a.as_deref(), Some(typed));
        let issues: Vec<_> = field.issues.iter().map(english).collect();
        assert_eq!(issues, [why], "{typed}");
    }
}

#[test]
fn a_value_typed_or_cleared_replaces_what_was_not_taken() {
    for (typed, value) in [("7", count(7)), ("", None)] {
        let (session, id) = started(PluginConfig::of::<Scored>());
        let mut form = session.result_form(id).unwrap();
        form.enter(&[], &SCORE, Team::A, "70000");

        form.enter(&[], &SCORE, Team::A, typed);

        let field = &form.view().root.fields[0];
        assert_eq!(field.values.a, value, "{typed:?}");
        assert_eq!(field.entered.a, None, "{typed:?}");
        assert!(field.issues.is_empty(), "{typed:?}");
    }
}

#[test]
fn why_values_in_one_row_are_not_taken_is_said_once() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    form.enter(&[0], &SCORE, Team::A, "70000");
    form.enter(&[1], &SCORE, Team::B, "80000");

    let quick = form.quick_entry().unwrap();

    let entered: Vec<_> = quick
        .fields
        .iter()
        .map(|quick| quick.field.entered.clone())
        .collect();
    assert_eq!(
        entered,
        [
            Sides::new(Some("70000".to_owned()), None),
            Sides::new(None, Some("80000".to_owned())),
        ]
    );
    let issues: Vec<_> = quick.issues.iter().map(english).collect();
    assert_eq!(issues, ["this takes a whole number from 0 to 65535"]);
}

#[test]
fn a_game_holding_only_what_was_not_taken_stays_shown() {
    let (session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 17);
    score(&mut form, &[1], 21, 15);
    form.enter(&[2], &SCORE, Team::A, "7");

    form.enter(&[2], &SCORE, Team::A, "70000");

    assert_eq!(form.view().root.parts.len(), 3);
}

#[test]
fn a_match_with_a_value_not_taken_is_not_reported() {
    let (mut session, id) = started(PluginConfig::of::<BestOf>().with("games", 3));
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 17);
    score(&mut form, &[1], 21, 15);
    form.enter(&[2], &SCORE, Team::A, "70000");
    assert_eq!(
        form.result(),
        Some(MatchResult::AWins),
        "the games before decide it"
    );

    assert!(matches!(
        session.report_form(&form),
        Err(ReportError::NotTaken)
    ));
    assert_eq!(the_match(session.competition()).result(), None);
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
    assert_eq!(parts, [(vec![0], 1), (vec![1], 2), (vec![2], 3)]);
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
            min: 6,
            max: 6,
        },
        ..scored("set")
    };
    let r#match = Level {
        parts: Parts::Same {
            level: Box::new(set),
            min: 2,
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
            min: 2,
            max: 3,
        },
        ..scored("match")
    };

    assert_eq!(form_of(r#match).quick_entry(), None);
}

#[test]
fn a_finished_match_shows_its_result_as_reported() {
    let (mut session, id) = started(PluginConfig::of::<BestOf>().with("games", 5));
    assert_eq!(session.reported_result(id), None, "not finished yet");
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[0], 21, 17);
    score(&mut form, &[1], 15, 21);
    score(&mut form, &[2], 21, 9);
    score(&mut form, &[3], 21, 19);
    session.report_form(&form).unwrap();

    let root = session.reported_result(id).unwrap().root;

    assert_eq!(root.parts.len(), 4);
    assert_eq!(
        root.parts[1].fields[0].values,
        Sides::new(count(15), count(21))
    );
    assert_eq!(root.parts[1].result, Some(MatchResult::BWins));
    assert_eq!(root.result, Some(MatchResult::AWins));
    assert_eq!(root.parts_won, Some(Sides::new(3, 1)));
    assert!(!root.chosen, "the games decide the match");
    assert_eq!(root.warnings, []);
}

#[test]
fn a_result_reported_against_the_score_shows_as_chosen() {
    let (mut session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    score(&mut form, &[], 3, 1);
    form.choose(&[], Some(MatchResult::BWins));
    session.report_form(&form).unwrap();

    let root = session.reported_result(id).unwrap().root;

    assert_eq!(root.result, Some(MatchResult::BWins));
    assert!(root.chosen);
    assert_eq!(root.warnings, [Text::new("warning-result-disagrees")]);
}

/// What `complete_score` does after entering `entered` for the first side of the score of a match
/// to 21, with `other` already entered for the second: whether it completed it, and the score.
fn completed(entered: u16, other: Option<EnteredValue>) -> (bool, Sides<Option<EnteredValue>>) {
    let (session, id) = started(PluginConfig::of::<Scored>().with("winning_score", 21));
    let mut form = session.result_form(id).unwrap();
    form.set_value(&[], &SCORE, Team::B, other);
    form.set_value(&[], &SCORE, Team::A, count(entered));

    let completed = form.complete_score(&[], &SCORE, Team::A);

    (completed, form.view().root.fields[0].values)
}

#[test]
fn a_score_two_or_more_below_the_winning_score_completes_the_other_side_as_winning() {
    for entered in [0, 15, 19] {
        assert_eq!(
            completed(entered, None),
            (true, Sides::new(count(entered), count(21))),
            "{entered}"
        );
    }
}

#[test]
fn a_score_from_one_below_the_winning_score_on_completes_nothing() {
    for entered in [20, 21, 25, u16::MAX] {
        assert_eq!(
            completed(entered, None),
            (false, Sides::new(count(entered), None)),
            "{entered}"
        );
    }
}

#[test]
fn a_score_entered_on_both_sides_is_not_completed() {
    assert_eq!(
        completed(15, count(17)),
        (false, Sides::new(count(15), count(17)))
    );
}

#[test]
fn a_score_without_a_winning_score_is_not_completed() {
    let (session, id) = started(PluginConfig::of::<Scored>());
    let mut form = session.result_form(id).unwrap();
    form.set_value(&[], &SCORE, Team::B, count(3));

    assert!(!form.complete_score(&[], &SCORE, Team::B));
    assert_eq!(
        form.view().root.fields[0].values,
        Sides::new(None, count(3))
    );
}

#[test]
fn what_was_typed_and_not_taken_completes_nothing() {
    let (session, id) = started(PluginConfig::of::<Scored>().with("winning_score", 21));
    let mut form = session.result_form(id).unwrap();
    form.enter(&[], &SCORE, Team::A, "4294967295");

    assert!(!form.complete_score(&[], &SCORE, Team::A));
    assert_eq!(form.view().root.fields[0].values, Sides::new(None, None));
}

#[test]
fn no_score_is_completed_onto_what_was_typed_and_not_taken() {
    let (session, id) = started(PluginConfig::of::<Scored>().with("winning_score", 21));
    let mut form = session.result_form(id).unwrap();
    form.enter(&[], &SCORE, Team::B, "70000");
    form.enter(&[], &SCORE, Team::A, "15");

    assert!(!form.complete_score(&[], &SCORE, Team::A));
    let field = &form.view().root.fields[0];
    assert_eq!(field.values, Sides::new(count(15), None));
    assert_eq!(field.entered.b.as_deref(), Some("70000"));
}

#[test]
fn the_score_of_a_game_is_completed_in_its_game() {
    let (session, id) = started(
        PluginConfig::of::<BestOf>()
            .with("games", 3)
            .with("winning_score", 11),
    );
    let mut form = session.result_form(id).unwrap();
    form.set_value(&[1], &SCORE, Team::B, count(4));

    assert!(form.complete_score(&[1], &SCORE, Team::B));
    let games = form.view().root.parts;
    assert_eq!(games[0].fields[0].values, Sides::new(None, None));
    assert_eq!(games[1].fields[0].values, Sides::new(count(11), count(4)));
}
