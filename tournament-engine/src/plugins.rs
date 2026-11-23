//! The plugin axes of a phase and the plugins of this crate, built on `plugin_factory`.
//!
//! A phase is played in one [`PhaseFormat`] between the teams its [`Teams`] form of its field, by
//! divisions its [`Draw`] deals them to, each in
//! the order its [`Seeding`] gives it, ranked by a list of [`SliceRanker`]s and calculates the
//! metrics of a list of [`MetricsCalculator`]s; [`PhasePlugins`] is that composite. Every plugin
//! registers itself in the catalog of its axis, see [`plugin_factory::catalog`], so a
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

use std::num::NonZeroU32;

pub use plugin_factory::Phrase;
use plugin_factory::{
    catalog::Axis,
    compatibility::Declaration,
    config::{Config, ConfigError, Path, PluginConfig, Value},
    draft::Draft,
    schema::{Configurable, DependencyOrderedSet},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[cfg(feature = "test-util")]
pub use crate::plugins::builder::PhasePluginsBuilder;
use crate::{
    entities::{DivisionData, DivisionView},
    plugins::{
        draw::Draw,
        match_format::MatchFormat,
        metrics_calculator::MetricsCalculator,
        phase_format::{PhaseFormat, Schedule},
        ranker::{HierarchicalRanker, SliceRanker},
        seeding::Seeding,
        teams::Teams,
    },
};

#[cfg(feature = "test-util")]
mod builder;
pub mod deprecated;
pub mod draw;
pub mod facts;
pub mod match_format;
pub mod metrics_calculator;
pub mod phase_format;
pub mod preset;
pub mod ranker;
pub mod seeding;
pub mod teams;

/// The plugins of one phase: the format its matches are scheduled by, who plays together, how
/// many divisions it is
/// played in, the draw dealing its field to them and the seeding ordering each, the slice rankers
/// each division is ranked by, the calculators of its metrics, and the slice rankers comparing
/// parallel divisions.
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
///     Competition, Source,
///     plugin_factory::config::PluginConfig,
///     plugins::{
///         metrics_calculator::{MorePoints, PointsCalculator},
///         phase_format::RoundRobin,
///     },
/// };
///
/// let mut draft = Competition::default()
///     .phase_draft(&[Source::registration()])
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
        default = Box::new(phase_format::RoundRobin),
        level = basic
    )]
    format: Box<dyn PhaseFormat>,
    #[plugin(
        label = "teams",
        description = "whether entrants play on their own or together",
        default = Box::new(teams::Keep)
    )]
    teams: Box<dyn Teams>,
    #[plugin(
        label = "groups",
        description = "how many parts the phase is played in, each on its own",
        default = NonZeroU32::MIN
    )]
    divisions: NonZeroU32,
    #[plugin(
        label = "draw",
        description = "how the entrants are dealt to the groups",
        default = Box::new(draw::Snake),
        level = expert
    )]
    draw: Box<dyn Draw>,
    #[plugin(
        label = "seeding",
        description = "the order the format sets the entrants of a group in",
        default = Box::new(seeding::SeededInOrder),
        level = expert
    )]
    seeding: Box<dyn Seeding>,
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
    #[serde(default)]
    #[plugin(
        label = "compared across groups by",
        description = "after the place in the group: how entrants of the same place in different \
                       groups are ranked against each other",
        default = Vec::new(),
        level = expert
    )]
    across: Vec<Box<dyn SliceRanker>>,
}

impl PhasePlugins {
    /// A draft of the plugins of a phase, with nothing answered yet, whose context provides that
    /// every qualifier of its field stands for `entrants_per_qualifier` entrants
    /// ([`EntrantsPerQualifier`](facts::EntrantsPerQualifier)), or nothing if `None`, for
    /// qualifiers of different sizes. [`Competition::phase_draft`](crate::Competition::phase_draft)
    /// is the draft of a phase of a competition, from its sources.
    pub fn draft(entrants_per_qualifier: Option<NonZeroU32>) -> Draft<Self> {
        Draft::new(|declaration| Self::declare_context(entrants_per_qualifier, declaration))
    }

    /// Declares the facts the context of a phase provides, see [`Self::draft`].
    pub(crate) fn declare_context(
        entrants_per_qualifier: Option<NonZeroU32>,
        declaration: &mut Declaration,
    ) {
        if let Some(entrants) = entrants_per_qualifier {
            declaration.provide(facts::EntrantsPerQualifier(entrants));
        }
    }

    /// The names of the fields the format suggests answers for, see
    /// [`PhaseFormat::suggestions`].
    pub const SUGGESTED: [&str; 3] = [Self::RANKING, Self::METRICS, Self::ACROSS];

    /// The suggested answer for a root ranking, metrics or across field, otherwise its schema
    /// default. Suggestions come only from the configured format and are not filtered for
    /// compatibility; the comparison across groups is suggested only where the draft answers more
    /// than one group, and nothing otherwise.
    ///
    /// # Errors
    /// Returns a configuration error if the path is unknown. An incomplete or rejected format
    /// supplies no suggestions; validation reports its problems independently.
    pub fn default_value(draft: &Draft<Self>, path: &Path) -> Result<Option<Value>, ConfigError> {
        let field = Self::SUGGESTED
            .into_iter()
            .find(|field| *path == Path::ROOT.field(*field));
        if let Some(field) = field
            && let Some(format) = draft
                .answers()
                .get(Self::FORMAT)
                .and_then(PluginConfig::from_answer)
                .and_then(|config| config.build::<dyn PhaseFormat>().ok())
        {
            let suggestions = format.suggestions();
            let several = draft
                .answers()
                .get(Self::DIVISIONS)
                .and_then(Value::as_u64)
                .is_some_and(|divisions| divisions > 1);
            return Ok(Some(match field {
                Self::RANKING => suggestions.ranking.into(),
                Self::METRICS => suggestions.metrics.into(),
                _ if several => suggestions.across.into(),
                _ => Vec::<PluginConfig>::new().into(),
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
            teams: Box::new(teams::Keep),
            divisions: NonZeroU32::MIN,
            draw: Box::new(draw::Snake),
            seeding: Box::new(seeding::SeededInOrder),
            match_format,
            ranking,
            metrics: DependencyOrderedSet::new(metrics)?,
            across: Vec::new(),
        })
    }

    /// These plugins played in `divisions` divisions, dealt by `draw`, each seeded by `seeding`,
    /// ranked across them by `across`.
    pub(crate) fn with_divisions(
        mut self,
        divisions: NonZeroU32,
        (draw, seeding): (Box<dyn Draw>, Box<dyn Seeding>),
        across: Vec<Box<dyn SliceRanker>>,
    ) -> Self {
        self.divisions = divisions;
        self.draw = draw;
        self.seeding = seeding;
        self.across = across;
        self
    }

    /// These plugins forming the teams by `teams`.
    pub(crate) fn with_teams(mut self, teams: Box<dyn Teams>) -> Self {
        self.teams = teams;
        self
    }

    pub fn format(&self) -> &dyn PhaseFormat {
        &*self.format
    }

    /// Who plays together: how the field is formed into the teams the phase ranks.
    pub fn teams(&self) -> &dyn Teams {
        &*self.teams
    }

    /// How many divisions the phase is played in.
    pub fn divisions(&self) -> NonZeroU32 {
        self.divisions
    }

    /// How the field of the phase is dealt to its divisions.
    pub fn draw(&self) -> &dyn Draw {
        &*self.draw
    }

    /// How the qualifiers dealt to a division are ordered.
    pub fn seeding(&self) -> &dyn Seeding {
        &*self.seeding
    }

    /// The slice rankers ranking the rankings of one rank in their parallel divisions against each
    /// other, best first, see [`Phase::standings`](crate::Phase::standings).
    pub fn across(&self) -> &[Box<dyn SliceRanker>] {
        &self.across
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

    /// These plugins as they are configured, without their runtime state: each built from its
    /// answer in [`Self::config`].
    pub(crate) fn without_state(&self) -> Self {
        fn built<A: Axis + ?Sized>(answer: &Value) -> Box<A>
        where
            Box<A>: DeserializeOwned,
        {
            PluginConfig::from_answer(answer)
                .and_then(|plugin| plugin.build::<A>().ok())
                .expect("a plugin builds from the config it serializes as")
        }
        let config = self.config();
        let answer = |field| config.get(field).expect("every field is answered");
        let list = |field| answer(field).as_array().expect("a list is answered by one");
        Self::from_parts(
            built(answer(Self::FORMAT)),
            built(answer(Self::MATCH_FORMAT)),
            list(Self::RANKING).iter().map(built).collect(),
            list(Self::METRICS).iter().map(built).collect(),
        )
        .expect("the calculators were in order")
        .with_divisions(
            self.divisions,
            (built(answer(Self::DRAW)), built(answer(Self::SEEDING))),
            list(Self::ACROSS).iter().map(built).collect(),
        )
        .with_teams(built(answer(Self::TEAMS)))
    }

    /// The ranker of a division with these plugins: its slice rankers in order.
    pub(crate) fn ranker(&self) -> HierarchicalRanker<'_> {
        HierarchicalRanker::new(&self.ranking)
    }

    /// The ranker across the parallel divisions of a phase with these plugins: the slice rankers
    /// of [`Self::across`] in order.
    pub(crate) fn across_ranker(&self) -> HierarchicalRanker<'_> {
        HierarchicalRanker::new(&self.across)
    }

    /// The matches and byes the format schedules next in the phase of `data` played with these
    /// plugins, see [`PhaseFormat::create_new_matches`].
    pub(crate) fn schedule(&mut self, data: &DivisionData) -> Schedule {
        let view = DivisionView::new(data, HierarchicalRanker::new(&self.ranking));
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
