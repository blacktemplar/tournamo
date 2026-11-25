use std::num::NonZeroU32;

use plugin_factory::schema::DependencyOrderedSetError;

use crate::{
    PhasePlugins,
    plugins::{
        draw::{Draw, Snake},
        match_format::{self, MatchFormat},
        metrics_calculator::MetricsCalculator,
        phase_format::{PhaseFormat, RoundRobin},
        ranker::SliceRanker,
        seeding::{SeededInOrder, Seeding},
        teams::{Keep, Teams},
    },
};

/// Builds [`PhasePlugins`] directly. Without a format, the phase is a [`RoundRobin`]; without a
/// ranking, it leaves every entrant tied; without metrics, it calculates none; everyone plays as
/// they come in ([`Keep`]), in one division, dealt [`Snake`] and [`SeededInOrder`], and ranked
/// across divisions by nothing but the rank in them.
///
/// Nothing else is added: unlike a draft, the builder adds no plugin the others imply. Nor are the
/// plugins checked against each other, so a fact one of them requires may be missing; adding them
/// to a competition with [`AddPhase`](crate::AddPhase) checks that.
///
/// Requires the `test-util` feature.
///
/// # Examples
/// ```
/// use tournament_engine::{
///     PhasePluginsBuilder, Score,
///     plugins::{
///         metrics_calculator::{MorePoints, PointsCalculator, WinsDrawsLossesCalculator},
///         phase_format::SingleElimination,
///     },
/// };
///
/// // a round robin, neither ranked by anything nor calculating any metrics
/// let round_robin = PhasePluginsBuilder::new().build()?;
/// assert!(round_robin.ranking().is_empty());
///
/// let knockout = PhasePluginsBuilder::new()
///     .format(SingleElimination::new())
///     .add_ranking(MorePoints)
///     .add_metric(PointsCalculator::new(Score::new(3.0), Score::ONE))
///     .add_metric(WinsDrawsLossesCalculator)
///     .build()?;
/// // the points are scored from the wins, draws and losses, so those are calculated first
/// assert_eq!(
///     format!("{:?}", knockout.metrics()),
///     "DependencyOrderedSet([WinsDrawsLossesCalculator, PointsCalculator { points_for_win: \
///      3, points_for_draw: 1, points_for_bye: 0 }])"
/// );
/// # Ok::<(), tournament_engine::plugin_factory::schema::DependencyOrderedSetError>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct PhasePluginsBuilder {
    format: Option<Box<dyn PhaseFormat>>,
    teams: Option<Box<dyn Teams>>,
    match_format: Option<Box<dyn MatchFormat>>,
    ranking: Vec<Box<dyn SliceRanker>>,
    metrics: Vec<Box<dyn MetricsCalculator>>,
    divisions: Option<NonZeroU32>,
    draw: Option<Box<dyn Draw>>,
    seeding: Option<Box<dyn Seeding>>,
    across: Vec<Box<dyn SliceRanker>>,
}

impl PhasePluginsBuilder {
    /// A builder with nothing given, i.e. of a round robin neither ranked by anything nor
    /// calculating any metrics, whose results are only who won.
    pub fn new() -> Self {
        Self::default()
    }

    /// Plays the phase in `format`, replacing a format given before.
    pub fn format(mut self, format: impl PhaseFormat) -> Self {
        self.format = Some(Box::new(format));
        self
    }

    /// Forms the teams by `teams` instead of [`Keep`].
    pub fn teams(mut self, teams: impl Teams) -> Self {
        self.teams = Some(Box::new(teams));
        self
    }

    /// Enters the results of matches by `format`, replacing a match format given before.
    pub fn match_format(mut self, format: impl MatchFormat) -> Self {
        self.match_format = Some(Box::new(format));
        self
    }

    /// Ranks the phase by `criterion` after the criteria given before, i.e. `criterion` breaks the
    /// ties those leave.
    pub fn add_ranking(mut self, criterion: impl SliceRanker) -> Self {
        self.ranking.push(Box::new(criterion));
        self
    }

    /// Plays the phase in `divisions` divisions instead of one.
    ///
    /// # Panics
    /// Panics if `divisions` is 0.
    pub fn divisions(mut self, divisions: u32) -> Self {
        self.divisions = Some(NonZeroU32::new(divisions).expect("at least one division"));
        self
    }

    /// Deals the field to the divisions by `draw` instead of [`Snake`].
    pub fn draw(mut self, draw: impl Draw) -> Self {
        self.draw = Some(Box::new(draw));
        self
    }

    /// Orders the qualifiers of each division by `seeding` instead of [`SeededInOrder`].
    pub fn seeding(mut self, seeding: impl Seeding) -> Self {
        self.seeding = Some(Box::new(seeding));
        self
    }

    /// Ranks the rankings of one rank in parallel divisions by `criterion` after the criteria
    /// given before.
    pub fn add_across(mut self, criterion: impl SliceRanker) -> Self {
        self.across.push(Box::new(criterion));
        self
    }

    /// Calculates the metric of `calculator` as well.
    pub fn add_metric(self, calculator: impl MetricsCalculator) -> Self {
        self.add_boxed_metric(Box::new(calculator))
    }

    /// Calculates the metric of `calculator` as well, see [`Self::add_metric`].
    pub fn add_boxed_metric(mut self, calculator: Box<dyn MetricsCalculator>) -> Self {
        self.metrics.push(calculator);
        self
    }

    /// The plugins given, with the calculators put into the order they run in, see
    /// [`PhasePluginsBuilder`].
    ///
    /// # Errors
    /// Returns an error if a calculator is given twice, or calculators require facts of each other
    /// in a cycle, see [`DependencyOrderedSet::new`].
    ///
    /// [`DependencyOrderedSet::new`]: plugin_factory::schema::DependencyOrderedSet::new
    pub fn build(self) -> Result<PhasePlugins, DependencyOrderedSetError> {
        let format = self.format.unwrap_or_else(|| Box::new(RoundRobin));
        let match_format = self.match_format.unwrap_or_else(match_format::default);
        Ok(
            PhasePlugins::from_parts(format, match_format, self.ranking, self.metrics)?
                .with_divisions(
                    self.divisions.unwrap_or(NonZeroU32::MIN),
                    (
                        self.draw.unwrap_or_else(|| Box::new(Snake)),
                        self.seeding.unwrap_or_else(|| Box::new(SeededInOrder)),
                    ),
                    self.across,
                )
                .with_teams(self.teams.unwrap_or_else(|| Box::new(Keep))),
        )
    }
}

#[cfg(test)]
mod tests {
    use plugin_factory::{compatibility, schema::Configurable};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{Competition, Source};

    #[test]
    fn nothing_given_is_a_round_robin_that_fits_a_competition() {
        let plugins = PhasePluginsBuilder::new().build().unwrap();

        assert_eq!(format!("{:?}", plugins.format()), "RoundRobin");
        assert!(plugins.ranking().is_empty());
        assert!(plugins.metrics().is_empty());
        assert_eq!(
            compatibility::check(
                |declaration| Competition::default()
                    .declare_phase_context(&[Source::registration()], declaration),
                &plugins.plugins(),
            ),
            Ok(())
        );
    }
}
