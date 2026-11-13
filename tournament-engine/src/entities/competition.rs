use plugin_factory::{compatibility::Declaration, draft::Draft};
use rand::{RngExt, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use vec1::vec1;

use super::seed::Entropy;
use crate::{
    Entrant, EntrantId, Phase, PhaseId, Seed,
    map::IdMap,
    plugins::{PhasePlugins, facts::EntrantsPerRanking},
};

#[derive(Default, Serialize, Deserialize)]
pub struct Competition {
    entrants: IdMap<Entrant>,
    phases: IdMap<Phase>,
    started: bool,
    seed: Seed,
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

    pub(crate) fn add_phase(&mut self, id: PhaseId, plugins: PhasePlugins) {
        let seed = Seed::new(self.seed.rng(id).random());
        self.phases.insert(Phase::new(id, plugins, seed));
    }

    /// Declares the facts that hold for a phase added to this competition before any of its
    /// plugins declare theirs, i.e. the context of its [`PhasePlugins`] for a [`Draft`] and for
    /// [`plugin_factory::compatibility`].
    pub(crate) fn declare_phase_context(&self, declaration: &mut Declaration) {
        // every entrant gets a ranking of its own when the competition starts, see `Self::start`
        declaration.provide(EntrantsPerRanking::SINGLE);
    }

    /// A draft of the plugins of a phase of this competition, with nothing answered yet. Building
    /// [`PhasePlugins`] from it is the way to make them, see their example.
    pub fn phase_draft(&self) -> Draft<PhasePlugins> {
        Draft::new(|declaration| self.declare_phase_context(declaration))
    }

    pub fn phases(&self) -> &IdMap<Phase> {
        &self.phases
    }

    pub(crate) fn phases_mut(&mut self) -> &mut IdMap<Phase> {
        &mut self.phases
    }

    pub fn phase(&self, phase_id: &PhaseId) -> Option<&Phase> {
        self.phases.get(phase_id)
    }

    /// The phase of this competition, once it is added: a competition has a single phase, see
    /// [`AddPhase`](crate::AddPhase).
    pub fn only_phase(&self) -> Option<&Phase> {
        self.phases.values().next()
    }

    fn only_phase_mut(&mut self) -> Option<&mut Phase> {
        self.phases.values_mut().next()
    }

    pub(crate) fn start(&mut self) {
        let mut ranking_entrants: Vec<_> = self.entrants().keys().map(|e| vec1![*e]).collect();
        // Note that keys() is not guaranteed to give a stable ordering. To make sure ordering is
        // stable we sort before shuffling
        ranking_entrants.sort_by_key(|e| e[0].uuid());
        ranking_entrants.shuffle(&mut self.seed.rng(Entropy::CompetitionStart));
        self.started = true;
        self.only_phase_mut()
            .expect("a started competition has its single phase")
            .start(ranking_entrants);
    }
}
