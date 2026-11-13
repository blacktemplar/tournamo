use std::num::NonZeroU32;

use plugin_factory::{Plugin, config::PluginConfig, register};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::{
    entities::PhaseView,
    plugins::{
        metrics_calculator::{
            buchholz::{
                BuchholzCalculator, BuchholzOfBuchholzCalculator, MoreBuchholz,
                MoreBuchholzOfBuchholz, MoreSonnebornBerger, SonnebornBergerCalculator,
            },
            points::{MorePoints, PointsCalculator},
            wins_draws_losses::WinsDrawsLossesCalculator,
        },
        phase_format::{PhaseFormat, PhaseSuggestions, Schedule, round_robin::circle::Circle},
    },
};

/// Randomly chosen rounds of a round robin, all scheduled at once.
///
/// If there are more rounds than a full round robin has, a full round robin is scheduled instead.
/// With an odd number of rankings, one ranking has a bye in every round, and no ranking has two.
#[derive(Debug, Clone, Deserialize, Serialize, Plugin)]
#[plugin(
    id = "tournament_engine::random_rounds",
    label = "random rounds",
    description = "for many entrants and little time: some rounds of a round robin, chosen at \
                   random, so that nobody meets anyone twice"
)]
pub struct RandomRounds {
    #[plugin(
        label = "rounds",
        description = "a full round robin is scheduled if that is shorter",
        min = 1,
        default = 5
    )]
    rounds: u32,
}

impl RandomRounds {
    /// Schedules `rounds` randomly chosen rounds of a round robin.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new(rounds: u32) -> Self {
        Self { rounds }
    }
}

#[register]
impl PhaseFormat for RandomRounds {
    /// Ranked by points, then Buchholz, Sonneborn-Berger and Buchholz of Buchholz, and showing the
    /// wins, draws and losses as well.
    fn suggestions(&self) -> PhaseSuggestions {
        PhaseSuggestions {
            ranking: vec![
                PluginConfig::of::<MorePoints>(),
                PluginConfig::of::<MoreBuchholz>(),
                PluginConfig::of::<MoreSonnebornBerger>(),
                PluginConfig::of::<MoreBuchholzOfBuchholz>(),
            ],
            metrics: vec![
                PluginConfig::of::<WinsDrawsLossesCalculator>(),
                PluginConfig::of::<PointsCalculator>(),
                PluginConfig::of::<BuchholzCalculator>(),
                PluginConfig::of::<SonnebornBergerCalculator>(),
                PluginConfig::of::<BuchholzOfBuchholzCalculator>(),
            ],
        }
    }

    fn create_new_matches(&mut self, phase: &PhaseView<'_>) -> Schedule {
        let phase_data = phase.data();
        // all rounds are scheduled at once
        if phase_data.has_schedule() {
            return Schedule::default();
        }
        let circle = Circle::new(phase_data);
        // every rotation in an order drawn whatever the number of rounds, the first ones taken, so
        // that more rounds keep those drawn for fewer, e.g. when the phase is changed after the
        // start
        let mut rotations: Vec<_> = (0..circle.rotations()).collect();
        rotations.shuffle(&mut phase_data.rng());
        rotations.truncate(self.rounds as usize);
        let mut schedule = Schedule::default();
        for (rotation, round) in rotations.into_iter().zip(NonZeroU32::MIN..) {
            circle.schedule_round(rotation, round, &mut schedule);
        }
        schedule
    }
}

#[cfg(test)]
mod tests;
