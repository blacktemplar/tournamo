use std::{any::Any, num::NonZeroU32};

use itertools::Itertools;
use plugin_factory::config::Config;
#[cfg(doc)]
use plugin_factory::draft::Draft;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use vec1::Vec1;

#[cfg(feature = "test-util")]
pub(super) mod builder;

use super::seed::Entropy;
#[cfg(doc)]
use crate::Competition;
#[cfg(test)]
use crate::Decision;
use crate::{
    Bye, ByeId, EntrantId, Match, MatchId, NewMatch, Part, PhaseId, Ranking, RankingId, Seed,
    Standing, Standings,
    map::{IdAble, IdMap, Map},
    plugins::{
        PhasePlugins,
        match_format::MatchFormat,
        metrics_calculator::{Metric, MetricsCalculator, OldMetrics},
        phase_format::{PhaseFormat, Schedule},
        ranker::{HierarchicalRanker, SliceRanker},
    },
    type_map::TypeMap,
};

#[derive(Debug)]
pub struct Phase {
    id: PhaseId,
    phase_format: Box<dyn PhaseFormat>,
    ranker: HierarchicalRanker,
    metrics_calculators: Vec<Box<dyn MetricsCalculator>>,
    match_format: Box<dyn MatchFormat>,
    data: PhaseData,
}

/// Everything a [`Phase`] holds but its plugins: its rankings, matches, byes and seed. Where the
/// rankings stand depends on how the phase ranks them, see [`Phase::standings`].
#[derive(Debug)]
pub struct PhaseData {
    rankings: IdMap<Ranking>,
    matches: IdMap<Match>,
    byes: IdMap<Bye>,
    seed: Seed,
}

impl IdAble for Phase {
    type Id = PhaseId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

impl PhaseData {
    /// The rankings of this phase, ordered by their [`Ranking::initial_unique_rank`].
    pub fn rankings(&self) -> &IdMap<Ranking> {
        &self.rankings
    }

    pub fn matches(&self) -> &IdMap<Match> {
        &self.matches
    }

    /// The byes of this phase, in the order they were scheduled in.
    pub fn byes(&self) -> &IdMap<Bye> {
        &self.byes
    }

    /// The seed of this phase, derived from the competition seed and the phase id.
    pub fn seed(&self) -> Seed {
        self.seed
    }

    /// The random stream of the [`PhaseFormat`] of this phase. Every call starts the same stream
    /// again, so a format draws everything it needs from a single one.
    pub fn rng(&self) -> StdRng {
        self.seed.rng(Entropy::PhaseFormat)
    }

    /// Whether any match or bye has been scheduled in this phase.
    pub fn has_schedule(&self) -> bool {
        !self.matches.is_empty() || !self.byes.is_empty()
    }
}

impl PhaseData {
    /// `matches` numbered as they ask, or else each with the number after the highest one so far.
    fn number(&self, matches: Vec<NewMatch>) -> Vec<Match> {
        // looked for only if a match asks for no number, since a bracket schedules after every
        // result, each match with a number
        let mut highest = matches
            .iter()
            .any(|r#match| r#match.number().is_none())
            .then(|| {
                self.matches
                    .values()
                    .map(Match::number)
                    .chain(matches.iter().filter_map(NewMatch::number))
                    .max()
            })
            .flatten();
        matches
            .into_iter()
            .map(|r#match| {
                let number = r#match.number().unwrap_or_else(|| {
                    let next = highest.map_or(NonZeroU32::MIN, |highest| {
                        highest.checked_add(1).expect("fewer matches than u32::MAX")
                    });
                    highest = Some(next);
                    next
                });
                r#match.numbered(number)
            })
            .collect()
    }

    /// Checks, with debug assertions, that the `matches` and `byes` a format scheduled can be
    /// added: they are of rankings of this phase, their ids and the numbers of the matches are new
    /// and unique, and no ranking has two of them, or one and one already scheduled, in a round.
    fn debug_check(&self, matches: &[Match], byes: &[Bye]) {
        debug_assert!(
            matches
                .iter()
                .flat_map(Match::rankings)
                .copied()
                .chain(byes.iter().map(Bye::ranking))
                .all(|r| self.rankings.contains_key(&r))
        );
        debug_assert!(
            self.matches
                .keys()
                .copied()
                .chain(matches.iter().map(Match::id))
                .all_unique()
        );
        debug_assert!(
            self.byes
                .keys()
                .copied()
                .chain(byes.iter().map(Bye::id))
                .all_unique()
        );
        debug_assert!(
            self.matches
                .values()
                .chain(matches)
                .map(Match::number)
                .all_unique()
        );
        debug_assert!(
            self.matches
                .values()
                .chain(matches)
                .flat_map(|m| m.rankings().map(|r| (m.round(), *r)))
                .chain(
                    self.byes
                        .values()
                        .chain(byes)
                        .map(|b| (b.round(), b.ranking()))
                )
                .all_unique()
        );
    }
}

/// A phase as its format sees it when scheduling: its data, and its standings when asked for.
#[derive(Debug, Clone, Copy)]
pub struct PhaseView<'a> {
    data: &'a PhaseData,
    ranker: &'a HierarchicalRanker,
}

impl<'a> PhaseView<'a> {
    pub fn data(&self) -> &'a PhaseData {
        self.data
    }

    /// The standings, sorted on every call: ask once and keep them.
    pub fn standings(&self) -> Standings<'a> {
        Standings::new(self.ranker.rank(self.data))
    }
}

impl Phase {
    /// A phase without rankings, matches and byes yet.
    pub(super) fn new(id: PhaseId, plugins: PhasePlugins, seed: Seed) -> Self {
        let (phase_format, match_format, ranker, metrics_calculators) = plugins.into_parts();
        Self {
            id,
            data: PhaseData {
                rankings: IdMap::new(),
                matches: IdMap::new(),
                byes: IdMap::new(),
                seed,
            },
            phase_format,
            ranker,
            metrics_calculators,
            match_format,
        }
    }

    pub fn id(&self) -> PhaseId {
        self.id
    }

    /// The rankings, matches and seed of this phase, i.e. everything but its plugins.
    pub fn data(&self) -> &PhaseData {
        &self.data
    }

    /// The standings of this phase, sorted on every call: a caller reading them more than once
    /// keeps them.
    pub fn standings(&self) -> Standings<'_> {
        PhaseView {
            data: &self.data,
            ranker: &self.ranker,
        }
        .standings()
    }

    /// The calculators of this phase's metrics, in the order they run in.
    pub fn metrics_calculators(&self) -> &[Box<dyn MetricsCalculator>] {
        &self.metrics_calculators
    }

    /// The criteria this phase is ranked by, each breaking the ties of those before it.
    pub fn slice_rankers(&self) -> &[Box<dyn SliceRanker>] {
        self.ranker.slice_rankers()
    }

    /// The format the matches of this phase are scheduled by.
    pub fn format(&self) -> &dyn PhaseFormat {
        &*self.phase_format
    }

    /// The match format of this phase: what is entered for its matches.
    pub fn match_format(&self) -> &dyn MatchFormat {
        &*self.match_format
    }

    /// The answers to a [`Competition::phase_draft`] the plugins of this phase are built from,
    /// without their runtime state, see [`PhasePlugins::config`]. Put back into a draft with
    /// [`Draft::with_answers`], they are changed and built into the plugins the phase is
    /// reconfigured with, see
    /// [`reconfigure_phase`](crate::CompetitionStateMachineExt::reconfigure_phase).
    pub fn config(&self) -> Config {
        PhasePlugins::from_parts(
            self.phase_format.clone(),
            self.match_format.clone(),
            self.ranker.slice_rankers().to_vec(),
            self.metrics_calculators.clone(),
        )
        .expect("the calculators of a phase are in running order")
        .config()
    }

    pub(crate) fn start(&mut self, ranking_entrants: Vec<Vec1<EntrantId>>) {
        self.data.rankings = ranking_entrants
            .into_iter()
            .enumerate()
            .map(|(index, entrants)| {
                Ranking::new(entrants, NonZeroU32::new(index as u32 + 1).unwrap())
            })
            .collect();
        self.initialize_metrics();
        self.create_new_matches();
    }

    /// Gives every ranking of this phase the default value of every metric calculated for this
    /// phase.
    fn initialize_metrics(&mut self) {
        let ranking_ids: Vec<_> = self.data.rankings.keys().copied().collect();
        let metrics: Vec<_> = self
            .metrics_calculators
            .iter()
            .flat_map(|metrics_calculator| {
                ranking_ids
                    .iter()
                    .map(move |ranking_id| (*ranking_id, metrics_calculator.default_metric_value()))
            })
            .collect();
        for (ranking_id, metric) in metrics {
            self.data
                .rankings
                .get_mut(&ranking_id)
                .expect("Ranking must exist")
                .metrics
                .insert_boxed(metric);
        }
    }

    /// Adds the matches and byes the phase format schedules next, and updates the metrics for every
    /// new bye its ranking reaches.
    pub(crate) fn create_new_matches(&mut self) {
        let view = PhaseView {
            data: &self.data,
            ranker: &self.ranker,
        };
        let Schedule { matches, byes } = self.phase_format.create_new_matches(&view);
        let matches = self.data.number(matches);
        self.data.debug_check(&matches, &byes);
        let new_bye_ids: Vec<_> = byes.iter().map(Bye::id).collect();
        self.data.matches.extend(matches);
        self.data.byes.extend(byes);
        // The format is not asked again afterwards, even though a bye reached may have changed the
        // standings: only a reported result makes it schedule.
        self.reach_byes(new_bye_ids);
    }

    /// Marks those of the byes `candidates` (in this order) whose ranking has no open match in an
    /// earlier round as reached, and updates the metrics for each, as if they were matches reported
    /// as soon as they can be played.
    fn reach_byes(&mut self, candidates: impl IntoIterator<Item = ByeId>) {
        for bye_id in candidates {
            let bye = &self.data.byes[&bye_id];
            if bye.reached {
                continue;
            }
            let (ranking, round) = (bye.ranking(), bye.round());
            let blocked = self.data.matches.values().any(|r#match| {
                r#match.round() < round
                    && !r#match.is_finished()
                    && r#match.rankings().any(|r| *r == ranking)
            });
            if blocked {
                continue;
            }
            self.data.byes.get_mut(&bye_id).unwrap().reached = true;
            self.update_metrics(|metrics_calculator, data, old_metrics| {
                metrics_calculator.update_after_bye(data, &data.byes[&bye_id], old_metrics)
            });
        }
    }

    /// Reports `decision` alone for the match `match_id`, see [`Phase::report_match`].
    #[cfg(test)]
    pub(crate) fn report_match_result(&mut self, match_id: MatchId, decision: impl Into<Decision>) {
        self.report_match(match_id, Part::new().with_decision(decision));
    }

    /// Finishes the match `match_id` with what was `entered` for it: its decision, values and
    /// parts, as its [contest](Match::contest).
    ///
    /// # Panics
    /// Panics if the match is finished, or `entered` has no decision.
    pub(crate) fn report_match(&mut self, match_id: MatchId, entered: Part) {
        assert!(entered.decision().is_some(), "a report decides its match");
        let r#match = self.data.matches.get_mut(&match_id).unwrap();
        assert!(!r#match.is_finished());
        r#match.contest.enter(entered);
        let tallies = self
            .match_format
            .tally(&self.data, &self.data.matches[&match_id]);
        self.data.matches.get_mut(&match_id).unwrap().tallies = tallies;
        for ranking_id in self.data.matches[&match_id].rankings() {
            let ranking = self.data.rankings.get_mut(ranking_id).unwrap();
            ranking.played_matches.push(match_id);
        }
        self.update_metrics(|metrics_calculator, data, old_metrics| {
            metrics_calculator.update_after_match_result(
                data,
                &data.matches[&match_id],
                old_metrics,
            )
        });
        // only the byes of the match's rankings can have been reached by its result
        let rankings: Vec<_> = self.data.matches[&match_id].rankings().copied().collect();
        let candidates: Vec<_> = self
            .data
            .byes
            .values()
            .filter(|bye| !bye.reached && rankings.contains(&bye.ranking()))
            .map(Bye::id)
            .collect();
        self.reach_byes(candidates);
        self.create_new_matches();
    }

    /// Runs every metrics calculator, in order, through `updates_of` and stores the metrics it
    /// returns, handing each calculator the metrics replaced by the ones before it.
    fn update_metrics(
        &mut self,
        mut updates_of: impl FnMut(
            &mut dyn MetricsCalculator,
            &PhaseData,
            &OldMetrics,
        ) -> Vec<(RankingId, Metric)>,
    ) {
        let mut old_metrics = OldMetrics::default();
        for metrics_calculator in &mut self.metrics_calculators {
            let updates = updates_of(&mut **metrics_calculator, &self.data, &old_metrics);
            // check requirements of the update methods of MetricsCalculator
            debug_assert!(updates.iter().all(|(ranking_id, metric)| {
                let metric: &dyn Any = &**metric;
                self.data.rankings.contains_key(ranking_id)
                    && (&*metrics_calculator.default_metric_value() as &dyn Any).type_id()
                        == metric.type_id()
            }));

            for (ranking_id, metric) in updates {
                let old_metric = self
                    .data
                    .rankings
                    .get_mut(&ranking_id)
                    .expect("Ranking must exist")
                    .metrics
                    .insert_boxed(metric)
                    .unwrap();
                old_metrics.push(ranking_id, old_metric);
            }
        }
    }
}

// A phase is saved as it was when its rankings stored their ranks and its data the order of its
// standings: they are computed when it is written, and read but dropped when it is read, since a
// format that is not self-describing cannot skip them.
impl Serialize for Phase {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "Phase")]
        struct Saved<'a> {
            id: PhaseId,
            phase_format: &'a dyn PhaseFormat,
            ranker: &'a HierarchicalRanker,
            metrics_calculators: &'a [Box<dyn MetricsCalculator>],
            match_format: &'a dyn MatchFormat,
            data: SavedData<'a>,
        }
        #[derive(Serialize)]
        #[serde(rename = "PhaseData")]
        struct SavedData<'a> {
            rankings: Vec<SavedRanking<'a>>,
            ranking_ids_ordered_by_unique_rank: Vec<RankingId>,
            matches: &'a IdMap<Match>,
            byes: &'a IdMap<Bye>,
            seed: Seed,
        }
        #[derive(Serialize)]
        #[serde(rename = "Ranking")]
        struct SavedRanking<'a> {
            id: RankingId,
            entrants: &'a Vec1<EntrantId>,
            initial_unique_rank: NonZeroU32,
            unique_rank: NonZeroU32,
            rank: NonZeroU32,
            played_matches: &'a [MatchId],
            metrics: &'a TypeMap,
        }

        let standings = self.standings();
        let standing_of: Map<RankingId, &Standing> = standings
            .iter()
            .map(|standing| (standing.ranking().id(), standing))
            .collect();
        let rankings = self
            .data
            .rankings
            .values()
            .map(|ranking| {
                let standing = standing_of[&ranking.id];
                SavedRanking {
                    id: ranking.id,
                    entrants: &ranking.entrants,
                    initial_unique_rank: ranking.initial_unique_rank,
                    unique_rank: standing.unique_rank(),
                    rank: standing.rank(),
                    played_matches: &ranking.played_matches,
                    metrics: &ranking.metrics,
                }
            })
            .collect();
        Saved {
            id: self.id,
            phase_format: &*self.phase_format,
            ranker: &self.ranker,
            metrics_calculators: &self.metrics_calculators,
            match_format: &*self.match_format,
            data: SavedData {
                rankings,
                ranking_ids_ordered_by_unique_rank: standings
                    .iter()
                    .map(|standing| standing.ranking().id())
                    .collect(),
                matches: &self.data.matches,
                byes: &self.data.byes,
                seed: self.data.seed,
            },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Phase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "Phase")]
        struct Saved {
            id: PhaseId,
            phase_format: Box<dyn PhaseFormat>,
            ranker: HierarchicalRanker,
            metrics_calculators: Vec<Box<dyn MetricsCalculator>>,
            match_format: Box<dyn MatchFormat>,
            data: SavedData,
        }
        #[derive(Deserialize)]
        #[serde(rename = "PhaseData")]
        struct SavedData {
            rankings: Vec<SavedRanking>,
            #[serde(rename = "ranking_ids_ordered_by_unique_rank")]
            _ranking_ids_ordered_by_unique_rank: Vec<RankingId>,
            matches: IdMap<Match>,
            byes: IdMap<Bye>,
            seed: Seed,
        }
        #[derive(Deserialize)]
        #[serde(rename = "Ranking")]
        struct SavedRanking {
            id: RankingId,
            entrants: Vec1<EntrantId>,
            initial_unique_rank: NonZeroU32,
            #[serde(rename = "unique_rank")]
            _unique_rank: NonZeroU32,
            #[serde(rename = "rank")]
            _rank: NonZeroU32,
            played_matches: Vec<MatchId>,
            metrics: TypeMap,
        }

        let saved = Saved::deserialize(deserializer)?;
        let rankings = saved
            .data
            .rankings
            .into_iter()
            .map(|ranking| Ranking {
                id: ranking.id,
                entrants: ranking.entrants,
                initial_unique_rank: ranking.initial_unique_rank,
                played_matches: ranking.played_matches,
                metrics: ranking.metrics,
            })
            .collect();
        Ok(Phase {
            id: saved.id,
            phase_format: saved.phase_format,
            ranker: saved.ranker,
            metrics_calculators: saved.metrics_calculators,
            match_format: saved.match_format,
            data: PhaseData {
                rankings,
                matches: saved.data.matches,
                byes: saved.data.byes,
                seed: saved.data.seed,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        MatchResult, PhaseBuilder,
        plugins::metrics_calculator::test_utils::{self, match_id},
    };

    /// The played matches of the ranking with the given index in the ordering by initial unique
    /// rank.
    fn played(phase: &Phase, ranking_index: usize) -> &[MatchId] {
        phase
            .data()
            .rankings()
            .values()
            .nth(ranking_index)
            .unwrap()
            .played_matches()
    }

    #[test]
    fn every_ranking_of_both_teams_plays_a_reported_match() {
        let matches = vec![(1, vec![0, 1], vec![2, 3]), (1, vec![4], vec![5])];
        let mut phase = test_utils::phase_with(PhaseBuilder::new().add_players(6), matches);
        let match_id = match_id(&phase, 1, &[0, 1], &[2, 3]);

        phase.report_match_result(match_id, MatchResult::BWins);

        for ranking_index in 0..4 {
            assert_eq!(played(&phase, ranking_index), [match_id]);
        }
        // the rankings of the other, still unreported match have played nothing
        assert!(played(&phase, 4).is_empty());
        assert!(played(&phase, 5).is_empty());
    }

    #[test]
    fn played_matches_are_in_the_order_they_were_reported_in() {
        let matches = vec![
            (1, vec![0], vec![1]),
            (1, vec![2], vec![3]),
            (2, vec![0], vec![2]),
            (2, vec![1], vec![3]),
            (3, vec![1], vec![2]),
        ];
        let mut phase = test_utils::phase_with(PhaseBuilder::new().add_players(4), matches.clone());
        let match_ids: Vec<_> = matches
            .iter()
            .map(|(round, team_a, team_b)| match_id(&phase, *round, team_a, team_b))
            .collect();
        let [m_0_1, m_2_3, m_0_2, m_1_3, m_1_2] = match_ids[..] else {
            unreachable!("five matches are scheduled")
        };

        // reported in the reverse order of the schedule
        for match_id in match_ids.iter().rev() {
            phase.report_match_result(*match_id, MatchResult::AWins);
        }

        assert_eq!(played(&phase, 0), [m_0_2, m_0_1]);
        assert_eq!(played(&phase, 1), [m_1_2, m_1_3, m_0_1]);
        assert_eq!(played(&phase, 2), [m_1_2, m_0_2, m_2_3]);
        assert_eq!(played(&phase, 3), [m_1_3, m_2_3]);
    }
}
