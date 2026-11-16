use std::num::NonZeroU32;

use itertools::Itertools;
use plugin_factory::config::Config;
#[cfg(doc)]
use plugin_factory::draft::Draft;
use rand::RngExt;
use serde::{Deserialize, Deserializer, Serialize};
use vec1::Vec1;

use crate::{
    Columns, Division, Entrant, MatchId, Part, PhaseId, PhaseStandings, Qualifier, RankingId, Seed,
    Source, Standing, Standings,
    entities::{
        Entropy,
        columns::{self, Rule, Table},
    },
    map::{IdAble, IdMap, Map},
    plugins::{
        PhasePlugins,
        draw::{DivisionOrder, DrawView},
        metrics_calculator::MetricsCalculator,
        ranker::RankingScope,
    },
};
#[cfg(doc)]
use crate::{
    Competition,
    plugins::{draw::Draw, phase_format::PhaseFormat},
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
    columns: Columns,
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
        columns: Columns,
        seed: Seed,
    ) -> Self {
        Self {
            id,
            name,
            sources,
            plugins,
            columns,
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

    /// What the operator said about the columns of its tables of standings, see [`Self::shown`].
    pub fn columns(&self) -> &Columns {
        &self.columns
    }

    /// The calculators of this phase that `table` shows by rule, by id, with why, see
    /// [`columns::by_rule`].
    pub fn rule(&self, table: Table) -> Vec<(&'static str, Rule)> {
        let criteria = match table {
            Table::Division => self.plugins.ranking(),
            Table::Across => self.plugins.across(),
        };
        let criteria: Vec<&str> = criteria
            .iter()
            .map(|criterion| plugin_factory::id_of(&**criterion))
            .collect();
        let metrics: Vec<&str> = self
            .plugins
            .metrics()
            .iter()
            .map(|calculator| plugin_factory::id_of(&**calculator))
            .collect();
        columns::by_rule(&criteria, &metrics)
    }

    /// The calculators whose column `table` shows, in the order of the columns: those the
    /// [rule](Self::rule) shows and the operator did not hide, and those the operator showed
    /// ([`Self::columns`]), if they offer a column ([`MetricsCalculator::column`]). Those a
    /// criterion of the table ranks by come first, in the order of the criteria, then the others
    /// in running order.
    pub fn shown(&self, table: Table) -> Vec<&dyn MetricsCalculator> {
        let rule = self.rule(table);
        let mut shown: Vec<&dyn MetricsCalculator> = self
            .plugins
            .metrics()
            .iter()
            .map(|calculator| &**calculator)
            .filter(|calculator| {
                let id = plugin_factory::id_of(*calculator);
                self.columns
                    .get(id)
                    .of(table)
                    .unwrap_or_else(|| rule.iter().any(|&(ruled, _)| ruled == id))
                    && calculator.column().is_some()
            })
            .collect();
        // stable, so that the others stay in running order; the rule lists those ranked by
        // first, in the order of the criteria
        shown.sort_by_key(|calculator| {
            let id = plugin_factory::id_of(*calculator);
            rule.iter()
                .position(|&(ruled, why)| ruled == id && matches!(why, Rule::RankedBy(_)))
                .unwrap_or(usize::MAX)
        });
        shown
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

    /// The overall standings of this phase, empty before it is started. Sorted on every call: a
    /// caller reading them more than once keeps them.
    ///
    /// With one division they are its standings. Otherwise they depend on how its draw sets the
    /// divisions against each other ([`Draw::order`]), but never against a division: a ranking
    /// with a better rank than another in their division has a better overall rank and unique
    /// rank too.
    ///
    /// - [`DivisionOrder::Levels`]: the standings of the divisions one after the other.
    /// - [`DivisionOrder::Parallel`]: by unique rank in the division, so that each place of a
    ///   division is one place of the overall standings, ties in a division broken by its lots;
    ///   the rankings of one unique rank in their divisions are ranked against each other by the
    ///   slice rankers of [`PhasePlugins::across`], with [`RankingScope::AcrossDivisions`], and
    ///   what stays tied shares an overall rank.
    ///
    /// Ties of the overall rank are broken by [lot](crate::Ranking::lot).
    pub fn standings(&self) -> PhaseStandings<'_> {
        let mut divisions = self.divisions.iter().map(Division::standings);
        let rows = match (self.divisions.len(), self.plugins.draw().order()) {
            (0 | 1, _) => divisions
                .next()
                .map(|standings| {
                    standings
                        .iter()
                        .map(|&standing| (standing, 0, standing.rank()))
                        .collect()
                })
                .unwrap_or_default(),
            (_, DivisionOrder::Levels) => {
                let mut rows = Vec::new();
                for (division, standings) in divisions.enumerate() {
                    let before = rows.len() as u32;
                    rows.extend(standings.iter().map(|&standing| {
                        let rank = standing
                            .rank()
                            .checked_add(before)
                            .expect("fewer rankings than u32::MAX");
                        (standing, division, rank)
                    }));
                }
                rows
            }
            (_, DivisionOrder::Parallel) => self.parallel_rows(divisions.collect()),
        };
        PhaseStandings::new(rows)
    }

    /// The rows of the overall standings of parallel divisions with the `standings` of each: by
    /// unique rank in the division, then by the ranking across divisions.
    fn parallel_rows<'a>(
        &'a self,
        standings: Vec<Standings<'a>>,
    ) -> Vec<(Standing<'a>, usize, NonZeroU32)> {
        let mut rows: Vec<(Standing, usize)> = standings
            .iter()
            .enumerate()
            .flat_map(|(division, standings)| {
                standings.iter().map(move |&standing| (standing, division))
            })
            .collect();
        rows.sort_by_key(|&(standing, _)| standing.unique_rank());
        let row_of: Map<RankingId, (Standing, usize)> = rows
            .iter()
            .map(|&(standing, division)| (standing.ranking().id(), (standing, division)))
            .collect();
        let across = self.plugins.across_ranker();
        let mut ranked = Vec::with_capacity(rows.len());
        for slice in rows.chunk_by(|(s1, _), (s2, _)| s1.unique_rank() == s2.unique_rank()) {
            let start = NonZeroU32::new(ranked.len() as u32 + 1).expect("one more than a count");
            let mut slice: Vec<_> = slice
                .iter()
                .map(|(standing, _)| (standing.ranking(), start))
                .collect();
            across.rank_tied(RankingScope::AcrossDivisions, &mut slice);
            ranked.extend(slice.into_iter().map(|(ranking, rank)| {
                let (standing, division) = row_of[&ranking.id()];
                (standing, division, rank)
            }));
        }
        ranked
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

    /// Starts this phase with `field`, each qualifier with the best lot of its entrants: its draw
    /// deals the field to its divisions, each starting with a ranking of each of its qualifiers in
    /// the order dealt. It deals to as many divisions as the phase is configured with, but no more
    /// than leave each the entrants its format needs (see [`PhaseFormat::min_entrants`]): a
    /// consumer refuses to start a phase with too few for its divisions, this only keeps a start
    /// it does not refuse, e.g. of a log written elsewhere, from leaving a division too small.
    /// The first division has the seed of the phase, every other one a seed derived from it.
    /// Their matches are numbered after `highest`, see [`Division::start`].
    pub(crate) fn start(
        &mut self,
        field: Vec<(Qualifier, NonZeroU32)>,
        entrants: &IdMap<Entrant>,
        highest: &mut u32,
    ) {
        let needed = self.plugins.format().min_entrants().max(1);
        let count = (self.plugins.divisions().get() as usize).min((field.len() / needed).max(1));
        let (qualifiers, lots): (Vec<_>, Vec<_>) = field.into_iter().unzip();
        let dealt =
            self.plugins
                .draw()
                .draw(&DrawView::new(&qualifiers, count, entrants, self.seed));
        debug_assert!(
            dealt.len() == count
                && dealt
                    .iter()
                    .flatten()
                    .copied()
                    .sorted()
                    .eq(0..qualifiers.len())
                && (qualifiers.is_empty() || dealt.iter().all(|positions| !positions.is_empty())),
            "a draw deals every qualifier to one of the divisions, none empty"
        );
        self.divisions = dealt
            .into_iter()
            .enumerate()
            .map(|(index, positions)| {
                let seed = if index == 0 {
                    self.seed
                } else {
                    Seed::new(self.seed.rng(Entropy::Division(index)).random())
                };
                let mut division = Division::new(self.plugins.clone(), seed);
                let field = positions
                    .into_iter()
                    .map(|position| (qualifiers[position].entrants.clone(), lots[position]))
                    .collect();
                division.start(field, highest);
                division
            })
            .collect();
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
            columns: Columns,
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
            columns: saved.columns,
            seed: saved.seed,
            started: saved.started,
            divisions: saved.divisions,
        })
    }
}
