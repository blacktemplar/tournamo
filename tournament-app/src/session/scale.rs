//! How many rounds and matches a phase takes, found by playing it out.

use tournament_engine::{
    AddEntrant, AddPhase, CompetitionStateMachine, Entrant, EntrantId, Match, MatchResult, PhaseId,
    PhasePlugins, ReportMatchResult, Seed, SetCompetitionSeed, StartCompetition,
    plugin_factory::config::{Path, PluginConfig},
    plugins::teams::Keep,
    uuid::Uuid,
};

use super::{MOST_MATCHES, Session, phase::completed, teams_formed, teams_plugin};
use crate::{
    form::PhaseForm,
    i18n::Text,
    view::{Count, Scale},
};

impl Session {
    /// How many entrants the phase `form` configures takes from its sources, as many as there are
    /// now: those registered for the first phase, else those of the places of the phases before
    /// it, as many as their sources take in turn until they start. Each member of a team counts.
    pub fn entrants_taken(&self, form: &PhaseForm) -> usize {
        self.field_taken(form.sources()).entrants
    }

    /// What keeps the phase `form` configures from forming its teams of the places its sources
    /// take, as many as there are now, e.g. an odd number of them into pairs; `None` if it can,
    /// or if the form is not complete.
    pub fn unfit(&self, form: &PhaseForm) -> Option<Text> {
        let taken = self.field_taken(form.sources());
        let configured = completed(form.draft()).ok()?;
        let teams = configured.teams();
        (!teams.fits(taken.qualifiers)).then(|| {
            Text::new("warning-teams-unfit")
                .with("count", taken.qualifiers)
                .with("teams", teams_plugin(teams))
        })
    }

    /// How many rounds and matches the phase `form` configures takes with the entrants its sources
    /// take, as many as there are now, formed into its teams, found by playing it out apart from
    /// this competition, once with every match won by side A and once by side B, which sets how
    /// far the numbers depend on the results; `None` if it does not start with them or takes more
    /// than [`MOST_MATCHES`].
    pub fn scale(&self, form: &PhaseForm) -> Option<Scale> {
        let taken = self.field_taken(form.sources());
        let configured = completed(form.draft()).ok()?;
        let teams = configured.teams();
        if !teams.fits(taken.qualifiers) {
            return None;
        }
        let formed = teams_formed(teams, taken, form.entrants_per_qualifier());
        // played by as many single entrants as there are teams, which take as long
        let mut draft = form.draft().clone();
        draft
            .set(
                &Path::ROOT.field(PhasePlugins::TEAMS),
                PluginConfig::of::<Keep>(),
            )
            .ok()?;
        let plugins = completed(&draft).ok()?;
        let [a, b] = [MatchResult::AWins, MatchResult::BWins]
            .map(|result| play_out(formed, plugins.clone(), result));
        let ((a_rounds, a_matches), (b_rounds, b_matches)) = (a?, b?);
        let count = |a: usize, b: usize| Count {
            least: a.min(b),
            most: a.max(b),
        };
        Some(Scale {
            entrants: taken.entrants,
            teams: formed,
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
    machine.apply_new(StartCompetition::default().into()).ok()?;
    for _ in 0..=MOST_MATCHES {
        let matches = || {
            machine
                .current()
                .phases()
                .get(&phase)
                .into_iter()
                .flat_map(|phase| phase.divisions())
                .flat_map(|division| division.data().matches().values())
        };
        let open = matches().find(|r#match| r#match.result().is_none());
        let Some(open) = open else {
            // the divisions play their rounds side by side
            let rounds = matches().map(Match::round).max();
            let rounds = rounds.map_or(0, |round| round.get() as usize);
            return Some((rounds, matches().count()));
        };
        let report = ReportMatchResult::new(phase, open.id(), result);
        machine.apply_new(report.into()).ok()?;
    }
    None
}
