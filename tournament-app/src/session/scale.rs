//! How many rounds and matches a phase takes, found by playing it out.

use tournament_engine::{
    AddEntrant, AddPhase, CompetitionStateMachine, Entrant, EntrantId, EntryMeta, Match,
    MatchResult, PhaseId, PhasePlugins, ReportMatchResult, Seed, SetCompetitionSeed,
    StartCompetition, Timestamp,
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

    /// What keeps the phase `form` configures from starting with the places its sources take, as
    /// many as there are now: its teams cannot be formed of them, e.g. an odd number of them into
    /// pairs, or they are too few for its groups, saying how many groups they make at most;
    /// `None` if neither, or if the form is not complete or holds what a field did not take.
    pub fn unfit(&self, form: &PhaseForm) -> Option<Text> {
        if !form.all_taken() {
            return None;
        }
        let taken = self.field_taken(form.sources());
        let configured = completed(form.draft()).ok()?;
        let teams = configured.teams();
        if !teams.fits(taken.qualifiers) {
            return Some(
                Text::new("warning-teams-unfit")
                    .with("count", taken.qualifiers)
                    .with("teams", teams_plugin(teams)),
            );
        }
        let formed = teams_formed(teams, taken, form.entrants_per_qualifier());
        let per_division = configured.format().min_entrants().max(1);
        let divisions = configured.divisions().get() as usize;
        // too few for the format alone is said once the start is refused
        (formed >= per_division && formed < per_division.saturating_mul(divisions)).then(|| {
            Text::new("warning-too-many-groups")
                .with("divisions", divisions)
                .with("count", per_division.saturating_mul(divisions))
                .with("teams", formed)
                .with("most", formed / per_division)
        })
    }

    /// How many rounds and matches the phase `form` configures takes with the entrants its sources
    /// take, as many as there are now, formed into its teams, found by playing it out apart from
    /// this competition, once with every match won by side A and once by side B, which sets how
    /// far the numbers depend on the results; `None` if it does not start with them, or the form
    /// holds what a field did not take, since its draft is then not what it shows. Beyond
    /// [`MOST_MATCHES`] it is not played out further: the counts are then those scheduled until
    /// then, with no most.
    pub fn scale(&self, form: &PhaseForm) -> Option<Scale> {
        if !form.all_taken() {
            return None;
        }
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
        let (a, b) = (a?, b?);
        let finished = a.finished && b.finished;
        let count = |a: usize, b: usize| Count {
            least: a.min(b),
            most: finished.then(|| a.max(b)),
        };
        Some(Scale {
            entrants: taken.entrants,
            teams: formed,
            rounds: count(a.rounds, b.rounds),
            matches: count(a.matches, b.matches),
        })
    }
}

/// How far a phase was played out.
struct Played {
    rounds: usize,
    matches: usize,
    /// Whether it was played to its end, rather than stopped after [`MOST_MATCHES`].
    finished: bool,
}

/// The rounds and matches of a phase of `plugins` with `entrants` entrants, every match ending in
/// `result`, see [`Session::scale`].
fn play_out(entrants: usize, plugins: PhasePlugins, result: MatchResult) -> Option<Played> {
    let mut machine = CompetitionStateMachine::default();
    // any time, since the competition is never saved
    let meta = EntryMeta::recorded_at(Timestamp::from_unix_millis(0));
    // any seed, since what a format draws does not change how long it takes
    let seed = SetCompetitionSeed::new(Seed::new([0; 12]));
    machine.apply_new(seed.into(), meta).ok()?;
    for index in 0..entrants {
        let id = EntrantId::new(Uuid::from_u128(index as u128 + 1));
        machine
            .apply_new(AddEntrant::new(Entrant::new(id)).into(), meta)
            .ok()?;
    }
    let phase = PhaseId::new(Uuid::nil());
    machine
        .apply_new(AddPhase::new(phase, plugins).into(), meta)
        .ok()?;
    machine
        .apply_new(StartCompetition::default().into(), meta)
        .ok()?;
    for _ in 0..MOST_MATCHES {
        let open = matches_of(&machine, phase).find(|r#match| r#match.result().is_none());
        let Some(open) = open else {
            return Some(played(&machine, phase, true));
        };
        let report = ReportMatchResult::new(phase, open.id(), result);
        machine.apply_new(report.into(), meta).ok()?;
    }
    let finished = matches_of(&machine, phase).all(|r#match| r#match.result().is_some());
    Some(played(&machine, phase, finished))
}

/// The matches of the phase `phase` of `machine`, of all its divisions.
fn matches_of(machine: &CompetitionStateMachine, phase: PhaseId) -> impl Iterator<Item = &Match> {
    machine
        .current()
        .phases()
        .get(&phase)
        .into_iter()
        .flat_map(|phase| phase.divisions())
        .flat_map(|division| division.data().matches().values())
}

/// How far the phase `phase` of `machine` was played out.
fn played(machine: &CompetitionStateMachine, phase: PhaseId, finished: bool) -> Played {
    // the divisions play their rounds side by side
    let rounds = matches_of(machine, phase).map(Match::round).max();
    Played {
        rounds: rounds.map_or(0, |round| round.get() as usize),
        matches: matches_of(machine, phase).count(),
        finished,
    }
}
