//! The metrics of a phase: those the operator added and those something needs, with who
//! calculates them, see [`PhaseForm::add_metric`].

use tournament_engine::plugins::{
    match_format::BestOf,
    metrics_calculator::{
        Buchholz, GamesBalance, GamesDifference, GamesDifferenceCalculator, HigherGamesDifference,
        MetricId, Points, WinsDrawsLosses,
    },
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

/// The ids of the metrics of the form, in the order of its rows.
fn metrics(form: &PhaseForm) -> Vec<&'static str> {
    list(&form.view(), "metrics")
        .metrics
        .iter()
        .map(|row| row.metric.id)
        .collect()
}

/// The row of the metric `id`.
#[track_caller]
fn row(form: &PhaseForm, id: &str) -> MetricRow {
    list(&form.view(), "metrics")
        .metrics
        .iter()
        .find(|row| row.metric.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("{id} is not in the list of metrics"))
}

/// A form of a phase ranked by nothing and calculating nothing, its results scored.
fn bare_form() -> PhaseForm {
    let mut form = new_form();
    while !items(&form.view(), "ranking").is_empty() {
        form.remove(&path("ranking[0]")).unwrap();
    }
    form.choose(&path("match_format"), Some(Scored::ID))
        .unwrap();
    form
}

/// Games won and lost need a match format of games; the issue is where that is chosen, and says
/// what needs them.
#[test]
fn a_metric_needed_in_turn_says_what_it_is_needed_for_where_it_is_fixed() {
    let mut form = bare_form();

    form.add_metric(GamesDifference::ID).unwrap();

    let view = form.view();
    assert_eq!(metrics(&form), [GamesDifference::ID, GamesBalance::ID]);
    assert_eq!(
        english_all(&field(&view, "match_format").issues),
        ["games difference needs games won and lost, which needs one of these: best of games"]
    );
    assert!(field(&view, "metrics").issues.is_empty());
    let needed = row(&form, GamesBalance::ID);
    assert!(!needed.added);
    assert_eq!(needed.needed_by[0].id, GamesDifferenceCalculator::ID);

    form.choose(&path("match_format"), Some(BestOf::ID))
        .unwrap();
    assert!(form.view().complete, "{:?}", form.view().issues);
}

#[test]
fn a_criterion_needing_a_metric_in_turn_is_named_by_the_issue() {
    let mut form = bare_form();

    form.add(&path("ranking"), HigherGamesDifference::ID)
        .unwrap();

    assert_eq!(metrics(&form), [GamesDifference::ID, GamesBalance::ID]);
    assert_eq!(
        english_all(&field(&form.view(), "match_format").issues),
        [
            "higher games difference needs games won and lost, which needs one of these: best of \
             games"
        ]
    );
}

/// Removing what needs a metric removes the metric, and what it needed in turn.
#[test]
fn a_metric_only_needed_goes_with_what_needed_it() {
    let mut form = bare_form();
    form.add_metric(GamesDifference::ID).unwrap();

    form.remove_metric(GamesDifference::ID).unwrap();

    assert!(metrics(&form).is_empty());
    let view = form.view();
    assert!(view.complete, "{:?}", view.issues);
    assert!(field(&view, "match_format").issues.is_empty());
}

#[test]
fn a_metric_needed_cannot_be_removed() {
    let mut form = bare_form();
    form.add(&path("ranking"), HigherGamesDifference::ID)
        .unwrap();

    assert!(form.remove_metric(GamesDifference::ID).is_err());
    assert!(form.remove_metric(GamesBalance::ID).is_err());
    assert_eq!(metrics(&form), [GamesDifference::ID, GamesBalance::ID]);
}

/// Hiding a metric in every table keeps it: the checkboxes say where it is shown, not whether
/// it is calculated.
#[test]
fn a_metric_hidden_everywhere_stays_and_does_nothing() {
    let mut form = bare_form();
    form.add_metric(Points::ID).unwrap();
    assert!(row(&form, Points::ID).of(Table::Division).on);

    form.set_shown(Points::ID, Table::Division, false);

    let points = row(&form, Points::ID);
    assert!(!points.of(Table::Division).on);
    assert!(points.unused);
    assert_eq!(metrics(&form), [Points::ID, WinsDrawsLosses::ID]);
}

/// The points are needed by the Buchholz and have settings, so they are added as they offer; what
/// was entered for them comes back with them.
#[test]
fn a_metric_needed_with_settings_comes_back_as_it_was() {
    let mut form = bare_form();
    form.add(&path("ranking"), MoreBuchholz::ID).unwrap();
    assert_eq!(
        metrics(&form),
        [Buchholz::ID, Points::ID, WinsDrawsLosses::ID]
    );
    let points = row(&form, Points::ID).item.unwrap().path;
    form.enter(&points.clone().field("points_for_win"), "3")
        .unwrap();

    form.remove(&path("ranking[0]")).unwrap();
    assert!(metrics(&form).is_empty());
    form.add(&path("ranking"), MoreBuchholz::ID).unwrap();

    let points = row(&form, Points::ID).item.unwrap();
    let win = points
        .fields
        .iter()
        .find(|field| field.name == "points_for_win")
        .unwrap();
    assert!(
        matches!(win.input, Input::Decimal { value: Some(won), .. } if won == 3.0),
        "{:?}",
        win.input
    );
}

#[test]
fn a_metric_added_or_needed_is_not_offered_to_add() {
    let mut form = bare_form();
    form.add(&path("ranking"), MoreBuchholz::ID).unwrap();

    let view = form.view();
    let addable: Vec<_> = list(&view, "metrics")
        .addable
        .iter()
        .map(|metric| metric.id)
        .collect();
    assert!(addable.contains(&GamesDifference::ID));
    assert!(!addable.contains(&Points::ID));
    assert!(!addable.contains(&Buchholz::ID));
}

/// A phase as saved is not touched by opening its form: a metric nothing needs counts as added.
#[test]
fn a_metric_nothing_needs_counts_as_added() {
    let mut form = bare_form();
    form.add_metric(Points::ID).unwrap();
    let draft = form.draft().clone();

    let reopened = PhaseForm::new(draft.clone());

    assert_eq!(reopened.draft().answers(), draft.answers());
    assert!(row(&reopened, Points::ID).added);
    assert!(!row(&reopened, WinsDrawsLosses::ID).added);
}
