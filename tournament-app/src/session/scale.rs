//! How many rounds and matches a phase takes, found by playing it out.

use plugin_factory::draft::Draft;
use tournament_engine::{
    AddEntrant, AddPhase, CompetitionStateMachine, CompetitionStateMachineExt as _, Entrant,
    EntrantId, Match, MatchResult, PhaseId, PhasePlugins, ReportMatchResult, Seed,
    SetCompetitionSeed, StartCompetition, uuid::Uuid,
};

use super::{MOST_MATCHES, Session, phase::completed};
use crate::view::{Count, Scale};

impl Session {
    /// How many rounds and matches the phase `draft` configures takes with the entrants entered
    /// now, found by playing it out apart from this competition, once with every match won by
    /// side A and once by side B, which sets how far the numbers depend on the results; `None` if
    /// it does not start with them or takes more than [`MOST_MATCHES`].
    pub fn scale(&self, draft: &Draft<PhasePlugins>) -> Option<Scale> {
        let plugins = completed(draft).ok()?;
        let [a, b] = [MatchResult::AWins, MatchResult::BWins]
            .map(|result| self.play_out(plugins.clone(), result));
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

    /// The rounds and matches of a phase of `plugins` with as many entrants as this competition,
    /// every match ending in `result`, see [`Self::scale`].
    fn play_out(&self, plugins: PhasePlugins, result: MatchResult) -> Option<(usize, usize)> {
        let mut machine = CompetitionStateMachine::default();
        // any seed, since what a format draws does not change how long it takes
        let seed = SetCompetitionSeed::new(Seed::new([0; 12]));
        machine.apply_transition(seed).ok()?;
        for index in 0..self.competition().entrants().len() {
            let id = EntrantId::new(Uuid::from_u128(index as u128 + 1));
            machine
                .apply_transition(AddEntrant::new(Entrant::new(id)))
                .ok()?;
        }
        let phase = PhaseId::new(Uuid::nil());
        machine
            .apply_transition(AddPhase::new(phase, plugins))
            .ok()?;
        machine.apply_transition(StartCompetition).ok()?;
        for _ in 0..=MOST_MATCHES {
            let data = machine.current().phases().get(&phase)?.data();
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
            machine.apply_transition(report).ok()?;
        }
        None
    }
}
