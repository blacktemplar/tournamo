//! Phases of several divisions: listing, filtering and reporting their matches, their standings,
//! and who advances from them.

use std::collections::HashSet;

use tournament_engine::plugins::draw::DivisionOrder;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::StartPhaseError,
    form::End,
    view::{Advance, Advancing, division_letter},
};

const SIX: [&str; 6] = ["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"];

/// Six entrants in two groups of three, then a knockout of the places 1–3 of the groups, not
/// started.
fn groups_and_knockout() -> Session {
    let mut session = Given::new().entrants(&SIX).divisions(2).added();
    let mut knockout = session.phase_form().unwrap();
    knockout
        .choose(
            &ConfigPath::ROOT.field("format"),
            Some(SingleElimination::ID),
        )
        .unwrap();
    knockout.enter_place(0, End::To, "3");
    session.add_phase(&knockout).unwrap();
    session
}

/// Reports every open match of the phase `phase` as won by the side with the better lot, so that
/// every group ends in the order of the lots, without a tie in it.
fn play_out_by_lot(session: &mut Session, phase: PhaseId) {
    while let Some(open) = session.playable(MatchesOf::phase(phase)).first().cloned() {
        let lot = |team: &[EntrantRow]| session.competition().lot(&team[0].id);
        let result = if lot(&open.a) < lot(&open.b) {
            MatchResult::AWins
        } else {
            MatchResult::BWins
        };
        session.report(phase, open.id, result).unwrap();
    }
}

#[test]
fn divisions_are_lettered() {
    assert_eq!(division_letter(0), "A");
    assert_eq!(division_letter(2), "C");
    assert_eq!(division_letter(25), "Z");
    assert_eq!(division_letter(26), "AA");
}

#[test]
fn the_phases_list_their_divisions_with_their_progress() {
    let mut session = groups_and_knockout();
    session.start().unwrap();
    let groups = session.first();

    let phases = session.phases();

    assert_eq!(phases[0].configured_divisions, 2);
    assert_eq!(phases[0].order, DivisionOrder::Parallel);
    let divisions = &phases[0].divisions;
    assert_eq!(divisions.len(), 2);
    assert_eq!(divisions[1].division.letter(), "B");
    assert!(divisions.iter().all(|d| d.played == 0 && d.matches == 3));
    assert!(phases[1].divisions.is_empty());

    let some = session.playable(MatchesOf::phase(groups))[0].clone();
    session.report(groups, some.id, MatchResult::AWins).unwrap();
    let played: Vec<_> = session.phases()[0]
        .divisions
        .iter()
        .map(|d| d.played)
        .collect();
    assert_eq!(played.iter().sum::<usize>(), 1);
    assert_eq!(played[some.division], 1);
}

#[test]
fn the_matches_of_every_division_are_listed_or_those_of_one() {
    let session = Given::new().entrants(&SIX).divisions(2).started();
    let groups = session.first();

    let all = session.playable(MatchesOf::phase(groups));
    let of_b = session.playable(MatchesOf {
        phase: groups,
        division: Some(1),
    });

    assert_eq!(all.len(), 2, "one match of each group can be played");
    assert_eq!(of_b.len(), 1);
    assert_eq!(of_b[0].division, 1);
    assert!(all.iter().any(|row| row.division == 0));
    let numbers: HashSet<_> = matches(&session).iter().map(|row| row.number).collect();
    assert_eq!(numbers.len(), 6, "numbers are unique in the phase");
}

#[test]
fn a_typed_result_finds_a_match_of_any_division() {
    let mut session = Given::new().entrants(&SIX).divisions(2).started();
    let groups = session.first();
    let of_b = session.playable(MatchesOf {
        phase: groups,
        division: Some(1),
    })[0]
        .clone();

    let reported = session
        .report_entered(MatchesOf::phase(groups), &format!("{} 1", of_b.number))
        .unwrap();

    assert_eq!(reported.row.id, of_b.id);
    assert_eq!(reported.row.division, 1);
}

#[test]
fn the_standings_of_a_division_and_the_overall_standings() {
    let mut session = groups_and_knockout();
    session.start().unwrap();
    let groups = session.first();
    play_out_by_lot(&mut session, groups);

    let of_b = session
        .standings(DivisionRef {
            phase: groups,
            division: 1,
        })
        .unwrap();
    let overall = session.overall_standings(groups).unwrap();

    assert_eq!(of_b.rows.len(), 3);
    assert!(of_b.rows.iter().all(|row| row.division == 1));
    let ranks: Vec<_> = of_b.rows.iter().map(|row| row.rank.get()).collect();
    assert_eq!(ranks, [1, 2, 3]);
    // the firsts, the seconds and the thirds of both groups, each two sharing a rank
    let overall: Vec<_> = overall
        .rows
        .iter()
        .map(|row| (row.rank.get(), row.division))
        .collect();
    assert_eq!(overall.len(), 6);
    assert_eq!(
        overall.iter().map(|(rank, _)| *rank).collect::<Vec<_>>(),
        [1, 1, 3, 3, 5, 5]
    );
    assert!(overall.chunks(2).all(|pair| pair[0].1 != pair[1].1));
}

/// The knockout takes the two firsts and one of the seconds, which share their rank: the firsts
/// go on by their place, the seconds are compared, and the lot decides.
#[test]
fn of_parallel_divisions_a_cut_through_a_rank_compares_its_rows() {
    let mut session = groups_and_knockout();
    session.start().unwrap();
    let (groups, knockout) = (session.first(), session.phases()[1].id);
    play_out_by_lot(&mut session, groups);

    let advancing: Vec<_> = (0..2)
        .flat_map(|division| {
            session
                .standings(DivisionRef {
                    phase: groups,
                    division,
                })
                .unwrap()
                .rows
        })
        .map(|row| (row.rank.get(), row.advances))
        .collect();
    let by_place = Some(Advance {
        to: knockout,
        how: Advancing::ByPlace,
    });
    let compared = Some(Advance {
        to: knockout,
        how: Advancing::Compared,
    });
    for (rank, advances) in &advancing {
        match rank {
            1 => assert_eq!(*advances, by_place),
            2 => assert!(*advances == compared || advances.is_none()),
            _ => assert_eq!(*advances, None),
        }
    }
    assert_eq!(advancing.iter().filter(|(_, a)| *a == compared).count(), 1);

    let compared = session.compared(groups);
    assert_eq!(compared.len(), 1);
    assert_eq!(compared[0].rank.get(), 2);
    assert!(compared[0].columns.is_empty(), "compared by nothing");
    let rows: Vec<_> = compared[0]
        .rows
        .iter()
        .map(|row| (row.rank.get(), row.advances.is_some()))
        .collect();
    assert_eq!(rows, [(3, true), (3, false)]);
    assert!(matches!(
        session.start_phase_refusal(knockout),
        Some(StartPhaseError::CutThroughTie(_))
    ));
}

/// Medals are of the competition, so the firsts of two groups going on win none.
#[test]
fn a_phase_of_divisions_gives_medals_by_its_overall_ranks() {
    let mut session = Given::new().entrants(&SIX).divisions(2).started();
    let groups = session.first();
    play_out_by_lot(&mut session, groups);

    let medals: Vec<_> = session
        .overall_standings(groups)
        .unwrap()
        .rows
        .iter()
        .map(|row| row.medal)
        .collect();

    assert_eq!(
        medals,
        [
            Some(Medal::Gold),
            Some(Medal::Gold),
            Some(Medal::Bronze),
            Some(Medal::Bronze),
            None,
            None
        ]
    );
}

#[test]
fn a_phase_of_one_division_compares_nothing() {
    let mut session = Given::new().started();
    let phase = session.first();
    play_out_by_lot(&mut session, phase);

    assert!(session.compared(phase).is_empty());
    assert_eq!(
        session.overall_standings(phase),
        session.standings(session.only())
    );
}
