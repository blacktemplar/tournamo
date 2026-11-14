use std::num::NonZeroU32;

use plugin_factory::{compatibility::Declaration, draft::Draft};
use rand::{RngExt, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use vec1::{Vec1, vec1};

use super::seed::Entropy;
use crate::{
    Entrant, EntrantId, MatchId, Origin, Part, Phase, PhaseId, Qualifier, RankingId, Seed, Source,
    SourceOf,
    map::{IdMap, Map},
    plugins::{PhasePlugins, facts::EntrantsPerRanking},
};

/// A competition: its entrants, its phases, and the random seed everything random in it derives
/// from.
#[derive(Default, Serialize, Deserialize)]
pub struct Competition {
    entrants: IdMap<Entrant>,
    phases: IdMap<Phase>,
    started: bool,
    seed: Seed,
    /// The entrants by lot, best first; empty until the competition starts.
    lots: Vec<EntrantId>,
    /// The highest number of a match so far, or kept for the matches of a division, 0 for none.
    highest_number: u32,
}

/// Where an entrant is placed in the competition, see [`Competition::ranking`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Placed {
    pub entrant: EntrantId,
    /// Shared by entrants placed equal.
    pub place: NonZeroU32,
    /// The latest started phase the entrant is in, which places them.
    pub phase: PhaseId,
    /// The ranking of the entrant in that phase.
    pub ranking: RankingId,
}

impl Competition {
    pub fn is_started(&self) -> bool {
        self.started
    }

    pub fn entrants(&self) -> &IdMap<Entrant> {
        &self.entrants
    }

    pub(crate) fn add_entrant(&mut self, entrant: Entrant) {
        self.entrants.insert(entrant);
    }

    pub(crate) fn remove_entrant(&mut self, entrant_id: &EntrantId) {
        self.entrants.remove(entrant_id);
    }

    pub(crate) fn set_seed(&mut self, seed: Seed) {
        self.seed = seed;
    }

    pub fn seed(&self) -> Seed {
        self.seed
    }

    /// The registration: every entrant, ordered by lot, drawn when the competition starts; empty
    /// before. The lot breaks every tie nothing else breaks, see
    /// [`Ranking::lot`](crate::Ranking::lot).
    pub fn lots(&self) -> &[EntrantId] {
        &self.lots
    }

    /// The lot of `entrant`, i.e. their place in [`Self::lots`]; `None` before the start.
    pub fn lot(&self, entrant: &EntrantId) -> Option<NonZeroU32> {
        self.lots
            .iter()
            .position(|e| e == entrant)
            .map(|index| NonZeroU32::new(index as u32 + 1).expect("one more than an index"))
    }

    pub(crate) fn add_phase(
        &mut self,
        id: PhaseId,
        name: Option<String>,
        sources: Vec1<Source>,
        plugins: PhasePlugins,
    ) {
        let seed = Seed::new(self.seed.rng(id).random());
        self.phases
            .insert(Phase::new(id, name, sources, plugins, seed));
    }

    /// Declares the facts that hold for a phase added to this competition before any of its
    /// plugins declare theirs, i.e. the context of its [`PhasePlugins`] for a [`Draft`] and for
    /// [`plugin_factory::compatibility`].
    pub(crate) fn declare_phase_context(&self, declaration: &mut Declaration) {
        // every entrant gets a ranking of its own when the competition starts, see `Self::start`,
        // and every source passes on the rankings it takes
        declaration.provide(EntrantsPerRanking::SINGLE);
    }

    /// A draft of the plugins of a phase of this competition, with nothing answered yet. Building
    /// [`PhasePlugins`] from it is the way to make them, see their example.
    pub fn phase_draft(&self) -> Draft<PhasePlugins> {
        Draft::new(|declaration| self.declare_phase_context(declaration))
    }

    /// The phases, in the order they were added in; a phase takes entrants only from phases
    /// before it.
    pub fn phases(&self) -> &IdMap<Phase> {
        &self.phases
    }

    pub fn phase(&self, phase_id: &PhaseId) -> Option<&Phase> {
        self.phases.get(phase_id)
    }

    /// The first phase of this competition, once one is added.
    pub fn only_phase(&self) -> Option<&Phase> {
        self.phases.values().next()
    }

    /// Draws the lots and starts every phase fed by the registration alone.
    pub(crate) fn start(&mut self) {
        let mut lots: Vec<_> = self.entrants().keys().copied().collect();
        // Note that keys() is not guaranteed to give a stable ordering. To make sure ordering is
        // stable we sort before shuffling
        lots.sort_by_key(|e| e.uuid());
        lots.shuffle(&mut self.seed.rng(Entropy::CompetitionStart));
        self.lots = lots;
        self.started = true;
        let starting: Vec<_> = self
            .phases
            .values()
            .filter(|phase| phase.is_fed_by_registration_alone())
            .map(Phase::id)
            .collect();
        for phase_id in starting {
            self.start_phase(phase_id);
        }
    }

    /// Starts the phase `phase_id` with its field, see [`Self::field_of`].
    ///
    /// # Panics
    /// Panics if there is no such phase.
    pub(crate) fn start_phase(&mut self, phase_id: PhaseId) {
        let field = self
            .field_of(&phase_id)
            .into_iter()
            .map(|qualifier| {
                let lot = qualifier
                    .entrants
                    .iter()
                    .map(|entrant| self.lot(entrant).expect("every entrant has a lot"))
                    .min()
                    .expect("a qualifier has entrants");
                (qualifier.entrants, lot)
            })
            .collect();
        self.phases
            .get_mut(&phase_id)
            .expect("the phase exists")
            .start(field, &mut self.highest_number);
    }

    /// The field of the phase `phase_id`, what it starts with as things stand: its sources in
    /// order, each the rows of its range best first, as many as there are. A phase that has not
    /// started has no rows. Empty if there is no such phase.
    pub fn field_of(&self, phase_id: &PhaseId) -> Vec<Qualifier> {
        let Some(phase) = self.phase(phase_id) else {
            return Vec::new();
        };
        let mut field = Vec::new();
        for source in phase.sources() {
            match source.of() {
                SourceOf::Registration => {
                    field.extend(
                        self.lots
                            .iter()
                            .zip(NonZeroU32::MIN..)
                            .filter(|&(_, place)| source.takes(place))
                            .map(|(entrant, place)| Qualifier {
                                entrants: vec1![*entrant],
                                origin: Origin::Registration { place },
                            }),
                    );
                }
                SourceOf::Phase(from) => {
                    let Some(from) = self.phase(&from) else {
                        continue;
                    };
                    let division = 0;
                    field.extend(
                        from.standings()
                            .iter()
                            .filter(|standing| source.takes(standing.unique_rank()))
                            .map(|standing| Qualifier {
                                entrants: standing.ranking().entrants().clone(),
                                origin: Origin::Phase {
                                    phase: from.id(),
                                    division,
                                    rank: standing.rank(),
                                    unique_rank: standing.unique_rank(),
                                    overall_rank: standing.rank(),
                                    overall_unique_rank: standing.unique_rank(),
                                },
                            }),
                    );
                }
            }
        }
        field
    }

    /// Finishes the match `match_id` of the phase `phase_id` with what was `entered` for it.
    ///
    /// # Panics
    /// Panics if there is no such phase, or as [`Phase::report_match`] does.
    pub(crate) fn report_match(&mut self, phase_id: PhaseId, match_id: MatchId, entered: Part) {
        self.phases
            .get_mut(&phase_id)
            .expect("the phase exists")
            .report_match(match_id, entered, &mut self.highest_number);
    }

    /// The place in the competition every entrant of a started phase has as things stand, best
    /// first. An entrant is placed by the latest started phase they are in, at the place its
    /// overall standings start at plus their rank there: a phase starts at the smallest place its
    /// sources start at, a source of the registration at its first place, a source of a phase at
    /// that phase's start plus its first place, less one.
    ///
    /// Final once every phase is finished. Sources leaving places between them that no phase
    /// takes leave gaps between the places; entrants of such places are placed by the phase they
    /// stopped in, and entrants in no started phase are not placed at all.
    pub fn ranking(&self) -> Vec<Placed> {
        let mut starts: Map<PhaseId, u32> = Map::default();
        // (place, unique place, index of the phase, placed) of every entrant placed so far
        let mut placed: Map<EntrantId, (u32, u32, usize, Placed)> = Map::default();
        for (index, phase) in self.phases.values().enumerate() {
            let start = phase
                .sources()
                .iter()
                .map(|source| {
                    let of = match source.of() {
                        SourceOf::Registration => 1,
                        SourceOf::Phase(from) => starts.get(&from).copied().unwrap_or(1),
                    };
                    of + source.from().get() - 1
                })
                .min()
                .expect("a phase has a source");
            starts.insert(phase.id(), start);
            for standing in phase.standings().iter() {
                let place = start + standing.rank().get() - 1;
                for entrant in standing.ranking().entrants() {
                    placed.insert(
                        *entrant,
                        (
                            place,
                            start + standing.unique_rank().get() - 1,
                            index,
                            Placed {
                                entrant: *entrant,
                                place: NonZeroU32::new(place).expect("a place is at least 1"),
                                phase: phase.id(),
                                ranking: standing.ranking().id(),
                            },
                        ),
                    );
                }
            }
        }
        let mut placed: Vec<_> = placed.into_values().collect();
        placed.sort_by_key(|&(place, unique, index, _)| (place, unique, index));
        placed.into_iter().map(|(.., placed)| placed).collect()
    }
}
