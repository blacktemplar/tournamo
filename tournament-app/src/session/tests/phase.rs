#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

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
