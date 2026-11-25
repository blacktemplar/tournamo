use std::{any::Any, num::NonZeroU32};

use itertools::Itertools;
use plugin_factory::schema::DependencyOrderedSet;
use rand::rngs::StdRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use vec1::Vec1;

#[cfg(feature = "test-util")]
pub(super) mod builder;

use super::seed::Entropy;
#[cfg(test)]
use crate::Decision;
use crate::{
    Bye, ByeId, EntrantId, Match, MatchId, NewMatch, Part, Ranking, RankingId, Seed, Standing,
    Standings,
    map::{IdMap, Map},
    plugins::{
        PhasePlugins,
        match_format::MatchFormat,
        metrics_calculator::{BoxedMetric, MetricsCalculator, OldMetrics},
        phase_format::{PhaseFormat, Schedule},
        ranker::HierarchicalRanker,
    },
    type_map::TypeMap,
};

/// A part of a [`Phase`](crate::Phase) played on its own: its rankings, matches and byes, and its
/// own copy of the phase's plugins, which holds the state of its format and calculators.
#[derive(Debug)]
pub struct Division {
    plugins: PhasePlugins,
    data: DivisionData,
    number_base: u32,
}

/// Everything a [`Division`] holds but its plugins: its rankings, matches, byes and seed. Where
/// the rankings stand depends on how the division ranks them, see [`Division::standings`].
#[derive(Debug)]
pub struct DivisionData {
    rankings: IdMap<Ranking>,
    matches: IdMap<Match>,
    byes: IdMap<Bye>,
    seed: Seed,
}

impl DivisionData {
    /// The rankings of this division, ordered by their [`Ranking::initial_unique_rank`].
    pub fn rankings(&self) -> &IdMap<Ranking> {
        &self.rankings
    }

    pub fn matches(&self) -> &IdMap<Match> {
        &self.matches
    }

    /// The byes of this division, in the order they were scheduled in.
    pub fn byes(&self) -> &IdMap<Bye> {
        &self.byes
    }

    /// The seed of this division: that of its phase.
    pub fn seed(&self) -> Seed {
        self.seed
    }

    /// The random stream of the [`PhaseFormat`] of this division. Every call starts the same stream
    /// again, so a format draws everything it needs from a single one.
    pub fn rng(&self) -> StdRng {
        self.seed.rng(Entropy::PhaseFormat)
    }

    /// Whether any match or bye has been scheduled in this division.
    pub fn has_schedule(&self) -> bool {
        !self.matches.is_empty() || !self.byes.is_empty()
    }
}

impl DivisionData {
    /// `matches` numbered as they ask, relative to `base`, or else each with the number after
    /// `highest`, the highest number given in the competition so far, which is raised to the
    /// highest number given or `reserved`.
    fn number(matches: Vec<NewMatch>, base: u32, reserved: u32, highest: &mut u32) -> Vec<Match> {
        let relative = |number: NonZeroU32| {
            number
                .checked_add(base)
                .expect("fewer matches than u32::MAX")
        };
        *highest = matches
            .iter()
            .filter_map(NewMatch::number)
            .map(|number| relative(number).get())
            .chain([*highest, reserved])
            .max()
            .unwrap_or(*highest);
        matches
            .into_iter()
            .map(|r#match| {
                let number = r#match.number().map_or_else(
                    || {
                        *highest = highest.checked_add(1).expect("fewer matches than u32::MAX");
                        NonZeroU32::new(*highest).expect("one more than a number")
                    },
                    relative,
                );
                r#match.numbered(number)
            })
            .collect()
    }

    /// Checks, with debug assertions, that the `matches` and `byes` a format scheduled can be
    /// added: they are of rankings of this division, their ids and the numbers of the matches are
    /// new and unique, and no ranking has two of them, or one and one already scheduled, in a
    /// round.
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

/// A division as its format sees it when scheduling: its data, and its standings when asked for.
#[derive(Debug, Clone, Copy)]
pub struct DivisionView<'a> {
    data: &'a DivisionData,
    ranker: HierarchicalRanker<'a>,
}

impl<'a> DivisionView<'a> {
    pub(crate) fn new(data: &'a DivisionData, ranker: HierarchicalRanker<'a>) -> Self {
        Self { data, ranker }
    }

    pub fn data(&self) -> &'a DivisionData {
        self.data
    }

    /// The standings, sorted on every call: ask once and keep them.
    pub fn standings(&self) -> Standings<'a> {
        Standings::new(self.ranker.rank(self.data))
    }
}

/// A finished match or a reached bye, counted again by [`Division::recount`].
#[derive(Debug, Clone, Copy)]
enum Counted {
    Match(MatchId),
    Bye(ByeId),
}

impl Division {
    /// A division without rankings, matches and byes yet.
    pub(super) fn new(plugins: PhasePlugins, seed: Seed) -> Self {
        Self {
            data: DivisionData {
                rankings: IdMap::new(),
                matches: IdMap::new(),
                byes: IdMap::new(),
                seed,
            },
            plugins,
            number_base: 0,
        }
    }

    /// The rankings, matches and seed of this division, i.e. everything but its plugins.
    pub fn data(&self) -> &DivisionData {
        &self.data
    }

    /// The standings of this division, sorted on every call: a caller reading them more than once
    /// keeps them.
    pub fn standings(&self) -> Standings<'_> {
        DivisionView::new(&self.data, self.plugins.ranker()).standings()
    }

    /// The plugins this division is played with: its format, match format, ranking and metrics,
    /// holding the state of its format and calculators.
    pub fn plugins(&self) -> &PhasePlugins {
        &self.plugins
    }

    /// What the numbers its format asks for are relative to: the highest number of a match in the
    /// competition when this division started. A match asking for number `n` has number
    /// `number_base + n`, see [`NewMatch::with_number`].
    pub fn number_base(&self) -> u32 {
        self.number_base
    }

    /// The format the matches of this division are scheduled by, with its state.
    pub fn format(&self) -> &dyn PhaseFormat {
        self.plugins.format()
    }

    /// The match format of this division: what is entered for its matches.
    pub fn match_format(&self) -> &dyn MatchFormat {
        self.plugins.match_format()
    }

    /// Starts this division with a ranking of each of `field`, its entrants and their best lot, in
    /// seeding order, and schedules its first matches, numbered after `highest`, the highest
    /// number of a match in the competition, which is raised to the highest number given.
    pub(crate) fn start(&mut self, field: Vec<(Vec1<EntrantId>, NonZeroU32)>, highest: &mut u32) {
        self.number_base = *highest;
        self.set_rankings(field);
        self.schedule(highest);
    }

    /// Gives this division a ranking of each of `field`, its entrants and lot, ranked initially in
    /// this order, with the default value of every metric calculated for it.
    pub(super) fn set_rankings(&mut self, field: Vec<(Vec1<EntrantId>, NonZeroU32)>) {
        self.data.rankings = field
            .into_iter()
            .zip(NonZeroU32::MIN..)
            .map(|((entrants, lot), initial_unique_rank)| {
                Ranking::new(entrants, initial_unique_rank, lot)
            })
            .collect();
        self.initialize_metrics();
    }

    /// Gives every ranking of this division the default value of every metric calculated for it.
    fn initialize_metrics(&mut self) {
        let ranking_ids: Vec<_> = self.data.rankings.keys().copied().collect();
        let metrics: Vec<_> = self
            .plugins
            .metrics()
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

    /// Adds the matches and byes the format schedules next, numbered after `highest` (see
    /// [`Self::start`]), and updates the metrics for every new bye its ranking reaches.
    pub(crate) fn schedule(&mut self, highest: &mut u32) {
        let Schedule { matches, byes } = self.plugins.schedule(&self.data);
        let reserved = self
            .format()
            .highest_number()
            .map_or(0, |number| number.get().saturating_add(self.number_base));
        let matches = DivisionData::number(matches, self.number_base, reserved, highest);
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

    /// Reports `decision` alone for the match `match_id`, see [`Division::report`].
    #[cfg(test)]
    pub(crate) fn report_match_result(&mut self, match_id: MatchId, decision: impl Into<Decision>) {
        self.report_match(match_id, Part::new().with_decision(decision));
    }

    /// Reports what was `entered` for the match `match_id` of this division standing alone, see
    /// [`Division::report`].
    #[cfg(test)]
    pub(crate) fn report_match(&mut self, match_id: MatchId, entered: Part) {
        let mut highest = self.highest_number();
        self.report(match_id, entered, &mut highest);
    }

    /// Schedules what the format schedules next in this division standing alone, see
    /// [`Division::schedule`].
    #[cfg(test)]
    pub(crate) fn create_new_matches(&mut self) {
        let mut highest = self.highest_number();
        self.schedule(&mut highest);
    }

    /// The highest number of a match of this division standing alone, or of those its format
    /// keeps.
    #[cfg(test)]
    fn highest_number(&self) -> u32 {
        self.data
            .matches
            .values()
            .map(|r#match| r#match.number().get())
            .chain(self.format().highest_number().map(NonZeroU32::get))
            .max()
            .unwrap_or(0)
    }

    /// Finishes the match `match_id` with what was `entered` for it: its decision, values and
    /// parts, as its [contest](Match::contest), and schedules what follows, numbered after
    /// `highest` (see [`Self::start`]).
    ///
    /// # Panics
    /// Panics if the match is finished, or `entered` has no decision.
    pub(crate) fn report(&mut self, match_id: MatchId, entered: Part, highest: &mut u32) {
        assert!(entered.decision().is_some(), "a report decides its match");
        let r#match = self.data.matches.get_mut(&match_id).unwrap();
        assert!(!r#match.is_finished());
        r#match.contest.enter(entered);
        let tallies = self
            .plugins
            .match_format()
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
        self.schedule(highest);
    }

    /// Corrects the finished match `match_id` in place to what was `entered` for it: its decision,
    /// values and parts, as its [contest](Match::contest), keeping everything scheduled. The
    /// metrics are calculated anew by `calculators`, as configured for the phase, over every
    /// finished match and reached bye, see [`Self::recount`].
    ///
    /// # Panics
    /// Panics if the match is not finished, or `entered` has no decision.
    pub(crate) fn correct(
        &mut self,
        match_id: MatchId,
        entered: Part,
        calculators: DependencyOrderedSet<dyn MetricsCalculator>,
    ) {
        assert!(
            entered.decision().is_some(),
            "a correction decides its match"
        );
        let r#match = self.data.matches.get_mut(&match_id).unwrap();
        assert!(r#match.is_finished());
        r#match.contest.enter(entered);
        let tallies = self
            .plugins
            .match_format()
            .tally(&self.data, &self.data.matches[&match_id]);
        self.data.matches.get_mut(&match_id).unwrap().tallies = tallies;
        self.recount(calculators);
    }

    /// Calculates every metric anew by `calculators`: from its default value, as if every finished
    /// match was reported and every reached bye reached again, round by round, a round's matches
    /// before its byes. As the metrics do not depend on the order results come in (see
    /// [`MetricsCalculator`]), they are those the results and byes give.
    fn recount(&mut self, calculators: DependencyOrderedSet<dyn MetricsCalculator>) {
        self.plugins.set_metrics(calculators);
        for ranking in self.data.rankings.values_mut() {
            ranking.played_matches.clear();
        }
        self.initialize_metrics();
        let mut counted: Vec<_> = self
            .data
            .matches
            .values()
            .filter(|r#match| r#match.is_finished())
            .map(|r#match| (r#match.round(), Counted::Match(r#match.id())))
            .chain(
                self.data
                    .byes
                    .values()
                    .filter(|bye| bye.reached)
                    .map(|bye| (bye.round(), Counted::Bye(bye.id()))),
            )
            .collect();
        // stable, so the matches of a round keep their order, before its byes
        counted.sort_by_key(|(round, _)| *round);
        for (_, counted) in counted {
            match counted {
                Counted::Match(match_id) => {
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
                }
                Counted::Bye(bye_id) => {
                    self.update_metrics(|metrics_calculator, data, old_metrics| {
                        metrics_calculator.update_after_bye(data, &data.byes[&bye_id], old_metrics)
                    });
                }
            }
        }
    }

    /// Runs every metrics calculator, in order, through `updates_of` and stores the metrics it
    /// returns, handing each calculator the metrics replaced by the ones before it.
    fn update_metrics(
        &mut self,
        mut updates_of: impl FnMut(
            &mut dyn MetricsCalculator,
            &DivisionData,
            &OldMetrics,
        ) -> Vec<(RankingId, BoxedMetric)>,
    ) {
        let mut old_metrics = OldMetrics::default();
        for metrics_calculator in self.plugins.metrics_mut() {
            let updates = updates_of(metrics_calculator, &self.data, &old_metrics);
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

// A division is saved as a phase was when its rankings stored their ranks and its data the order
// of its standings: they are computed when it is written, and read but dropped when it is read,
// since a format that is not self-describing cannot skip them.
impl Serialize for Division {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "Division")]
        struct Saved<'a> {
            plugins: &'a PhasePlugins,
            data: SavedData<'a>,
            number_base: u32,
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
            lot: NonZeroU32,
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
                    lot: ranking.lot,
                    unique_rank: standing.unique_rank(),
                    rank: standing.rank(),
                    played_matches: &ranking.played_matches,
                    metrics: &ranking.metrics,
                }
            })
            .collect();
        Saved {
            plugins: &self.plugins,
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
            number_base: self.number_base,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Division {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "Division")]
        struct Saved {
            plugins: PhasePlugins,
            data: SavedData,
            number_base: u32,
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
            lot: NonZeroU32,
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
                lot: ranking.lot,
                played_matches: ranking.played_matches,
                metrics: ranking.metrics,
            })
            .collect();
        Ok(Division {
            plugins: saved.plugins,
            data: DivisionData {
                rankings,
                matches: saved.data.matches,
                byes: saved.data.byes,
                seed: saved.data.seed,
            },
            number_base: saved.number_base,
        })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        DivisionBuilder, MatchResult,
        plugins::metrics_calculator::test_utils::{self, match_id},
    };

    /// The played matches of the ranking with the given index in the ordering by initial unique
    /// rank.
    fn played(division: &Division, ranking_index: usize) -> &[MatchId] {
        division
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
        let mut division = test_utils::phase_with(DivisionBuilder::new().add_players(6), matches);
        let match_id = match_id(&division, 1, &[0, 1], &[2, 3]);

        division.report_match_result(match_id, MatchResult::BWins);

        for ranking_index in 0..4 {
            assert_eq!(played(&division, ranking_index), [match_id]);
        }
        // the rankings of the other, still unreported match have played nothing
        assert!(played(&division, 4).is_empty());
        assert!(played(&division, 5).is_empty());
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
        let mut division =
            test_utils::phase_with(DivisionBuilder::new().add_players(4), matches.clone());
        let match_ids: Vec<_> = matches
            .iter()
            .map(|(round, team_a, team_b)| match_id(&division, *round, team_a, team_b))
            .collect();
        let [m_0_1, m_2_3, m_0_2, m_1_3, m_1_2] = match_ids[..] else {
            unreachable!("five matches are scheduled")
        };

        // reported in the reverse order of the schedule
        for match_id in match_ids.iter().rev() {
            division.report_match_result(*match_id, MatchResult::AWins);
        }

        assert_eq!(played(&division, 0), [m_0_2, m_0_1]);
        assert_eq!(played(&division, 1), [m_1_2, m_1_3, m_0_1]);
        assert_eq!(played(&division, 2), [m_1_2, m_0_2, m_2_3]);
        assert_eq!(played(&division, 3), [m_1_3, m_2_3]);
    }
}
