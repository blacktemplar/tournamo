//! The match format of a phase: what a report has to fit, and what it tallies.

use tournament_engine::{
    CompetitionStateMachine, Decision, EnteredValue, FieldKey, MatchResult, Part,
    ReportMatchResult, ResultReason, Score, Sides,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::common::games::{self, GamesWon};

mod common;

/// The engine refuses only what does not fit what the match format says a result looks like.
#[test]
fn a_report_not_fitting_the_match_format_is_refused() {
    let (mut machine, open) = games::competition_entering_games();
    let report = || ReportMatchResult::new(games::phase_id(), open, MatchResult::AWins);
    let game = || Part::new().with_value(FieldKey::new("points"), games::count(21, 3));

    let misfits = [
        report().with_value(FieldKey::new("goals"), games::count(1, 0)),
        report().with_value(
            FieldKey::new("games"),
            Sides::new(Some(EnteredValue::Decimal(Score::new(1.5))), None),
        ),
        ReportMatchResult::new(
            games::phase_id(),
            open,
            Decision::new(MatchResult::AWins).with_reason(ResultReason::new("walkover")),
        ),
        report()
            .with_part(game())
            .with_part(game())
            .with_part(game())
            .with_part(game()),
        report().with_part(game().with_part(game())),
    ];
    for misfit in misfits {
        assert!(machine.apply_new(misfit.into()).is_err());
    }

    // a result breaking the rules of a sport fits, though: the referee decides
    machine
        .apply_new(
            ReportMatchResult::new(games::phase_id(), open, MatchResult::BWins)
                .with_value(FieldKey::new("games"), games::count(0, 0))
                .with_part(game())
                .into(),
        )
        .unwrap();
}

/// The tallies of a match are what its match format reads of it once it is decided.
#[test]
fn a_decided_match_holds_the_tallies_of_its_rules() {
    let (mut machine, open) = games::competition_entering_games();
    let tally = |machine: &CompetitionStateMachine| {
        machine.current().phases()[&games::phase_id()]
            .data()
            .matches()[&open]
            .tally::<GamesWon>()
            .cloned()
    };
    assert_eq!(tally(&machine), None);
    let game = |winner| {
        Part::new()
            .with_decision(winner)
            .with_value(FieldKey::new("points"), games::count(21, 3))
    };

    machine
        .apply_new(
            ReportMatchResult::new(games::phase_id(), open, MatchResult::BWins)
                .with_part(game(MatchResult::AWins))
                .with_part(game(MatchResult::BWins))
                .with_part(game(MatchResult::BWins))
                .into(),
        )
        .unwrap();

    assert_eq!(tally(&machine), Some(GamesWon(Sides::new(1, 2))));
}
