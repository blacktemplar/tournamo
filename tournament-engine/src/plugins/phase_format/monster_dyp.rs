use std::num::NonZeroU32;

use itertools::Itertools;
use plugin_factory::{Plugin, compatibility::Declaration, config::PluginConfig, register};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use vec1::vec1;

use crate::{
    entities::{Bye, NewMatch, PhaseView},
    plugins::{
        facts::EntrantsPerRanking,
        metrics_calculator::{
            points::{MorePoints, PointsCalculator},
            wins_draws_losses::WinsDrawsLossesCalculator,
        },
        phase_format::{PhaseFormat, PhaseSuggestions, Schedule},
    },
};

/// A Monster DYP ("draw your partner"): every round, the rankings are drawn into random teams of
/// two, and every team plays another randomly drawn team.
///
/// Every ranking keeps its own place in the ranking and gets the result of every match its team
/// played. If the number of rankings is not a multiple of four, the ones left over sit the round
/// out, each with a bye. Those are always drawn from the rankings that sat out the fewest rounds so
/// far, so the numbers of matches the rankings play differ by at most one. With fewer than four
/// rankings nothing is scheduled.
///
/// All rounds are scheduled at once, when the phase starts.
#[derive(Debug, Clone, Deserialize, Serialize, Plugin)]
#[plugin(
    id = "tournament_engine::monster_dyp",
    label = "Monster DYP",
    description = "“draw your partner”, as in foosball: every round, entrants are drawn into \
                   teams of two that play each other, and each entrant is ranked on its own",
    declare_static = Self::declare_static
)]
pub struct MonsterDyp {
    // all rounds are scheduled at once, so far more than a day's play would hold the page up
    #[plugin(
        label = "rounds",
        description = "all of them are drawn at the start",
        min = 1,
        max = 100,
        default = 5
    )]
    rounds: u32,
}

impl MonsterDyp {
    /// A Monster DYP of `rounds` rounds.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new(rounds: u32) -> Self {
        Self { rounds }
    }

    fn declare_static(declaration: &mut Declaration) {
        // the teams are drawn per match, so a ranking that already stands for a team of two
        // cannot be drawn into one
        declaration.require_that::<EntrantsPerRanking>(
            "the entrants per ranking",
            "rankings of single players",
            |entrants| *entrants == EntrantsPerRanking::SINGLE,
        );
    }
}

#[register]
impl PhaseFormat for MonsterDyp {
    /// Ranked by points, and showing the wins, draws and losses as well.
    fn suggestions(&self) -> PhaseSuggestions {
        // sitting a round out scores like a win, so the matches played need no tie break
        PhaseSuggestions {
            ranking: vec![PluginConfig::of::<MorePoints>()],
            metrics: vec![
                PluginConfig::of::<WinsDrawsLossesCalculator>(),
                PluginConfig::of::<PointsCalculator>(),
            ],
        }
    }

    fn min_entrants(&self) -> usize {
        4
    }

    fn create_new_matches(&mut self, phase: &PhaseView<'_>) -> Schedule {
        let phase_data = phase.data();
        // all rounds are scheduled at once
        if phase_data.has_schedule() || phase_data.rankings().len() < 4 {
            return Schedule::default();
        }
        let ranking_ids = phase_data.rankings().keys().copied().collect_vec();
        let num_sitting_out = ranking_ids.len() % 4;
        let mut rounds_sat_out = vec![0_u32; ranking_ids.len()];
        let mut order = (0..ranking_ids.len()).collect_vec();
        let mut rng = phase_data.rng();
        let mut schedule = Schedule::default();
        for round in 1..=self.rounds {
            let round = NonZeroU32::new(round).unwrap();
            order.shuffle(&mut rng);
            // the sort is stable, so the shuffle decides between rankings that sat out equally
            // often
            let sitting_out = order
                .iter()
                .copied()
                .sorted_by_key(|&i| rounds_sat_out[i])
                .take(num_sitting_out)
                .collect_vec();
            for &i in &sitting_out {
                rounds_sat_out[i] += 1;
                schedule.byes.push(Bye::new(ranking_ids[i], round));
            }
            // the playing rankings are taken in shuffled order rather than in the sorted one above,
            // which would team up rankings that sat out equally often
            schedule.matches.extend(
                order
                    .iter()
                    .filter(|i| !sitting_out.contains(i))
                    .map(|&i| ranking_ids[i])
                    .tuples()
                    .map(|(a1, a2, b1, b2)| NewMatch::new(vec1![a1, a2], vec1![b1, b2], round)),
            );
        }
        schedule
    }
}

#[cfg(test)]
mod tests;
