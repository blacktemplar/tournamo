//! Where the metrics are shown: the list of metrics as a table of the tables of standings.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

const WDL: &str = WinsDrawsLossesCalculator::ID;
const POINTS: &str = PointsCalculator::ID;
const SB: &str = SonnebornBergerCalculator::ID;
const BUCHHOLZ: &str = BuchholzCalculator::ID;

/// Where the metric `id` of the list of metrics of `view` is shown.
#[track_caller]
fn shown<'a>(view: &'a FormView, id: &str) -> &'a MetricShown {
    list(view, "metrics")
        .shown
        .iter()
        .find(|metric| metric.plugin.id == id)
        .unwrap_or_else(|| panic!("{id} is not in the list of metrics"))
}

/// Whether `table` shows the metric `id` of the form `form`, and who decided it.
#[track_caller]
fn shown_in(form: &PhaseForm, id: &str, table: Table) -> ShownIn {
    *shown(&form.view(), id).of(table)
}

/// The id of the plugin `because` names.
fn why(because: Because) -> (&'static str, &'static str) {
    match because {
        Because::RankedBy(criterion) => ("ranked by", criterion.id),
        Because::CalculatedFrom(calculator) => ("calculated from", calculator.id),
    }
}

/// What the tables of standings of `form` show by rule, with why, as [`why`] puts it.
fn by_rule(form: &PhaseForm, table: Table) -> Vec<(&'static str, (&'static str, &'static str))> {
    list(&form.view(), "metrics")
        .shown
        .iter()
        .filter_map(|metric| match metric.of(table).by {
            By::Rule(Some(because)) => Some((metric.plugin.id, why(because))),
            _ => None,
        })
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
    let buchholz = shown(&view, BUCHHOLZ);
    assert!(!buchholz.calculated && buchholz.column);
    assert_eq!(
        buchholz.tables,
        [ShownIn {
            on: false,
            by: By::Rule(None)
        }; 2]
    );
    let names: Vec<_> = shown(&view, WDL)
        .needed_by
        .iter()
        .map(|plugin| plugin.id)
        .collect();
    assert_eq!(names, [POINTS]);
    // the list shows an entry for every plugin of the list
    assert_eq!(
        list(&view, "metrics").shown.len(),
        list(&view, "metrics").set().len()
    );
}

#[test]
fn a_metric_ticked_is_calculated_and_unticked_is_no_longer() {
    let mut form = new_form();

    form.set_shown(BUCHHOLZ, Table::Division, true).unwrap();

    assert!(items(&form.view(), "metrics").contains(&BUCHHOLZ));
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

    form.set_shown(BUCHHOLZ, Table::Division, false).unwrap();

    assert!(!items(&form.view(), "metrics").contains(&BUCHHOLZ));
    assert!(form.columns().is_empty());
}

#[test]
fn a_metric_needed_and_hidden_is_still_calculated_and_ticked_back_follows_the_rule() {
    let mut form = new_form();

    form.set_shown(WDL, Table::Division, false).unwrap();

    assert!(items(&form.view(), "metrics").contains(&WDL));
    assert_eq!(
        shown_in(&form, WDL, Table::Division),
        ShownIn {
            on: false,
            by: By::Operator
        }
    );
    assert_eq!(form.view().overridden(Table::Division, false)[0].id, WDL);

    form.set_shown(WDL, Table::Division, true).unwrap();

    assert!(form.columns().is_empty());
    assert!(matches!(
        shown_in(&form, WDL, Table::Division).by,
        By::Rule(Some(_))
    ));
}

#[test]
fn a_metric_shown_in_one_table_stays_while_hidden_in_the_other() {
    let mut form = new_form();
    form.set_shown(BUCHHOLZ, Table::Division, true).unwrap();
    form.set_shown(BUCHHOLZ, Table::Across, true).unwrap();

    form.set_shown(BUCHHOLZ, Table::Division, false).unwrap();

    assert!(items(&form.view(), "metrics").contains(&BUCHHOLZ));
    assert_eq!(form.columns().get(BUCHHOLZ).across, Some(true));
    assert_eq!(form.columns().get(BUCHHOLZ).division, None);
}

#[test]
fn a_metric_leaving_the_list_takes_what_was_said_about_it_along() {
    let mut form = new_form();
    form.set_shown(BUCHHOLZ, Table::Division, true).unwrap();
    let item = list(&form.view(), "metrics")
        .items
        .iter()
        .find(|item| item.plugin.id == BUCHHOLZ)
        .unwrap()
        .path
        .clone();

    form.remove(&item).unwrap();

    assert!(form.columns().is_empty());
}

#[test]
fn applying_suggestions_follows_the_rule_again() {
    let mut form = new_form();
    form.set_shown(WDL, Table::Division, false).unwrap();

    form.apply_suggestions().unwrap();

    assert!(form.columns().is_empty());
}

#[test]
fn a_metric_nothing_needs_and_nothing_shows_stays_until_its_row_is_touched() {
    let mut session = Session::new("cup");
    let mut form = session.phase_form().unwrap();
    // ranked by points alone, the Sonneborn-Berger score is neither needed nor shown
    form.remove(&path("ranking[1]")).unwrap();
    session.add_phase(&form).unwrap();

    let mut form = session.phase_form_to_edit(session.first()).unwrap();
    assert!(items(&form.view(), "metrics").contains(&SB));
    assert!(
        Table::ALL
            .iter()
            .all(|&table| !shown_in(&form, SB, table).on)
    );
    form.set_shown(WDL, Table::Division, false).unwrap();
    assert!(items(&form.view(), "metrics").contains(&SB));

    form.set_shown(SB, Table::Division, false).unwrap();

    assert!(!items(&form.view(), "metrics").contains(&SB));
}

#[test]
fn the_columns_of_a_phase_are_added_edited_and_brought_back_with_it() {
    let mut session = Session::new("cup");
    let mut form = session.phase_form().unwrap();
    form.set_shown(BUCHHOLZ, Table::Division, true).unwrap();

    session.add_phase(&form).unwrap();

    let mut edited = session.phase_form_to_edit(session.first()).unwrap();
    assert_eq!(edited.columns(), form.columns());
    edited.set_shown(WDL, Table::Division, false).unwrap();
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
