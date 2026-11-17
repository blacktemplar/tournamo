use tournament_engine::plugins::{
    phase_format::SingleElimination,
    teams::{BestWithWorst, RandomPairs, Split},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::{SourcesError, StartPhaseError},
    form::End,
    view::Count,
};

const SIX: [&str; 6] = ["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"];

/// Every match of the phase `phase` played, each won by side A.
fn play_out(session: &mut Session, phase: PhaseId) {
    while let Some(open) = session.playable(MatchesOf::phase(phase)).first().cloned() {
        session.report(phase, open.id, MatchResult::AWins).unwrap();
    }
}

/// The names of each row of the standings of the only division of the first phase.
fn standings(session: &Session) -> Vec<Vec<String>> {
    session
        .standings(session.only())
        .unwrap()
        .rows
        .iter()
        .map(|row| {
            names(&row.entrants)
                .into_iter()
                .map(str::to_owned)
                .collect()
        })
        .collect()
}

#[test]
fn drawn_pairs_are_named_by_both_and_found_by_either() {
    let mut session = Given::new()
        .entrants(&["A1", "B1", "C1", "D1", "E1", "F1", "G1", "H1"])
        .teams::<RandomPairs>()
        .format(SingleElimination::ID)
        .started();

    let pairs = standings(&session);
    assert_eq!(pairs.len(), 4);
    assert!(pairs.iter().all(|pair| pair.len() == 2));
    let first = &matches(&session)[0];
    // the second member of side A names its team
    let partner = first.a[1].name.clone().unwrap();
    let reported = session
        .report_entered(session.all(), &partner)
        .expect("a team is found by either name");

    assert_eq!(reported.row.id, first.id);
    assert_eq!(reported.row.result, Some(MatchResult::AWins));
}

#[test]
fn a_field_that_cannot_be_formed_into_pairs_is_not_started() {
    let mut session = Given::new()
        .entrants(&SIX[..5])
        .teams::<RandomPairs>()
        .added();

    let Some(StartError::FieldDoesNotFit(does_not_fit)) = session.start_refusal() else {
        panic!("the start is refused for the pairs");
    };
    assert_eq!(does_not_fit.entrants, 5);
    assert_eq!(does_not_fit.teams.id, RandomPairs::ID);
    assert_eq!(
        crate::i18n::english(&StartError::FieldDoesNotFit(does_not_fit)),
        "1 · Round robin cannot form its teams (drawn pairs) of 5 entrants, e.g. an odd \
         number into pairs"
    );
    assert!(matches!(
        session.start(),
        Err(StartError::FieldDoesNotFit(_))
    ));
    session.add_entrant("Frank").unwrap();
    session.start().unwrap();
}

/// Four pairs are too few for two groups of a format needing four teams each.
#[test]
fn too_few_counts_the_teams() {
    let session = Given::new()
        .entrants(&["A", "B", "C", "D", "E", "F", "G", "H"])
        .format(MonsterDyp::ID)
        .divisions(2)
        .added();
    assert!(session.start_refusal().is_none(), "eight single entrants");

    let session = Given::new()
        .entrants(&["A", "B", "C", "D", "E", "F", "G", "H"])
        .teams::<BestWithWorst>()
        .format(SingleElimination::ID)
        .divisions(3)
        .added();
    assert!(matches!(
        session.start_refusal(),
        Some(StartError::TooFewForDivisions {
            divisions: 3,
            needed: 6
        })
    ));
}

#[test]
fn the_scale_of_a_phase_of_pairs_counts_the_pairs() {
    let session = Given::new()
        .entrants(&["A", "B", "C", "D", "E", "F", "G", "H"])
        .teams::<RandomPairs>()
        .added();
    let form = session.phase_form_to_edit(session.first()).unwrap();

    let scale = session.scale(&form).unwrap();

    assert_eq!(scale.rounds, Count { least: 3, most: 3 });
    assert_eq!(scale.matches, Count { least: 6, most: 6 });
}

/// A phase of pairs, then a Monster DYP of them split up: the form of the Monster DYP follows its
/// sources, so it fits only once the pairs are split up, and sources sending teams of different
/// sizes are named.
#[test]
fn the_form_of_a_later_phase_follows_the_teams_its_sources_send() {
    let mut session = Session::new("cup");
    for name in SIX.iter().chain(&["Grace", "Heidi"]) {
        session.add_entrant(name).unwrap();
    }
    // the pairs of the registration from place 3, the best two left for the Monster DYP
    let mut pairs = session.phase_form().unwrap();
    pairs.take_some_places();
    pairs.enter_place(0, End::From, "3");
    pairs
        .choose(&ConfigPath::ROOT.field("teams"), Some(RandomPairs::ID))
        .unwrap();
    session.add_phase(&pairs).unwrap();
    let mut form = session.phase_form().unwrap();
    form.choose(&ConfigPath::ROOT.field("format"), Some(MonsterDyp::ID))
        .unwrap();
    assert!(!form.view().complete, "a Monster DYP of pairs");

    form.choose(&ConfigPath::ROOT.field("teams"), Some(Split::ID))
        .unwrap();
    assert!(form.view().complete, "{:?}", form.view().issues);

    form.add_source();
    form.choose_source(1, None);
    form.enter_place(1, End::To, "2");
    assert!(!form.view().complete, "single entrants and pairs");
    assert!(matches!(
        session.add_phase(&form),
        Err(crate::error::AddPhaseError::Sources(
            SourcesError::MixedTeamSizes
        ))
    ));

    form.remove_source(1);
    let second = session.add_phase(&form).unwrap();
    session.start().unwrap();
    let first = session.first();
    play_out(&mut session, first);
    assert!(session.start_phase_refusal(second).is_none());
    session.start_phase(second).unwrap();
    let singles = session.standings(DivisionRef::first(second)).unwrap().rows;
    assert_eq!(singles.len(), 6);
    assert!(singles.iter().all(|row| row.entrants.len() == 1));
}

#[test]
fn a_later_phase_of_an_odd_field_is_not_started() {
    let mut session = Given::new().entrants(&SIX[..5]).added();
    let mut form = session.phase_form().unwrap();
    form.choose(&ConfigPath::ROOT.field("teams"), Some(BestWithWorst::ID))
        .unwrap();
    let second = session.add_phase(&form).unwrap();
    session.start().unwrap();
    let first = session.first();
    play_out(&mut session, first);

    assert!(matches!(
        session.start_phase_refusal(second),
        Some(StartPhaseError::FieldDoesNotFit(_))
    ));
    assert!(matches!(
        session.start_phase(second),
        Err(StartPhaseError::FieldDoesNotFit(_))
    ));
}

/// The ranking of the competition lists the members of a team in one row.
#[test]
fn the_ranking_lists_a_team_in_one_row() {
    let mut session = Given::new().entrants(&SIX).teams::<RandomPairs>().started();
    let first = session.first();
    play_out(&mut session, first);

    let ranking = session.ranking();

    assert_eq!(ranking.rows.len(), 3);
    assert!(ranking.rows.iter().all(|row| row.entrants.len() == 2));
}
