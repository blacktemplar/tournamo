use uuid::Uuid;
use vec1::vec1;

#[cfg(doc)]
use crate::Competition;
use crate::{
    EntrantId, Phase, PhaseId, Seed,
    plugins::{
        PhasePluginsBuilder, match_format::MatchFormat, metrics_calculator::MetricsCalculator,
        phase_format::PhaseFormat, ranker::SliceRanker,
    },
};

/// Builds a standalone [`Phase`] with rankings of single players, without a [`Competition`]
/// around it, for unit tests of plugins.
///
/// Its plugins are given as to a [`PhasePluginsBuilder`], with the same defaults: a round robin,
/// neither ranked by anything nor calculating any metrics. Without players, the phase has no
/// rankings.
///
/// Requires the `test-util` feature.
///
/// # Examples
/// ```
/// use tournament_engine::{
///     PhaseBuilder,
///     plugins::{metrics_calculator::WinsDrawsLossesCalculator, phase_format::SingleElimination},
/// };
///
/// let phase = PhaseBuilder::new()
///     .format(SingleElimination::new())
///     .add_metric(WinsDrawsLossesCalculator)
///     .add_players(4)
///     .build();
///
/// assert_eq!(phase.data().rankings().len(), 4);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PhaseBuilder {
    plugins: PhasePluginsBuilder,
    players: u32,
}

impl PhaseBuilder {
    /// A builder with nothing given, i.e. of a round robin without rankings, neither ranked by
    /// anything nor calculating any metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Plays the phase in `format`, see [`PhasePluginsBuilder::format`].
    pub fn format(mut self, format: impl PhaseFormat) -> Self {
        self.plugins = self.plugins.format(format);
        self
    }

    /// Ranks the phase by `criterion` as well, see [`PhasePluginsBuilder::add_ranking`].
    pub fn add_ranking(mut self, criterion: impl SliceRanker) -> Self {
        self.plugins = self.plugins.add_ranking(criterion);
        self
    }

    /// Calculates the metric of `calculator` as well, see [`PhasePluginsBuilder::add_metric`].
    /// Its metric is initialized for every ranking when the phase is built.
    pub fn add_metric(mut self, calculator: impl MetricsCalculator) -> Self {
        self.plugins = self.plugins.add_metric(calculator);
        self
    }

    /// Enters the results of matches by `format`, see [`PhasePluginsBuilder::match_format`].
    pub fn match_format(mut self, format: impl MatchFormat) -> Self {
        self.plugins = self.plugins.match_format(format);
        self
    }

    /// Adds `players` more rankings, each a single entrant, ranked after the rankings already
    /// added.
    pub fn add_players(mut self, players: u32) -> Self {
        self.players += players;
        self
    }

    /// The phase, with the metrics of its rankings initialized and nothing scheduled yet.
    ///
    /// # Panics
    /// Panics if building its plugins fails, see [`PhasePluginsBuilder::build`].
    pub fn build(self) -> Phase {
        let plugins = self
            .plugins
            .build()
            .expect("the plugins of the phase can be ordered");
        let mut phase = Phase::new(PhaseId::new(Uuid::nil()), plugins, Seed::default());
        phase.set_rankings(
            (0..self.players)
                .map(|index| vec1![EntrantId::new(Uuid::from_u128(index.into()))])
                .collect(),
        );
        phase
    }
}
