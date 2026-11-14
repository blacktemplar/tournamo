//! How many rounds and matches a phase takes, found by playing it out.

use tournament_engine::{
    AddEntrant, AddPhase, CompetitionStateMachine, Entrant, EntrantId, Match, MatchResult, PhaseId,
    PhasePlugins, ReportMatchResult, Seed, SetCompetitionSeed, StartCompetition, uuid::Uuid,
};

use super::{MOST_MATCHES, Session, phase::completed};
use crate::{
    form::PhaseForm,
    view::{Count, Scale},
};

impl Session {
    /// How many entrants the phase `form` configures takes from its sources, as many as there are
    /// now: those registered for the first phase, else the places of the phases before it, as
    /// many as their sources take in turn until they start.
    pub fn entrants_taken(&self, form: &PhaseForm) -> usize {
        self.places_taken(form.sources())
    }

    /// How many rounds and matches the phase `form` configures takes with the entrants its sources
    /// take, as many as there are now, found by playing it out apart from this competition, once
    /// with every match won by side A and once by side B, which sets how far the numbers depend on
    /// the results; `None` if it does not start with them or takes more than [`MOST_MATCHES`].
    pub fn scale(&self, form: &PhaseForm) -> Option<Scale> {
        let entrants = self.entrants_taken(form);
        let plugins = completed(form.draft()).ok()?;
        let [a, b] = [MatchResult::AWins, MatchResult::BWins]
            .map(|result| play_out(entrants, plugins.clone(), result));
        let ((a_rounds, a_matches), (b_rounds, b_matches)) = (a?, b?);
        let count = |a: usize, b: usize| Count {
            least: a.min(b),
            most: a.max(b),
        };
        Some(Scale {
            rounds: count(a_rounds, b_rounds),
            matches: count(a_matches, b_matches),
        })
    }
}

/// The rounds and matches of a phase of `plugins` with `entrants` entrants, every match ending in
/// `result`, see [`Session::scale`].
fn play_out(entrants: usize, plugins: PhasePlugins, result: MatchResult) -> Option<(usize, usize)> {
    let mut machine = CompetitionStateMachine::default();
    // any seed, since what a format draws does not change how long it takes
    let seed = SetCompetitionSeed::new(Seed::new([0; 12]));
    machine.apply_new(seed.into()).ok()?;
    for index in 0..entrants {
        let id = EntrantId::new(Uuid::from_u128(index as u128 + 1));
        machine
            .apply_new(AddEntrant::new(Entrant::new(id)).into())
            .ok()?;
    }
    let phase = PhaseId::new(Uuid::nil());
    machine
        .apply_new(AddPhase::new(phase, plugins).into())
        .ok()?;
    machine.apply_new(StartCompetition.into()).ok()?;
    for _ in 0..=MOST_MATCHES {
        let data = machine
            .current()
            .phases()
            .get(&phase)?
            .divisions()
            .first()?
            .data();
        let open = data
            .matches()
            .values()
            .find(|r#match| r#match.result().is_none());
        let Some(open) = open else {
            let rounds = data.matches().values().map(Match::round).max();
            let rounds = rounds.map_or(0, |round| round.get() as usize);
            return Some((rounds, data.matches().len()));
        };
        let report = ReportMatchResult::new(phase, open.id(), result);
        machine.apply_new(report.into()).ok()?;
    }
    None
}
