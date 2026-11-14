use std::num::NonZeroU32;

use plugin_factory::config::Config;
#[cfg(doc)]
use plugin_factory::draft::Draft;
use serde::{Deserialize, Deserializer, Serialize};
use vec1::Vec1;

#[cfg(doc)]
use crate::Competition;
use crate::{
    Division, EntrantId, MatchId, Part, PhaseId, Seed, Source, Standings, map::IdAble,
    plugins::PhasePlugins,
};

/// A phase of a competition: where its entrants come from, one configuration of plugins, played
/// once it is started by its divisions, each with its own copy of the plugins.
#[derive(Debug, Serialize)]
pub struct Phase {
    id: PhaseId,
    name: Option<String>,
    /// In the order the field of the phase takes them.
    sources: Vec1<Source>,
    /// As configured; never changed after the phase is added.
    plugins: PhasePlugins,
    seed: Seed,
    started: bool,
    /// Empty until the phase is started.
    divisions: Vec<Division>,
}

impl IdAble for Phase {
    type Id = PhaseId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl Phase {
    /// A phase that has not started, `seed` derived from the competition seed and `id`.
    pub(super) fn new(
        id: PhaseId,
        name: Option<String>,
        sources: Vec1<Source>,
        plugins: PhasePlugins,
        seed: Seed,
    ) -> Self {
        Self {
            id,
            name,
            sources,
            plugins,
            seed,
            started: false,
            divisions: Vec::new(),
        }
    }

    pub fn id(&self) -> PhaseId {
        self.id
    }

    /// The name the phase was given, if any.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Where the entrants of this phase come from, in the order its field takes them.
    pub fn sources(&self) -> &Vec1<Source> {
        &self.sources
    }

    /// Whether every source of this phase is the registration, so that the phase starts with the
    /// competition.
    pub fn is_fed_by_registration_alone(&self) -> bool {
        self.sources
            .iter()
            .all(|source| source.of() == crate::SourceOf::Registration)
    }

    /// The plugins this phase is configured with, without the state its divisions play them
    /// with, see [`Division::plugins`].
    pub fn plugins(&self) -> &PhasePlugins {
        &self.plugins
    }

    /// The answers to a [`Competition::phase_draft`] the plugins of this phase are built from,
    /// without their runtime state, see [`PhasePlugins::config`]. Put back into a draft with
    /// [`Draft::with_answers`], they are changed and built into the plugins the phase is
    /// reconfigured with, see
    /// [`reconfigure_phase`](crate::CompetitionStateMachineExt::reconfigure_phase).
    pub fn config(&self) -> Config {
        self.plugins.config()
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    /// The divisions of this phase, none before it is started.
    pub fn divisions(&self) -> &[Division] {
        &self.divisions
    }

    /// The division holding the match `match_id`, with its index.
    pub fn division_of(&self, match_id: &MatchId) -> Option<(usize, &Division)> {
        self.divisions
            .iter()
            .enumerate()
            .find(|(_, division)| division.data().matches().contains_key(match_id))
    }

    /// The overall standings of this phase: those of its only division, empty before it is
    /// started. Sorted on every call: a caller reading them more than once keeps them.
    pub fn standings(&self) -> Standings<'_> {
        self.divisions
            .first()
            .map_or_else(|| Standings::new(Vec::new()), Division::standings)
    }

    /// Whether this phase is started and none of its divisions has a match left to play.
    pub fn is_finished(&self) -> bool {
        self.started
            && self.divisions.iter().all(|division| {
                division
                    .data()
                    .matches()
                    .values()
                    .all(|r#match| r#match.is_finished())
            })
    }

    /// Starts this phase with one division of a ranking of each of `field`, its entrants and
    /// their best lot, ranked initially in this order; its matches are numbered after `highest`,
    /// see [`Division::start`].
    pub(crate) fn start(&mut self, field: Vec<(Vec1<EntrantId>, NonZeroU32)>, highest: &mut u32) {
        let mut division = Division::new(self.plugins.clone(), self.seed);
        division.start(field, highest);
        self.divisions = vec![division];
        self.started = true;
    }

    /// Finishes the match `match_id` with what was `entered` for it, see [`Division::report`].
    ///
    /// # Panics
    /// Panics if no division of this phase holds the match, or as `Division::report` does.
    pub(crate) fn report_match(&mut self, match_id: MatchId, entered: Part, highest: &mut u32) {
        let (index, _) = self
            .division_of(&match_id)
            .expect("a division holds the match");
        self.divisions[index].report(match_id, entered, highest);
    }
}

impl<'de> Deserialize<'de> for Phase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "Phase")]
        struct Saved {
            id: PhaseId,
            name: Option<String>,
            sources: Vec1<Source>,
            plugins: PhasePlugins,
            seed: Seed,
            started: bool,
            divisions: Vec<Division>,
        }

        let saved = Saved::deserialize(deserializer)?;
        // A phase of a competition saved before it had divisions held the plugins of its only one,
        // with their runtime state, which the converter, knowing no plugin, leaves in place.
        Ok(Self {
            id: saved.id,
            name: saved.name,
            sources: saved.sources,
            plugins: saved.plugins.without_state(),
            seed: saved.seed,
            started: saved.started,
            divisions: saved.divisions,
        })
    }
}
