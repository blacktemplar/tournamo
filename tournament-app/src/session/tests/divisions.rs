//! Phases of several divisions: listing, filtering and reporting their matches, their standings,
//! and who advances from them.

use std::collections::HashSet;

use tournament_engine::plugins::draw::DivisionOrder;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::{StartError, StartPhaseError},
    form::{Editable as _, End},
    view::{Advance, Advancing, Cut, division_letter},
};

const SIX: [&str; 6] = ["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"];

/// Six entrants in two groups of three, then a knockout of the places 1–3 of the groups, not
/// started.
fn groups_and_knockout() -> Session {
    with_knockout(Given::new().entrants(&SIX).divisions(2))
}

/// The session `groups` builds, of six entrants, with a knockout of the places 1–3 of its phase,
/// not started.
fn with_knockout(groups: Given) -> Session {
    let mut session = groups.added();
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

/// The tables of the groups show what the groups are ranked by, the rows compared across them what
/// they are compared by, each with what that is calculated from.
#[test]
fn each_table_shows_what_it_is_ranked_by_and_what_that_is_calculated_from() {
    let mut session = with_knockout(
        Given::new()
            .entrants(&SIX)
            .divisions(2)
            .across::<MorePointsPerMatch>(),
    );
    session.start().unwrap();
    let groups = session.first();
    play_out_by_lot(&mut session, groups);
    let headers =
        |columns: &[Column]| -> Vec<&str> { columns.iter().map(|column| column.header).collect() };

    let group = session
        .standings(DivisionRef {
            phase: groups,
            division: 0,
        })
        .unwrap();
    let compared = session.compared(groups);

    assert_eq!(
        headers(&group.columns),
        ["points", "Sonneborn-Berger", "W-D-L"]
    );
    assert_eq!(headers(&compared[0].columns), ["P/M", "W-D-L", "points"]);
    let ranks: Vec<bool> = compared[0]
        .columns
        .iter()
        .map(|column| column.ranks)
        .collect();
    assert_eq!(ranks, [true, false, false]);
}

const TWELVE: [&str; 12] = [
    "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi", "Ivan", "Judy", "Mallory",
    "Niaj",
];

/// Twelve entrants in three groups of four, then a knockout of the places 1–8 of the groups,
/// started: two of every group go on by their place, and two of the three thirds.
fn three_groups_and_a_knockout_of_eight() -> (Session, PhaseId, PhaseId) {
    let mut session = Given::new().entrants(&TWELVE).divisions(3).added();
    let mut knockout = session.phase_form().unwrap();
    knockout
        .choose(
            &ConfigPath::ROOT.field("format"),
            Some(SingleElimination::ID),
        )
        .unwrap();
    knockout.enter_place(0, End::To, "8");
    session.add_phase(&knockout).unwrap();
    session.start().unwrap();
    let (groups, knockout) = (session.first(), session.phases()[1].id);
    (session, groups, knockout)
}

/// How each row of each group of the phase `groups` goes on, by group, best first.
fn advancing_by_group(session: &Session, groups: PhaseId) -> Vec<Vec<Option<Advancing>>> {
    (0..3)
        .map(|division| {
            session
                .standings(DivisionRef {
                    phase: groups,
                    division,
                })
                .unwrap()
                .rows
                .iter()
                .map(|row| row.advances.map(|advance| advance.how))
                .collect()
        })
        .collect()
}

/// Every group has two places of the eight however its standings are, the thirds are compared
/// across the groups for the rest.
#[test]
fn of_parallel_divisions_each_takes_its_share_of_a_cut_by_place() {
    let (mut session, groups, _) = three_groups_and_a_knockout_of_eight();
    play_out_by_lot(&mut session, groups);

    let by_place = Some(Advancing::ByPlace);
    let compared = Some(Advancing::Compared);
    for group in advancing_by_group(&session, groups) {
        assert_eq!(group[..2], [by_place, by_place]);
        assert!(group[2] == compared || group[2].is_none());
        assert_eq!(group[3], None);
    }
    let compared = session.compared(groups);
    assert_eq!(compared.len(), 1);
    assert_eq!(compared[0].rank.get(), 3);
    assert_eq!(compared[0].rows.len(), 3);
}

/// Every match drawn, everyone of a group is tied, yet its places decide as ever, the lot of each
/// group having set them apart: the start is warned of the ties in the groups.
#[test]
fn of_parallel_divisions_a_cut_through_a_tie_in_a_group_is_decided_by_its_lot() {
    let (mut session, groups, knockout) = three_groups_and_a_knockout_of_eight();
    while let Some(open) = session.playable(MatchesOf::phase(groups)).first().cloned() {
        session.report(groups, open.id, MatchResult::Draw).unwrap();
    }

    let by_place = Some(Advancing::ByPlace);
    let compared = Some(Advancing::Compared);
    for group in advancing_by_group(&session, groups) {
        assert_eq!(group[..2], [by_place, by_place]);
        assert!(group[2] == compared || group[2].is_none());
        assert_eq!(group[3], None);
    }
    let Some(StartPhaseError::CutThroughTie(ties)) = session.start_phase_refusal(knockout) else {
        panic!("the cut goes through the ties in the groups");
    };
    let mut divisions: Vec<_> = ties.iter().filter_map(|tie| tie.division).collect();
    divisions.sort_unstable();
    assert_eq!(divisions, [0, 1, 2]);
    assert!(
        ties.iter()
            .filter(|tie| tie.division.is_some())
            .all(|tie| { tie.rank.get() == 1 && tie.favoured.len() + tie.others.len() == 4 })
    );
}

/// A line follows the places going on by their place, and one the place going on for now by
/// the comparison; in the comparison, one follows the rows going on.
#[test]
fn a_line_follows_the_rows_going_on() {
    let (mut session, groups, knockout) = three_groups_and_a_knockout_of_eight();
    play_out_by_lot(&mut session, groups);

    let cut = |how, from, through| {
        Some(Cut {
            to: knockout,
            how,
            from: NonZeroU32::new(from).unwrap(),
            through: NonZeroU32::new(through).unwrap(),
        })
    };
    let mut thirds_going_on = 0;
    for division in 0..3 {
        let cuts: Vec<_> = session
            .standings(DivisionRef {
                phase: groups,
                division,
            })
            .unwrap()
            .rows
            .iter()
            .map(|row| row.cut)
            .collect();
        assert_eq!(cuts[..2], [None, cut(Advancing::ByPlace, 1, 2)]);
        assert_eq!(cuts[3], None);
        if cuts[2].is_some() {
            assert_eq!(cuts[2], cut(Advancing::Compared, 3, 3));
            thirds_going_on += 1;
        }
    }
    assert_eq!(thirds_going_on, 2);
    let compared: Vec<_> = session.compared(groups)[0]
        .rows
        .iter()
        .map(|row| row.cut)
        .collect();
    assert_eq!(compared, [None, cut(Advancing::Compared, 3, 3), None]);
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

/// Three entrants are too few for two groups of a round robin, which needs two in each: the start
/// is refused until the groups are lowered, rather than played in fewer groups than configured.
#[test]
fn a_start_is_refused_with_too_few_entrants_for_the_groups() {
    let mut session = Given::new().entrants(&SIX[..3]).divisions(2).added();

    assert!(matches!(
        session.start_refusal(),
        Some(StartError::TooFewForDivisions {
            divisions: 2,
            needed: 4
        })
    ));
    assert!(matches!(
        session.start(),
        Err(StartError::TooFewForDivisions { .. })
    ));
    assert!(!session.competition().is_started());

    let phase = session.first();
    let mut form = session.phase_form_to_edit(phase).unwrap();
    form.enter(&ConfigPath::ROOT.field("divisions"), "1")
        .unwrap();
    session.edit_phase(phase, &form).unwrap();
    session.start().unwrap();
}

/// A further phase whose sources take too few for its groups is refused its start likewise.
#[test]
fn a_phase_is_refused_its_start_with_too_few_entrants_for_its_groups() {
    let mut session = Given::new().entrants(&SIX).added();
    let mut groups = session.phase_form().unwrap();
    groups.enter_place(0, End::To, "3");
    groups
        .enter(&ConfigPath::ROOT.field("divisions"), "2")
        .unwrap();
    session.add_phase(&groups).unwrap();
    session.start().unwrap();
    let (first, second) = (session.first(), session.phases()[1].id);
    play_out_by_lot(&mut session, first);

    assert!(matches!(
        session.start_phase(second),
        Err(StartPhaseError::TooFewForDivisions {
            divisions: 2,
            needed: 4
        })
    ));
}
