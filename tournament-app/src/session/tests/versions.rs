//! Phases of an earlier version of a plugin, with a format of the tests in two versions, so that
//! no test depends on a real earlier version, which goes once it is old.

use plugin_factory::{Plugin, register};
use serde::{Deserialize, Serialize};
use tournament_engine::{
    DivisionView, NewMatch, RankingId,
    plugins::phase_format::{PhaseFormat, Schedule},
    vec1::vec1,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::form::{Input, PluginInput};

/// The first version of a format of the tests: one round, the rankings paired in order, the first
/// with the second, the third with the fourth, and so on.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_app::paired",
    label = "paired",
    description = "a format of the tests"
)]
struct Paired;

#[register]
impl PhaseFormat for Paired {
    fn create_new_matches(&mut self, phase: &DivisionView<'_>) -> Schedule {
        one_round(phase, |rankings| rankings.as_chunks().0.to_vec())
    }
}

/// The second version of [`Paired`]: the first ranking with the last, the second with the one
/// before the last, and so on.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_app::paired",
    version = 2,
    label = "paired",
    description = "a format of the tests"
)]
struct PairedOutside;

#[register]
impl PhaseFormat for PairedOutside {
    fn create_new_matches(&mut self, phase: &DivisionView<'_>) -> Schedule {
        one_round(phase, |rankings| {
            let half = rankings.len() / 2;
            (0..half)
                .map(|index| [rankings[index], rankings[rankings.len() - 1 - index]])
                .collect()
        })
    }
}

/// The one round of `phase`, its rankings paired by `pairs`, unless it is scheduled already.
fn one_round(
    phase: &DivisionView<'_>,
    pairs: impl Fn(&[RankingId]) -> Vec<[RankingId; 2]>,
) -> Schedule {
    let data = phase.data();
    if data.has_schedule() {
        return Schedule::default();
    }
    let rankings: Vec<_> = data.rankings().keys().copied().collect();
    Schedule::from(
        pairs(&rankings)
            .into_iter()
            .map(|[a, b]| NewMatch::new(vec1![a], vec1![b], NonZeroU32::MIN))
            .collect::<Vec<_>>(),
    )
}

/// The format chosen, as the form of the phase `phase` shows it.
fn format_of(session: &Session, phase: PhaseId) -> PluginInput {
    let view = session.phase_form_to_edit(phase).unwrap().view();
    match &view.fields[0].input {
        Input::Plugin(format) => format.clone(),
        input => panic!("the format is chosen from plugins, not {input:?}"),
    }
}

/// The id of the format the phase `phase` is played with.
fn chosen(session: &Session, phase: PhaseId) -> Option<&'static str> {
    format_of(session, phase)
        .chosen
        .map(|chosen| chosen.plugin.id)
}

fn reported(session: &Session) -> Vec<(MatchId, MatchResult)> {
    matches(session)
        .into_iter()
        .filter_map(|row| Some((row.id, row.result?)))
        .collect()
}

/// A phase played with an earlier version of its format keeps it, shown as outdated, though only
/// the latest version is offered for a new phase.
#[test]
fn a_phase_keeps_the_earlier_version_of_its_format_it_was_added_with() {
    let session = Given::new().format(Paired::ID).started();

    let format = format_of(&session, session.first());

    assert_eq!(chosen(&session, session.first()), Some(Paired::ID));
    assert!(
        format
            .options
            .iter()
            .any(|option| option.outdated && option.id == Paired::ID)
    );
    assert!(
        format
            .options
            .iter()
            .any(|option| !option.outdated && option.id == PairedOutside::ID)
    );
    let outdated: Vec<_> = session
        .phase_form_to_edit(session.first())
        .unwrap()
        .view()
        .outdated
        .into_iter()
        .map(|plugin| plugin.id)
        .collect();
    assert_eq!(outdated, [Paired::ID]);
    assert!(
        Session::formats()
            .iter()
            .all(|format| format.id != Paired::ID)
    );
}

/// Without results, nothing keeps a phase from the latest version of its format.
#[test]
fn a_phase_is_updated_to_the_latest_version_of_its_plugins_its_results_fit() {
    let mut session = Given::new().format(Paired::ID).started();
    let mut form = session.phase_form_to_edit(session.first()).unwrap();

    form.update_plugins().unwrap();

    assert!(form.view().outdated.is_empty());
    assert!(session.edit_phase(session.first(), &form).unwrap());
    assert_eq!(chosen(&session, session.first()), Some(PairedOutside::ID));
}

/// The latest version pairs the entrants otherwise, so a result reported does not fit it, and the
/// phase keeps its version and its result.
#[test]
fn a_phase_whose_results_do_not_fit_the_latest_version_is_not_updated() {
    let mut session = Given::new().format(Paired::ID).started();
    session
        .report(session.first(), numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    let before = reported(&session);
    let mut form = session.phase_form_to_edit(session.first()).unwrap();
    form.update_plugins().unwrap();

    let result = session.edit_phase(session.first(), &form);

    assert!(
        matches!(&result, Err(EditPhaseError::ResultDoesNotFit(matches)) if matches.len() == 1),
        "{result:?}"
    );
    assert_eq!(chosen(&session, session.first()), Some(Paired::ID));
    assert_eq!(reported(&session), before);
}
