use tournament_engine::{Source, SourceOf};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::{RemovePhaseError, StartPhaseError, UnstartPhaseError},
    form::{Editable as _, End},
    i18n::Text,
    people::Rating,
    result_form::ResultForm,
    view::{Advance, Advancing, Cut, PhaseState, SourceProgress, SourceRow},
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

/// Reports every open match of the phase `phase`, each won by the entrant registered first, who
/// is first by name too.
fn play_out_by_name(session: &mut Session, phase: PhaseId) {
    while let Some(open) = session.playable(MatchesOf::phase(phase)).first().cloned() {
        let result = if names(&open.a) < names(&open.b) {
            MatchResult::AWins
        } else {
            MatchResult::BWins
        };
        session.report(phase, open.id, result).unwrap();
    }
}

/// A refused edit names the results of a later phase it does not fit, by that phase: ranked by
/// Buchholz alone, Dave and Carol, who lost most, played the strongest and would reach the final
/// Alice and Bob played.
#[test]
fn an_edit_names_the_results_of_a_later_phase_it_does_not_fit() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let qualification = session.first();
    play_out_by_name(&mut session, qualification);
    let r#final = phase(&session, 2);
    session.start_phase(r#final).unwrap();
    play_out(&mut session, r#final);
    let mut draft = session.phase_draft_to_edit(qualification).unwrap();
    draft
        .set(
            &ConfigPath::ROOT.field("ranking"),
            vec![PluginConfig::of::<MoreBuchholz>()],
        )
        .unwrap();

    let result = session.edit_phase(qualification, &form(draft));

    let later = MatchesOfPhase {
        phase: session.name_of_phase(r#final),
        numbers: vec![7],
    };
    assert!(
        matches!(&result, Err(EditPhaseError::ResultDoesNotFit(matches)) if *matches == [later]),
        "{result:?}"
    );
    assert_eq!(
        crate::i18n::english(&result.unwrap_err()),
        "changed this way, the phase does not fit the result of match 7 of Final, which it sends \
         entrants to; reopen that match first"
    );
}

/// The entrants of the overall standings of the phase `phase`, best first.
fn standing(session: &Session, phase: PhaseId) -> Vec<String> {
    session
        .overall_standings(phase)
        .unwrap()
        .rows
        .iter()
        .flat_map(|row| names(&row.entrants).into_iter().map(str::to_owned))
        .collect()
}

/// The qualification of [`qualification_and_final`] played by name, so that Alice and Bob reach the
/// final, which is played, and the form editing the result of Bob against Carol, entered as won by
/// Carol, which would have sent Carol to the final instead.
fn bob_beaten_after_the_final() -> (Session, ResultForm) {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let qualification = session.first();
    play_out_by_name(&mut session, qualification);
    let r#final = phase(&session, 2);
    session.start_phase(r#final).unwrap();
    play_out(&mut session, r#final);
    let bob_carol = matches(&session)
        .into_iter()
        .find(|row| {
            let mut both = names(&row.a);
            both.extend(names(&row.b));
            both.sort_unstable();
            both == ["Bob", "Carol"]
        })
        .unwrap();
    let carol = if names(&bob_carol.a) == ["Carol"] {
        MatchResult::AWins
    } else {
        MatchResult::BWins
    };
    let mut form = session.edit_form(qualification, bob_carol.id).unwrap();
    form.choose(&[], Some(carol));
    (session, form)
}

/// A result of a phase whose next phase has started is corrected in place: the standings count
/// it, while the next phase keeps the entrants it started with and what they played.
#[test]
fn a_result_is_corrected_in_place_keeping_the_next_phase_as_it_started() {
    let (mut session, form) = bob_beaten_after_the_final();
    let (qualification, r#final) = (session.first(), phase(&session, 2));
    let finalists = standing(&session, r#final);
    assert!(
        session.edit_report(&form).is_err(),
        "Carol would reach the final"
    );
    assert!(session.correctable(&form));

    let corrected = session.correct_report(&form).unwrap();

    assert_eq!(corrected.row.result, form.view().root.result);
    assert_eq!(standing(&session, qualification)[..2], ["Alice", "Carol"]);
    assert_eq!(standing(&session, r#final), finalists);
    assert_eq!(session.undo().unwrap(), Action::EditResult(corrected.row));
    assert_eq!(standing(&session, qualification)[..2], ["Alice", "Bob"]);
}

/// Once corrected in place, a result is corrected in place on every edit, without being refused
/// again; it is reopened only as any result of the phase would be.
#[test]
fn a_corrected_result_is_corrected_in_place_again() {
    let (mut session, form) = bob_beaten_after_the_final();
    let qualification = session.first();
    session.correct_report(&form).unwrap();
    let mut again = session.edit_form(qualification, form.match_id()).unwrap();
    again.choose(&[], Some(MatchResult::Draw));

    let edited = session.edit_report(&again).unwrap();

    assert_eq!(edited.row.result, Some(MatchResult::Draw));
    // a phase taking entrants from the qualification has started, so it is not reopened
    assert!(matches!(
        session.undo_report(qualification, form.match_id()),
        Err(UndoReportError::PhaseStarted(_))
    ));
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

/// Asked for, the first phase takes only some places of the registration, e.g. to leave the best
/// rated for a later phase.
#[test]
fn the_first_phase_takes_some_places_of_the_registration_when_asked() {
    let mut session = Session::new("cup");
    let mut form = session.phase_form().unwrap();

    form.take_some_places();
    form.enter_place(0, End::From, "3");

    let view = form.view().entry;
    assert!(!view.everyone_registered);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].of, None);
    assert_eq!((&*view.rows[0].from, &*view.rows[0].to), ("3", ""));
    session.add_phase(&form).unwrap();
    assert_eq!(
        session.competition().only_phase().unwrap().sources()[..],
        [Source::new(
            SourceOf::Registration,
            NonZeroU32::new(3).unwrap(),
            None
        )]
    );
}

/// The lot decides who of the registration a source takes where its cut goes through entrants
/// of equal ratings or none.
#[test]
fn a_cut_through_equal_ratings_or_none_is_decided_by_lot() {
    let mut session = Given::new()
        .entrants(&["Alice", "Bob", "Carol", "Dave", "Eve"])
        .added();
    let ids: Vec<_> = session.entrants().iter().map(|row| row.id).collect();
    for (id, rating) in ids.iter().zip(["1900", "1800", "1800", "1700"]) {
        session
            .set_rating(*id, Rating::parse(rating, Language::English))
            .unwrap();
    }
    let places = |from: u32, to: Option<u32>| {
        [Source::new(
            SourceOf::Registration,
            NonZeroU32::new(from).unwrap(),
            to.and_then(NonZeroU32::new),
        )]
    };
    let cuts = |session: &Session, sources: &[Source]| -> Vec<usize> {
        let warnings = session.lot_cuts(sources);
        (1..5)
            .filter(|&place| {
                warnings.contains(&Text::new("warning-lot-decides-cut").with("place", place))
            })
            .collect()
    };

    // 1900 | 1800 1800 | 1700 | none
    assert_eq!(cuts(&session, &places(1, Some(1))), Vec::<usize>::new());
    assert_eq!(cuts(&session, &places(1, Some(2))), [2]);
    assert_eq!(cuts(&session, &places(2, Some(3))), Vec::<usize>::new());
    assert_eq!(cuts(&session, &places(4, None)), Vec::<usize>::new());
    assert_eq!(cuts(&session, &places(3, Some(4))), [2]);
    session.set_rating(ids[4], None).unwrap();
    session.set_rating(ids[3], None).unwrap();
    // 1900 | 1800 1800 | none none
    assert_eq!(cuts(&session, &places(1, Some(4))), [4]);
    session.start().unwrap();
    assert!(session.lot_cuts(&places(1, Some(2))).is_empty());
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

    // before any result everyone is tied, yet the lot has set them apart: the first two go on
    // by their place, and a line follows them
    let advances: Vec<_> = standings.rows.iter().map(|row| row.advances).collect();
    let by_place = Some(Advance {
        to: last,
        how: Advancing::ByPlace,
    });
    assert_eq!(advances, [by_place, by_place, None, None]);
    let cuts: Vec<_> = standings.rows.iter().map(|row| row.cut.clone()).collect();
    let cut = Some(Cut {
        to: last,
        how: Advancing::ByPlace,
        from: NonZeroU32::MIN,
        through: NonZeroU32::new(2).unwrap(),
        below_by_lot: Vec::new(),
    });
    assert_eq!(cuts, [None, cut, None, None]);
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

/// Undoing the start of the final makes the results of the phase before it reopenable, without
/// the session's undo, which is gone after a reload.
#[test]
fn a_phase_whose_start_is_undone_frees_the_results_it_took_its_entrants_by() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let (first, last) = (phase(&session, 1), phase(&session, 2));
    play_out(&mut session, first);
    let reported = matches(&session)[0].id;
    session.start_phase(last).unwrap();

    assert!(matches!(
        session.unstart_phase_refusal(first),
        Some(UnstartPhaseError::InUse(phases)) if phases[0].name.as_deref() == Some("Final")
    ));
    assert!(session.unstart_phase_refusal(last).is_none());
    session.unstart_phase(last).unwrap();

    assert!(matches!(
        session.phases()[1].state,
        PhaseState::NotStarted { .. }
    ));
    assert!(matches!(
        session.unstart_phase_refusal(last),
        Some(UnstartPhaseError::NotStarted)
    ));
    session.undo_report(first, reported).unwrap();
    session.undo().unwrap();
    let undone = session.undo().unwrap();
    assert!(matches!(undone, Action::UnstartPhase(phase) if phase.number == 2));
    assert!(matches!(session.phases()[1].state, PhaseState::Running));
}

/// The first phase started with the competition, so undoing its start goes back to the setup.
#[test]
fn undoing_the_start_of_the_first_phase_goes_back_to_the_setup() {
    let mut session = qualification_and_final();
    session.start().unwrap();
    let first = phase(&session, 1);
    let reported = session.playable(MatchesOf::phase(first))[0].id;
    session.report(first, reported, MatchResult::AWins).unwrap();
    assert!(matches!(
        session.unstart_phase_refusal(first),
        Some(UnstartPhaseError::Played(phases)) if phases[0].number == 1
    ));
    session.undo_report(first, reported).unwrap();

    session.unstart_phase(first).unwrap();

    assert!(!session.competition().is_started());
    session.add_entrant("Erin").unwrap();
    let erin = session.entrants()[4].clone();
    assert_eq!(session.undo().unwrap(), Action::AddEntrant(erin));
    assert_eq!(session.undo().unwrap(), Action::Unstart);
    assert!(session.competition().is_started());
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
        session.scale(&form).and_then(|scale| scale.matches.most),
        Some(1)
    );
    // with its match for third place
    form.enter_place(0, End::To, "");
    assert_eq!(
        session.scale(&form).and_then(|scale| scale.matches.most),
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

/// Before a phase is finished, a cut through a rank of it is not decided: the rows of the rank
/// share their place in the ranking, as they share it in the standings.
#[test]
fn a_cut_through_a_tie_of_a_phase_still_played_splits_nothing() {
    let mut session = qualification_and_final();
    session.start().unwrap();

    let places: Vec<_> = session
        .ranking()
        .rows
        .iter()
        .map(|row| row.place.get())
        .collect();

    assert_eq!(places, [1, 1, 1, 1]);
}

/// Entrants of the registration that a phase not started yet takes are placed by it, waiting,
/// each a row of their own sharing the place their range of the registration starts at.
#[test]
fn entrants_waiting_for_a_phase_are_in_the_ranking() {
    let mut session = Session::new("cup");
    for name in ["A", "B", "C", "D", "E", "F"] {
        session.add_entrant(name).unwrap();
    }
    // a qualification of the registration from place 3
    let mut qualification = session.phase_form().unwrap();
    qualification.take_some_places();
    qualification.enter_place(0, End::From, "3");
    session.add_phase(&qualification).unwrap();
    // its best two, and the best two of the registration
    let mut knockout = session.phase_form().unwrap();
    knockout
        .choose(
            &ConfigPath::ROOT.field("format"),
            Some(SingleElimination::ID),
        )
        .unwrap();
    knockout.enter_place(0, End::To, "2");
    knockout.add_source();
    knockout.choose_source(1, None);
    knockout.enter_place(1, End::From, "1");
    knockout.enter_place(1, End::To, "2");
    session.add_phase(&knockout).unwrap();
    session.start().unwrap();

    let ranking = session.ranking();

    let rows: Vec<_> = ranking
        .rows
        .iter()
        .map(|row| (row.place.get(), row.entrants.len(), row.waiting))
        .collect();
    assert_eq!(
        rows,
        [
            (1, 1, true),
            (1, 1, true),
            (3, 1, false),
            (3, 1, false),
            (3, 1, false),
            (3, 1, false),
        ]
    );
    assert_eq!(ranking.rows[0].phase, phase(&session, 2));
}
