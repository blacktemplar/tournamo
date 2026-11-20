//! Where the metrics are shown: the list of metrics as a table of the tables of standings.

use tournament_engine::plugins::metrics_calculator::{
    Buchholz, MetricId, Points, SonnebornBerger, WinsDrawsLosses,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

const WDL: &str = WinsDrawsLosses::ID;
const POINTS: &str = Points::ID;
const SB: &str = SonnebornBerger::ID;
const BUCHHOLZ: &str = Buchholz::ID;

/// The row of the metric `id` of the list of metrics of `view`.
#[track_caller]
fn shown<'a>(view: &'a FormView, id: &str) -> &'a MetricRow {
    list(view, "metrics")
        .metrics
        .iter()
        .find(|row| row.metric.id == id)
        .unwrap_or_else(|| panic!("{id} is not in the list of metrics"))
}

/// Whether `table` shows the metric `id` of the form `form`, and who decided it.
#[track_caller]
fn shown_in(form: &PhaseForm, id: &str, table: Table) -> ShownIn {
    *shown(&form.view(), id).of(table)
}

/// The id of what `because` names.
fn why(because: Because) -> (&'static str, &'static str) {
    match because {
        Because::RankedBy(criterion) => ("ranked by", criterion.id),
        Because::CalculatedFrom(metric) => ("calculated from", metric.id),
    }
}

/// What the tables of standings of `form` show by rule, with why, as [`why`] puts it.
fn by_rule(form: &PhaseForm, table: Table) -> Vec<(&'static str, (&'static str, &'static str))> {
    list(&form.view(), "metrics")
        .metrics
        .iter()
        .filter_map(|row| match row.of(table).by {
            By::Rule(Some(because)) => Some((row.metric.id, why(because))),
            _ => None,
        })
        .collect()
}

/// The ids of the metrics of the form, in the order of its rows.
fn metrics(form: &PhaseForm) -> Vec<&'static str> {
    list(&form.view(), "metrics")
        .metrics
        .iter()
        .map(|row| row.metric.id)
        .collect()
}

#[test]
fn every_metric_is_shown_by_rule_where_the_table_is_ranked_by_it_or_what_that_is_calculated_from() {
    let form = new_form();

    let mut division = by_rule(&form, Table::Division);
    division.sort_unstable();
    assert_eq!(
        division,
        [
            (POINTS, ("ranked by", MorePoints::ID)),
            (SB, ("ranked by", MoreSonnebornBerger::ID)),
            (WDL, ("calculated from", POINTS)),
        ]
    );
    // compared by nothing, the rows compared show nothing
    assert_eq!(by_rule(&form, Table::Across), []);
    let view = form.view();
    let names: Vec<_> = shown(&view, WDL)
        .needed_by
        .iter()
        .map(|plugin| plugin.id)
        .collect();
    assert_eq!(names, [PointsCalculator::ID]);
    // a row for every metric added or needed, and none for the others
    assert_eq!(metrics(&form).len(), 3);
}

#[test]
fn a_metric_added_is_shown_and_removed_is_no_longer_calculated() {
    let mut form = new_form();

    form.add_metric(BUCHHOLZ).unwrap();

    assert!(items(&form.view(), "metrics").contains(&BuchholzCalculator::ID));
    assert_eq!(
        shown_in(&form, BUCHHOLZ, Table::Division),
        ShownIn {
            on: true,
            by: By::Operator
        }
    );
    assert_eq!(form.columns().get(BUCHHOLZ).division, Some(true));
    assert_eq!(
        form.view().overridden(Table::Division, true)[0].id,
        BUCHHOLZ
    );

    form.remove_metric(BUCHHOLZ).unwrap();

    assert!(!items(&form.view(), "metrics").contains(&BuchholzCalculator::ID));
    assert!(form.columns().is_empty());
}

#[test]
fn a_metric_hidden_is_still_calculated_and_shown_again_follows_the_rule() {
    let mut form = new_form();

    form.set_shown(WDL, Table::Division, false);

    assert!(items(&form.view(), "metrics").contains(&WinsDrawsLossesCalculator::ID));
    assert_eq!(
        shown_in(&form, WDL, Table::Division),
        ShownIn {
            on: false,
            by: By::Operator
        }
    );
    assert_eq!(form.view().overridden(Table::Division, false)[0].id, WDL);

    form.set_shown(WDL, Table::Division, true);

    assert!(form.columns().is_empty());
    assert!(matches!(
        shown_in(&form, WDL, Table::Division).by,
        By::Rule(Some(_))
    ));
}

#[test]
fn a_metric_hidden_in_one_table_keeps_what_was_said_about_the_other() {
    let mut form = new_form();
    form.add_metric(BUCHHOLZ).unwrap();
    form.set_shown(BUCHHOLZ, Table::Across, true);

    form.set_shown(BUCHHOLZ, Table::Division, false);

    assert!(items(&form.view(), "metrics").contains(&BuchholzCalculator::ID));
    assert_eq!(form.columns().get(BUCHHOLZ).across, Some(true));
    assert_eq!(form.columns().get(BUCHHOLZ).division, None);
}

#[test]
fn a_metric_leaving_the_list_takes_what_was_said_about_it_along() {
    let mut form = new_form();
    form.set_shown(SB, Table::Division, false);

    // ranked by points alone, the Sonneborn-Berger score is needed no more
    form.remove(&path("ranking[1]")).unwrap();

    assert!(!metrics(&form).contains(&SB));
    assert!(form.columns().is_empty());
}

#[test]
fn applying_suggestions_follows_the_rule_again() {
    let mut form = new_form();
    form.set_shown(WDL, Table::Division, false);

    form.apply_suggestions().unwrap();

    assert!(form.columns().is_empty());
}

#[test]
fn the_columns_of_a_phase_are_added_edited_and_brought_back_with_it() {
    let mut session = Session::new("cup");
    let mut form = session.phase_form().unwrap();
    form.add_metric(BUCHHOLZ).unwrap();

    session.add_phase(&form).unwrap();

    let mut edited = session.phase_form_to_edit(session.first()).unwrap();
    assert_eq!(edited.columns(), form.columns());
    edited.set_shown(WDL, Table::Division, false);
    assert!(session.edit_phase(session.first(), &edited).unwrap());
    assert_eq!(
        session
            .phase_form_to_edit(session.first())
            .unwrap()
            .columns(),
        edited.columns()
    );
    session.undo().unwrap();
    session.undo().unwrap();
    assert_eq!(session.phase_form().unwrap().columns(), form.columns());
}
