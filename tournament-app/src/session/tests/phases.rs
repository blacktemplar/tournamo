use tournament_engine::{Source, SourceOf};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::{RemovePhaseError, StartPhaseError},
    form::End,
    view::{Advance, Advancing, PhaseState, SourceProgress, SourceRow},
};

/// A round robin of Alice, Bob, Carol and Dave, then a knockout of its places 1–2, added through
/// the form a session offers for a further phase, not started.
fn qualification_and_final() -> Session {
    let mut session = Given::new().added();
    let mut knockout = session.phase_form().unwrap();
    knockout
        .choose(
            &ConfigPath::ROOT.field("format"),
            Some(SingleElimination::ID),
        )
        .unwrap();
    knockout.set_name(" Final ");
    knockout.enter_place(0, End::To, "2");
    session.add_phase(&knockout).unwrap();
    session
}

/// The id of the phase listed at `number`, from 1.
fn phase(session: &Session, number: usize) -> PhaseId {
    session.phases()[number - 1].id
}

/// Reports every open match of the phase `phase`, each won by side A.
fn play_out(session: &mut Session, phase: PhaseId) {
    while let Some(open) = session.playable(MatchesOf::phase(phase)).first().cloned() {
        session.report(phase, open.id, MatchResult::AWins).unwrap();
    }
}

#[test]
fn a_further_phase_starts_out_at_every_place_of_the_phase_before() {
    let session = Given::new().added();

    let form = session.phase_form().unwrap();

    let view = form.view().entry;
    assert_eq!(view.number, 2);
    assert!(!view.everyone_registered);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].of, Some(session.first()));
    assert_eq!((&*view.rows[0].from, &*view.rows[0].to), ("1", ""));
    assert_eq!(view.choices.len(), 2, "the registration and the phase");
    assert!(form.view().complete);
}

#[test]
fn the_first_phase_takes_everyone_registered() {
    let session = Session::new("cup");

    let view = session.phase_form().unwrap().view().entry;

    assert!(view.everyone_registered);
    assert!(view.rows.is_empty());
}

#[test]
fn the_phases_are_listed_with_their_names_sources_and_states() {
    let session = qualification_and_final();

    let phases = session.phases();

    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].name.number, 1);
    assert_eq!(phases[0].name.name, None);
    assert_eq!(phases[0].name.format.id, RoundRobin::ID);
    assert_eq!(phases[1].name.name.as_deref(), Some("Final"));
    assert_eq!(
        phases[1].sources,
        [SourceRow {
            phase: Some(phases[0].id),
            from: NonZeroU32::MIN,
            to: NonZeroU32::new(2),
        }]
    );
    assert_eq!(
        phases[1].state,
        PhaseState::NotStarted {
            waiting_for: vec![SourceProgress {
                phase: phases[0].id,
                played: 0,
                matches: 0,
            }]
        }
    );
}

#[test]
fn a_phase_starts_once_the_phase_it_takes_entrants_from_is_finished() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let (first, last) = (phase(&session, 1), phase(&session, 2));
    assert_eq!(session.shown_first(), Some(first));
    assert!(matches!(
        session.start_phase(last),
        Err(StartPhaseError::Waiting(waiting)) if waiting[0].1 == 0 && waiting[0].2 == 6
    ));

    play_out(&mut session, first);
    assert_eq!(
        session.phases()[0].state,
        PhaseState::Finished {
            can_start: vec![last]
        }
    );
    assert_eq!(session.shown_first(), Some(last));
    assert_eq!(session.competition_winners(), None, "a phase is left");
    session.start_phase(last).unwrap();

    assert_eq!(session.shown_first(), Some(last));
    assert_eq!(session.phases()[1].state, PhaseState::Running);
    assert_eq!(
        session
            .next_undo()
            .map(|action| matches!(action, Action::StartPhase(_))),
        Some(true)
    );
    let reported = {
        let open = session.playable(MatchesOf::phase(last))[0].id;
        session.report(last, open, MatchResult::AWins).unwrap()
    };
    assert!(reported.finished_phase);
    assert_eq!(reported.winners.as_ref().map(Vec::len), Some(1));
}

/// Side A wins every match of a round robin of four, which does not leave four different
/// records: the cut after place 2 goes through a tie or it does not, and the warning says so.
#[test]
fn a_cut_through_a_tie_is_warned_of_and_started_all_the_same() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let (first, last) = (phase(&session, 1), phase(&session, 2));
    play_out(&mut session, first);
    let standings = session.overall_standings(first).unwrap();
    let tied = standings.rows[1].rank == standings.rows[2].rank;

    let refusal = session.start_phase_refusal(last);

    assert_eq!(
        matches!(refusal, Some(StartPhaseError::CutThroughTie(_))),
        tied
    );
    if let Some(StartPhaseError::CutThroughTie(ties)) = refusal {
        assert_eq!(ties[0].rank, standings.rows[1].rank);
        assert!(ties[0].favoured.contains(&standings.rows[1].entrants));
        assert!(ties[0].others.contains(&standings.rows[2].entrants));
    }
    session.start_phase(last).unwrap();
}

#[test]
fn the_rows_that_go_on_are_marked_with_the_phase_they_go_to() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let (first, last) = (phase(&session, 1), phase(&session, 2));

    let standings = session.overall_standings(first).unwrap();

    // before any result everyone is tied, so the first two advance only by the lot
    let advances: Vec<_> = standings.rows.iter().map(|row| row.advances).collect();
    let compared = Some(Advance {
        to: last,
        how: Advancing::Compared,
    });
    assert_eq!(advances, [compared, compared, None, None]);
    assert!(standings.rows.iter().all(|row| row.medal.is_none()));
}

#[test]
fn a_result_of_a_phase_is_not_undone_once_a_phase_taking_its_entrants_started() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let (first, last) = (phase(&session, 1), phase(&session, 2));
    play_out(&mut session, first);
    let reported = matches(&session)[0].id;
    session.start_phase(last).unwrap();

    assert!(matches!(
        session.undo_report(first, reported),
        Err(UndoReportError::PhaseStarted(phase)) if phase.name.as_deref() == Some("Final")
    ));
    let mut draft = session.phase_draft_to_edit(first).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![PluginConfig::of::<MoreWins>()],
        )
        .unwrap();
    // the edit changes who advances, which is decided once the final started
    let edited = session.edit_phase(first, &form(draft));
    assert!(
        matches!(edited, Ok(_) | Err(EditPhaseError::PhaseStarted(_))),
        "{edited:?}"
    );
}

#[test]
fn a_phase_is_removed_unless_another_takes_its_entrants() {
    let mut session = qualification_and_final();
    let (first, last) = (phase(&session, 1), phase(&session, 2));

    assert!(matches!(
        session.remove_phase_refusal(first),
        Some(RemovePhaseError::InUse(phases)) if phases[0].name.as_deref() == Some("Final")
    ));
    assert!(session.remove_phase(first).is_err());
    assert!(session.remove_phase_refusal(last).is_none());
    session.remove_phase(last).unwrap();
    assert_eq!(session.phases().len(), 1);

    let undone = session.undo().unwrap();
    assert!(matches!(undone, Action::RemovePhase(phase) if phase.number == 2));
    assert_eq!(session.phases().len(), 2);
}

#[test]
fn a_phase_whose_adding_is_undone_comes_back_with_its_name_and_sources() {
    let mut session = qualification_and_final();
    let added = session.undo().unwrap();
    assert!(matches!(added, Action::AddPhase(phase) if phase.number == 2));

    let form = session.phase_form().unwrap();

    assert_eq!(form.name(), " Final ".trim());
    assert_eq!(
        form.sources(),
        [Source::new(
            SourceOf::Phase(session.first()),
            NonZeroU32::MIN,
            NonZeroU32::new(2)
        )]
    );
}

#[test]
fn sources_taking_places_taken_or_none_are_shown_as_issues() {
    let session = qualification_and_final();
    let mut form = session.phase_form().unwrap();

    // the phase before is the final, of which nobody takes a place yet
    assert!(form.view().complete);
    form.choose_source(0, Some(session.first()));
    let view = form.view();
    assert!(!view.complete);
    assert!(view.entry.rows[0].issue.is_some(), "the final takes 1–2");
    form.enter_place(0, End::From, "3");
    assert!(form.view().complete);
    form.enter_place(0, End::To, "two");
    assert!(form.view().entry.rows[0].to_rejected.is_some());
    assert!(!form.view().complete);
    form.enter_place(0, End::To, "2");
    assert!(form.view().entry.rows[0].issue.is_some(), "3 to 2 is empty");
    form.remove_source(0);
    assert!(!form.view().entry.issues.is_empty(), "no source");
    assert!(!form.view().complete);
}

#[test]
fn the_scale_of_a_phase_counts_the_places_its_sources_take() {
    let session = qualification_and_final();
    let mut form = session.phase_form_to_edit(phase(&session, 2)).unwrap();

    // a knockout of two is a final alone
    assert_eq!(
        session.scale(&form).map(|scale| scale.matches.most),
        Some(1)
    );
    // with its match for third place
    form.enter_place(0, End::To, "");
    assert_eq!(
        session.scale(&form).map(|scale| scale.matches.most),
        Some(4)
    );
}

#[test]
fn the_ranking_places_everyone_by_the_phase_they_stopped_in() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let (first, last) = (phase(&session, 1), phase(&session, 2));
    play_out(&mut session, first);
    session.start_phase(last).unwrap();
    assert!(!session.ranking().is_final);
    play_out(&mut session, last);

    let ranking = session.ranking();

    assert!(ranking.is_final);
    let in_phase: Vec<_> = ranking.rows.iter().map(|row| row.phase).collect();
    assert_eq!(in_phase, [last, last, first, first]);
    assert_eq!(ranking.rows[0].medal, Some(Medal::Gold));
    assert_eq!(
        session.competition_winners(),
        Some(vec![ranking.rows[0].entrants.clone()])
    );
    assert!(matches!(session.progress(), Progress::Won { .. }));
}
