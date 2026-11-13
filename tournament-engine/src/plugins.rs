//! The plugin axes of a phase and the plugins of this crate, built on `plugin_factory`.
//!
//! A phase is played in one [`PhaseFormat`], ranked by a list of [`SliceRanker`]s and calculates
//! the metrics of a list of [`MetricsCalculator`]s; [`PhasePlugins`] is that composite. Every
//! plugin registers itself in the catalog of its axis, see [`plugin_factory::catalog`], so a
//! consumer configuring a phase finds them there, and declares what it provides to and requires of
//! its phase, see [`facts`] and [`plugin_factory::compatibility`].
//!
//! # Labelling a plugin
//!
//! A consumer shows a plugin and its fields by their labels and descriptions alone, in lists of
//! options, in questions (`random rounds: rounds`) and in summaries (`round robin, ranked by more
//! points, then more Sonneborn-Berger`; `single elimination (match for third place: yes)`). A
//! plugin of any crate is shown next to the plugins of this one, so it should be labelled the same
//! way:
//!
//! - **Labels are lowercase noun phrases** without a final period, written as they read in the
//!   middle of a sentence. Proper names and established abbreviations keep their case
//!   (`Monster DYP`, `Buchholz`). A consumer capitalizes a label where a sentence starts with it.
//!   Labels contain no commas, since consumers list them separated by commas.
//! - **A format is labelled by the name of the format** (`round robin`, `double elimination`).
//! - **A metrics calculator is labelled by the metric it calculates**, as a column of a ranking
//!   would be named (`points`, `win-draw-loss record`), not by what it does.
//! - **A criterion states which way it sorts**: a comparative followed by what it compares, so that
//!   `ranked by <label>` reads as a sentence (`more points`, `fewer losses`, `later first loss`).
//!   Two criteria of one metric are told apart by their labels alone.
//! - **A field is labelled by what its answer is**, so that `<label>: <answer>` reads naturally
//!   (`rounds: 5`, `match for third place: yes`, `points for a win: 1`), not as a question or a
//!   command (not `Play a match for third place?`). A consumer prefixes it with the label of its
//!   plugin, so it does not repeat that.
//! - **A description says what the label leaves open**, as a lowercase phrase without a final
//!   period: what a format does, what a metric counts, when to rank by a criterion (`a tie break
//!   favouring those who met stronger opponents`) or how it treats its extremes (`never having
//!   lost is best`), what an answer means. Every criterion has one, since its label does not say
//!   when to choose it. Leave it out rather than restate the
//!   label (no `more points are better` for `more points`).
//! - **A field offers an answer**, with `#[plugin(default = ..)]`, wherever one value suits most
//!   phases, e.g. the scoring of chess for points.
//!
//! Labels and descriptions are English. A consumer may translate them, finding them by the
//! plugin's id and the field's name, and shows the English where it has no translation.

use plugin_factory::{
    config::{Config, ConfigError, Path, PluginConfig, Value},
    draft::Draft,
    schema::{Configurable, DependencyOrderedSet},
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "test-util")]
pub use crate::plugins::builder::PhasePluginsBuilder;
pub use crate::plugins::phrase::Phrase;
use crate::{
    entities::{PhaseData, PhaseView},
    plugins::{
        match_format::MatchFormat,
        metrics_calculator::MetricsCalculator,
        phase_format::{PhaseFormat, Schedule},
        ranker::{HierarchicalRanker, SliceRanker},
    },
};

#[cfg(feature = "test-util")]
mod builder;
pub mod facts;
pub mod match_format;
pub mod metrics_calculator;
pub mod phase_format;
mod phrase;
pub mod ranker;

/// The plugins of one phase: the format its matches are scheduled by, the slice rankers it is
/// ranked by and the calculators of its metrics.
///
/// It is configured as a composite of `plugin_factory`, through the
/// [`Competition::phase_draft`](crate::Competition::phase_draft) of the competition it is added to,
/// or loaded. Tests can build it directly with `PhasePluginsBuilder` (feature `test-util`).
///
/// The calculators are a [`DependencyOrderedSet`]: a phase calculates each metric once, and the
/// set holds them in the order they run in, each after the calculators of the metrics it reads.
///
/// # Examples
/// ```
/// use tournament_engine::{
///     Competition,
///     plugin_factory::config::PluginConfig,
///     plugins::{
///         metrics_calculator::{MorePoints, PointsCalculator},
///         phase_format::RoundRobin,
///     },
/// };
///
/// let mut draft = Competition::default()
///     .phase_draft()
///     .with("format", PluginConfig::of::<RoundRobin>())?
///     .with("ranking", vec![PluginConfig::of::<MorePoints>()])?
///     .with(
///         "metrics",
///         vec![PluginConfig::of::<PointsCalculator>().with("points_for_win", 3.0)],
///     )?
///     .with_defaults()?;
/// // the points are scored from the wins, draws and losses, which implies their calculator
/// draft.add_implied();
/// let plugins = draft.build().expect("the plugins fit together");
///
/// // the calculators run in the order of what they read
/// assert_eq!(
///     format!("{:?}", plugins.metrics()),
///     "DependencyOrderedSet([WinsDrawsLossesCalculator, PointsCalculator { points_for_win: \
///      3, points_for_draw: 0.5, points_for_bye: 1 }])"
/// );
/// # Ok::<(), plugin_factory::config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Configurable)]
pub struct PhasePlugins {
    #[plugin(
        label = "format",
        description = "how the matches are scheduled",
        default = Box::new(phase_format::RoundRobin)
    )]
    format: Box<dyn PhaseFormat>,
    #[plugin(
        label = "match format",
        description = "what is entered for a match",
        default = match_format::default()
    )]
    match_format: Box<dyn MatchFormat>,
    #[serde(default)]
    #[plugin(
        label = "ranked by",
        description = "best first, every criterion only breaks the ties of the ones before it"
    )]
    ranking: Vec<Box<dyn SliceRanker>>,
    #[serde(default)]
    #[plugin(label = "metrics", description = "calculated for every entrant")]
    metrics: DependencyOrderedSet<dyn MetricsCalculator>,
}

impl PhasePlugins {
    /// The name of the field taking the format.
    pub const FORMAT: &str = "format";
    /// The name of the field taking the match format.
    pub const MATCH_FORMAT: &str = "match_format";
    /// The name of the field taking the ranking criteria.
    pub const RANKING: &str = "ranking";
    /// The name of the field taking the metrics calculators.
    pub const METRICS: &str = "metrics";
    /// The names of the fields the format suggests answers for, see
    /// [`PhaseFormat::suggestions`].
    pub const SUGGESTED: [&str; 2] = [Self::RANKING, Self::METRICS];

    /// The suggested answer for a root ranking or metrics field, otherwise its schema default.
    /// Suggestions come only from the configured format and are not filtered for compatibility.
    ///
    /// # Errors
    /// Returns a configuration error if the path is unknown. An incomplete or rejected format
    /// supplies no suggestions; validation reports its problems independently.
    pub fn default_value(draft: &Draft<Self>, path: &Path) -> Result<Option<Value>, ConfigError> {
        let ranking = *path == Path::ROOT.field(Self::RANKING);
        if (ranking || *path == Path::ROOT.field(Self::METRICS))
            && let Some(format) = draft
                .answers()
                .get(Self::FORMAT)
                .and_then(PluginConfig::from_answer)
                .and_then(|config| config.build::<dyn PhaseFormat>().ok())
        {
            let suggestions = format.suggestions();
            return Ok(Some(if ranking {
                suggestions.ranking.into()
            } else {
                suggestions.metrics.into()
            }));
        }
        draft.default_value(path)
    }

    /// Fills unanswered fields with schema defaults and the format's suggested ranking and metrics.
    /// Existing choices are preserved. No implied plugins are added.
    ///
    /// # Errors
    /// Returns a configuration error if a default or suggestion does not fit its schema.
    pub fn with_suggestions(mut draft: Draft<Self>) -> Result<Draft<Self>, ConfigError> {
        let unanswered: Vec<_> = Self::SUGGESTED
            .into_iter()
            .filter(|field| draft.answers().get(field).is_none())
            .collect();
        draft = draft.with_defaults()?;
        for field in unanswered {
            let path = Path::ROOT.field(field);
            if let Some(answer) = Self::default_value(&draft, &path)? {
                draft.set(&path, answer)?;
            }
        }
        draft.with_defaults()
    }

    /// The plugins of a phase played in `format`, entering results by `match_format`, ranked by
    /// `ranking`, calculating the metrics of `metrics`, put into the order they run in.
    pub(crate) fn from_parts(
        format: Box<dyn PhaseFormat>,
        match_format: Box<dyn MatchFormat>,
        ranking: Vec<Box<dyn SliceRanker>>,
        metrics: Vec<Box<dyn MetricsCalculator>>,
    ) -> Result<Self, plugin_factory::schema::DependencyOrderedSetError> {
        Ok(Self {
            format,
            match_format,
            ranking,
            metrics: DependencyOrderedSet::new(metrics)?,
        })
    }

    pub fn format(&self) -> &dyn PhaseFormat {
        &*self.format
    }

    /// The slice rankers, best first.
    pub fn ranking(&self) -> &[Box<dyn SliceRanker>] {
        &self.ranking
    }

    /// The metrics calculators, in the order they run in.
    pub fn metrics(&self) -> &DependencyOrderedSet<dyn MetricsCalculator> {
        &self.metrics
    }

    /// The match format: what is entered for a match.
    pub fn match_format(&self) -> &dyn MatchFormat {
        &*self.match_format
    }

    /// The answers to a [`Competition::phase_draft`](crate::Competition::phase_draft) these
    /// plugins are built from, without their runtime state, see [`Config::of`].
    pub fn config(&self) -> Config {
        Config::of(self).expect("every plugin serializes as it is configured")
    }

    /// The ranker of a phase with these plugins: its slice rankers in order.
    pub(crate) fn ranker(&self) -> HierarchicalRanker<'_> {
        HierarchicalRanker::new(&self.ranking)
    }

    /// The matches and byes the format schedules next in the phase of `data` played with these
    /// plugins, see [`PhaseFormat::create_new_matches`].
    pub(crate) fn schedule(&mut self, data: &PhaseData) -> Schedule {
        let view = PhaseView::new(data, HierarchicalRanker::new(&self.ranking));
        self.format.create_new_matches(&view)
    }

    /// The metrics calculators, mutably and in the order they run in, to update metrics.
    pub(crate) fn metrics_mut(&mut self) -> impl Iterator<Item = &mut dyn MetricsCalculator> {
        self.metrics.iter_mut()
    }
}

/// What the plugins of this crate declare, i.e. the facts they provide to and require of their
/// phase, which plugins they imply and how a phase orders them. The mechanism itself is tested in
/// `plugin_factory`.
#[cfg(test)]
mod tests;
