use std::num::NonZeroU32;

use plugin_factory::{Plugin, config::PluginConfig, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::PhaseView,
    plugins::{
        metrics_calculator::{
            buchholz::{MoreSonnebornBerger, SonnebornBergerCalculator},
            points::{MorePoints, PointsCalculator},
            wins_draws_losses::WinsDrawsLossesCalculator,
        },
        phase_format::{PhaseFormat, PhaseSuggestions, Schedule},
    },
};

mod circle;
pub mod random_rounds;

use circle::Circle;

/// Everyone plays everyone once. With an odd number of rankings, every ranking has a bye in one of
/// the rounds.
///
/// All rounds are scheduled at once, when the phase starts.
#[derive(Debug, Clone, Deserialize, Serialize, Plugin)]
#[plugin(
    id = "tournament_engine::round_robin",
    label = "round robin",
    description = "everyone plays everyone"
)]
pub struct RoundRobin;

#[register]
impl PhaseFormat for RoundRobin {
    /// Ranked by points, then Sonneborn-Berger, and showing the wins, draws and losses as well.
    fn suggestions(&self) -> PhaseSuggestions {
        PhaseSuggestions {
            ranking: vec![
                PluginConfig::of::<MorePoints>(),
                PluginConfig::of::<MoreSonnebornBerger>(),
            ],
            metrics: vec![
                PluginConfig::of::<WinsDrawsLossesCalculator>(),
                PluginConfig::of::<PointsCalculator>(),
                PluginConfig::of::<SonnebornBergerCalculator>(),
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
        let mut schedule = Schedule::default();
        for (rotation, round) in (0..circle.rotations()).zip(NonZeroU32::MIN..) {
            circle.schedule_round(rotation, round, &mut schedule);
        }
        schedule
    }
}

#[cfg(test)]
mod tests;
