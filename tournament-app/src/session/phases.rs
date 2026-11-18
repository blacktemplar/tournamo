//! The phases of the competition: what each is, where it is, starting and removing one, who
//! advances, and the ranking of the competition.

use std::{collections::HashMap, num::NonZeroU32};

use plugin_factory::catalog::AnyAxis;
use tournament_engine::{
    AddPhase, CompetitionStateMachineExt as _, CompetitionTransition, Phase, PhaseId,
    PhaseStanding, RankingId, SourceOf, StartPhase,
    plugins::{draw::DivisionOrder, phase_format::PhaseFormat},
    transitions::{RemovePhaseError as EngineRemovePhaseError, StartPhaseError as Engine},
    undoable_state_machine::TransitionId,
};

use super::{CannotStart, Session, Taken, rejection};
use crate::{
    error::{RemovePhaseError, StartPhaseError, Tie},
    view::{
        Advance, Advancing, CompetitionRanking, DivisionOverview, DivisionRef, Medal, PhaseName,
        PhaseOverview, PhaseState, PlacedRow, Plugin, SourceProgress, SourceRow,
    },
};

impl Session {
    /// Every phase, in the order they were added, which is the order they are played in: a phase
    /// takes entrants only from phases before it.
    pub fn phases(&self) -> Vec<PhaseOverview> {
        let competition = self.competition();
        competition
            .phases()
            .values()
            .map(|phase| PhaseOverview {
                id: phase.id(),
                name: self.phase_name(phase),
                sources: phase.sources().iter().map(SourceRow::of).collect(),
                state: self.phase_state(phase),
                configured_divisions: phase.plugins().divisions().get() as usize,
                order: phase.plugins().draw().order(),
                divisions: phase
                    .divisions()
                    .iter()
                    .enumerate()
                    .map(|(index, division)| {
                        let matches = division.data().matches();
                        DivisionOverview {
                            division: DivisionRef {
                                phase: phase.id(),
                                division: index,
                            },
                            played: matches.values().filter(|m| m.is_finished()).count(),
                            matches: matches.len(),
                        }
                    })
                    .collect(),
            })
            .collect()
    }

    /// What the phase `id` is called, see [`PhaseName`]; `None` if there is no such phase.
    pub fn name_of_phase(&self, id: PhaseId) -> Option<PhaseName> {
        self.phase(id).map(|phase| self.phase_name(phase))
    }

    /// What `phase`, a phase of the competition, is called.
    pub(super) fn phase_name(&self, phase: &Phase) -> PhaseName {
        let number = self
            .competition()
            .phases()
            .get_index_of(&phase.id())
            .map_or(0, |index| index + 1);
        named(number, phase.name(), phase.plugins().format())
    }

    /// What the phase the [`AddPhase`] of the log entry `entry` adds is called, the number it has
    /// counting the phases added by the entries before it in effect: also of a phase no longer
    /// added, e.g. as its adding is undone.
    pub(super) fn added_phase_name(&self, entry: TransitionId, added: &AddPhase) -> PhaseName {
        let before = self
            .machine
            .log()
            .iter()
            .take_while(|logged| logged.id() != entry)
            .filter(|logged| {
                matches!(
                    self.machine.effective_transition(logged.id()),
                    Some(CompetitionTransition::AddPhase(_))
                )
            })
            .count();
        named(before + 1, added.name(), added.plugins().format())
    }

    /// The phase shown first when the competition is opened: the first running phase, else the
    /// first that can start, else the last finished, else the first; `None` if there is no phase.
    pub fn shown_first(&self) -> Option<PhaseId> {
        let phases = self.competition().phases();
        let running = phases
            .values()
            .find(|phase| phase.is_started() && !phase.is_finished());
        let can_start = || {
            phases
                .values()
                .find(|phase| self.competition().is_started() && self.can_start(phase))
        };
        let finished = || phases.values().rev().find(|phase| phase.is_finished());
        running
            .or_else(can_start)
            .or_else(finished)
            .or_else(|| phases.values().next())
            .map(Phase::id)
    }

    /// Whether `phase` is not started and every phase it takes entrants from is finished.
    fn can_start(&self, phase: &Phase) -> bool {
        !phase.is_started()
            && !phase.is_fed_by_registration_alone()
            && self.waiting_for(phase).is_empty()
    }

    /// The phases `phase` takes entrants from that are not finished, with their progress.
    fn waiting_for(&self, phase: &Phase) -> Vec<SourceProgress> {
        let mut waiting: Vec<SourceProgress> = Vec::new();
        for source in phase.sources() {
            let SourceOf::Phase(id) = source.of() else {
                continue;
            };
            let Some(from) = self.phase(id) else {
                continue;
            };
            if from.is_finished() || waiting.iter().any(|w| w.phase == id) {
                continue;
            }
            let matches = || {
                from.divisions()
                    .iter()
                    .flat_map(|division| division.data().matches().values())
            };
            waiting.push(SourceProgress {
                phase: id,
                played: matches().filter(|m| m.is_finished()).count(),
                matches: matches().count(),
            });
        }
        waiting
    }

    /// Where `phase` is.
    fn phase_state(&self, phase: &Phase) -> PhaseState {
        if !phase.is_started() {
            return PhaseState::NotStarted {
                waiting_for: self.waiting_for(phase),
            };
        }
        if !phase.is_finished() {
            return PhaseState::Running;
        }
        let started = self.competition().is_started();
        PhaseState::Finished {
            can_start: self
                .competition()
                .phases()
                .values()
                .filter(|next| {
                    started
                        && next
                            .sources()
                            .iter()
                            .any(|source| source.of() == SourceOf::Phase(phase.id()))
                        && self.can_start(next)
                })
                .map(Phase::id)
                .collect(),
        }
    }

    /// Why [`Self::start_phase`] would refuse to start the phase `id` now, or warns of starting
    /// it, see [`StartPhaseError::CutThroughTie`]; `None` if it would start it as it is.
    pub fn start_phase_refusal(&self, id: PhaseId) -> Option<StartPhaseError> {
        let competition = self.competition();
        let Some(phase) = self.phase(id) else {
            return Some(StartPhaseError::NoSuchPhase);
        };
        if phase.is_started() {
            return Some(StartPhaseError::Started);
        }
        if phase.is_fed_by_registration_alone() {
            return Some(StartPhaseError::WithTheCompetition);
        }
        if !competition.is_started() {
            return Some(StartPhaseError::CompetitionNotStarted);
        }
        let waiting = self.waiting_for(phase);
        if !waiting.is_empty() {
            let waiting = waiting
                .into_iter()
                .map(|source| (self.name_of_phase(source.phase), source))
                .filter_map(|(name, source)| Some((name?, source.played, source.matches)))
                .collect();
            return Some(StartPhaseError::Waiting(waiting));
        }
        let field = competition.field_of(&id);
        let taken = Taken {
            qualifiers: field.len(),
            entrants: field.iter().map(|qualifier| qualifier.entrants.len()).sum(),
        };
        match self.cannot_start(phase, taken) {
            Some(CannotStart::Format(needed)) => {
                return Some(StartPhaseError::TooFewForFormat(needed));
            }
            Some(CannotStart::Divisions { divisions, needed }) => {
                return Some(StartPhaseError::TooFewForDivisions { divisions, needed });
            }
            Some(CannotStart::Teams(does_not_fit)) => {
                return Some(StartPhaseError::FieldDoesNotFit(does_not_fit));
            }
            None => {}
        }
        let ties = self.ties_at_cuts(phase);
        (!ties.is_empty()).then_some(StartPhaseError::CutThroughTie(ties))
    }

    /// Starts the phase `id`, taking the entrants of its sources as their standings are now, a
    /// tie at a cut decided by lot (see [`StartPhaseError::CutThroughTie`]).
    ///
    /// # Errors
    /// Returns an error if the phase does not exist, has started, starts with the competition,
    /// the competition has not started, a phase it takes entrants from is not finished, or it
    /// would start with fewer entrants than its format needs.
    pub fn start_phase(&mut self, id: PhaseId) -> Result<(), StartPhaseError> {
        if let Some(refusal) = self.start_phase_refusal(id)
            && !matches!(refusal, StartPhaseError::CutThroughTie(_))
        {
            return Err(refusal);
        }
        self.history
            .apply_new(&mut self.machine, StartPhase::new(id).into())
            .map_err(|error| {
                let error = rejection(error);
                match error.downcast_ref::<Engine>() {
                    Some(Engine::AlreadyStarted) => StartPhaseError::Started,
                    _ => StartPhaseError::Rejected(error),
                }
            })?;
        Ok(())
    }

    /// The ties the cuts of the sources of `phase` go through: for each source of a phase, at its
    /// first and its last place, the rows sharing a rank on both sides of the cut, and of parallel
    /// divisions those sharing a rank in their division, which the overall standings set apart
    /// by its lots.
    fn ties_at_cuts(&self, phase: &Phase) -> Vec<Tie> {
        let mut ties = Vec::new();
        for source in phase.sources() {
            let SourceOf::Phase(from) = source.of() else {
                continue;
            };
            let Some(from_phase) = self.phase(from) else {
                continue;
            };
            let standings = from_phase.standings();
            let rows: Vec<PhaseStanding> = standings.iter().copied().collect();
            let parallel = side_by_side(from_phase);
            let team = |standing: &PhaseStanding| {
                let data = from_phase.divisions()[standing.division()].data();
                self.team(data, &[standing.ranking().id()])
            };
            let cuts = [
                source.from().get().checked_sub(1).and_then(NonZeroU32::new),
                source.to(),
            ];
            for last_above in cuts.into_iter().flatten() {
                let at = (last_above.get() as usize).min(rows.len());
                let (above, below) = rows.split_at(at);
                if let (Some(last), Some(first)) = (above.last(), below.first())
                    && last.rank() == first.rank()
                {
                    let rank = last.rank();
                    let tied = |range: &[PhaseStanding]| -> Vec<_> {
                        range
                            .iter()
                            .filter(|standing| standing.rank() == rank)
                            .map(team)
                            .collect()
                    };
                    ties.push(Tie {
                        phase: self.phase_name(from_phase),
                        division: None,
                        rank,
                        favoured: tied(above),
                        others: tied(below),
                    });
                }
                if !parallel {
                    continue;
                }
                for division in 0..from_phase.divisions().len() {
                    let of = |standing: &&PhaseStanding| standing.division() == division;
                    let (Some(last), Some(first)) = (above.iter().rfind(of), below.iter().find(of))
                    else {
                        continue;
                    };
                    let rank = last.in_division().rank();
                    if first.in_division().rank() != rank {
                        continue;
                    }
                    let tied = |range: &[PhaseStanding]| -> Vec<_> {
                        range
                            .iter()
                            .filter(|standing| {
                                standing.division() == division
                                    && standing.in_division().rank() == rank
                            })
                            .map(team)
                            .collect()
                    };
                    ties.push(Tie {
                        phase: self.phase_name(from_phase),
                        division: Some(division),
                        rank,
                        favoured: tied(above),
                        others: tied(below),
                    });
                }
            }
        }
        ties
    }

    /// Why [`Self::remove_phase`] would refuse to remove the phase `id`, or `None` if it would
    /// remove it.
    pub fn remove_phase_refusal(&self, id: PhaseId) -> Option<RemovePhaseError> {
        let phase = self.phase(id)?;
        if phase.is_started() {
            return Some(RemovePhaseError::Started);
        }
        let users: Vec<_> = self
            .competition()
            .phases()
            .values()
            .filter(|other| {
                other
                    .sources()
                    .iter()
                    .any(|source| source.of() == SourceOf::Phase(id))
            })
            .map(|other| self.phase_name(other))
            .collect();
        (!users.is_empty()).then_some(RemovePhaseError::InUse(users))
    }

    /// Removes the phase `id`, which has not started and from which no phase takes entrants. Undo
    /// brings it back.
    ///
    /// # Errors
    /// Returns an error if there is no such phase, it has started, or other phases take entrants
    /// from it.
    pub fn remove_phase(&mut self, id: PhaseId) -> Result<(), RemovePhaseError> {
        if let Some(refusal) = self.remove_phase_refusal(id) {
            return Err(refusal);
        }
        self.machine
            .remove_phase(&mut self.history, id)
            .map_err(|error| match error {
                EngineRemovePhaseError::NotAdded => RemovePhaseError::NoSuchPhase,
                EngineRemovePhaseError::Started => RemovePhaseError::Started,
                EngineRemovePhaseError::InUse(_) => {
                    unreachable!("a phase in use is refused before")
                }
                EngineRemovePhaseError::Rejected(error) => {
                    RemovePhaseError::Rejected(rejection(error))
                }
            })?;
        Ok(())
    }

    /// Every entrant's place in the competition, as things stand: by the latest started phase
    /// they are in, see [`Competition::ranking`](tournament_engine::Competition::ranking), the
    /// entrants of one ranking together. Final once every phase is finished.
    pub fn ranking(&self) -> CompetitionRanking {
        let competition = self.competition();
        let is_final = competition.is_started()
            && competition
                .phases()
                .values()
                .all(|phase| phase.is_finished());
        let mut rows: Vec<PlacedRow> = Vec::new();
        let mut last_ranking = None;
        for placed in competition.ranking() {
            let entrant = self.entrant_row(&placed.entrant);
            // the entrants of a ranking follow each other, being placed by their ranking
            if let Some(row) = rows.last_mut()
                && last_ranking == Some((placed.phase, placed.ranking))
            {
                row.entrants.push(entrant);
                continue;
            }
            last_ranking = Some((placed.phase, placed.ranking));
            rows.push(PlacedRow {
                place: placed.place,
                entrants: vec![entrant],
                phase: placed.phase,
                medal: is_final.then(|| Medal::of(placed.place)).flatten(),
            });
        }
        CompetitionRanking { rows, is_final }
    }

    /// Where each row of the standings of the phase `phase` goes on to, as they are now: a
    /// function of the ranking of a row, worked out on the overall standings, by unique rank.
    ///
    /// A row advances [`ByPlace`](Advancing::ByPlace) unless the phase has parallel divisions and
    /// rows of its place (unique rank) in other divisions go elsewhere: then it advances
    /// [`Compared`](Advancing::Compared).
    pub(super) fn advancing(&self, phase: PhaseId) -> impl Fn(RankingId) -> Option<Advance> {
        let competition = self.competition();
        // the sources of other phases taking places of this one, with the phase taking them
        let takers: Vec<_> = competition
            .phases()
            .values()
            .flat_map(|taker| {
                taker
                    .sources()
                    .iter()
                    .filter(|source| source.of() == SourceOf::Phase(phase))
                    .map(move |source| (taker.id(), *source))
            })
            .collect();
        let taker_of = move |unique_rank: NonZeroU32| {
            takers
                .iter()
                .find(|(_, source)| source.takes(unique_rank))
                .map(|(taker, _)| *taker)
        };
        // for every ranking, its place in its division where it is compared across parallel
        // divisions, and its overall unique rank
        let mut compared_by: HashMap<RankingId, (Option<NonZeroU32>, NonZeroU32)> = HashMap::new();
        // for every place compared by, the phases its rows go to
        let mut goes_to: HashMap<NonZeroU32, Vec<Option<PhaseId>>> = HashMap::new();
        if let Some(phase) = competition.phase(&phase) {
            let across = side_by_side(phase);
            for row in phase.standings().iter() {
                let by = across.then(|| row.in_division().unique_rank());
                compared_by.insert(row.ranking().id(), (by, row.unique_rank()));
                if let Some(by) = by {
                    let to = taker_of(row.unique_rank());
                    let phases = goes_to.entry(by).or_default();
                    if !phases.contains(&to) {
                        phases.push(to);
                    }
                }
            }
        }
        move |ranking: RankingId| {
            let &(by, unique_rank) = compared_by.get(&ranking)?;
            let to = taker_of(unique_rank)?;
            let how = if by.is_some_and(|by| goes_to[&by].len() > 1) {
                Advancing::Compared
            } else {
                Advancing::ByPlace
            };
            Some(Advance { to, how })
        }
    }
}

/// Whether the phase `phase` has started with several divisions side by side, e.g. groups, rather
/// than one or levels.
pub(super) fn side_by_side(phase: &Phase) -> bool {
    phase.divisions().len() > 1 && phase.plugins().draw().order() == DivisionOrder::Parallel
}

/// What the phase `number` with `name`, played in `format`, is called.
pub(crate) fn named(number: usize, name: Option<&str>, format: &dyn PhaseFormat) -> PhaseName {
    let id = plugin_factory::id_of(format);
    let entry = AnyAxis::of::<dyn PhaseFormat>().entry(id);
    PhaseName {
        number,
        name: name.map(str::to_owned),
        format: Plugin {
            id,
            label: entry.map_or(id, |entry| entry.label()),
            description: entry.and_then(|entry| entry.description()),
        },
    }
}
