use std::{any::type_name, fmt::Debug, num::NonZeroU32};

use plugin_factory::{
    Plugin, PluginType,
    catalog::AnyAxis,
    compatibility::{self, Declaration, Incompatibility, provides_fact},
    config::PluginConfig,
    draft::{Draft, Issue, Problem},
    schema::{DependencyOrderedSetError, Field},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    entities::{Bye, Competition, Match, PhaseData, RankingId},
    plugins::{
        facts::{CalculatesMetric, EntrantsPerRanking, MatchAnnotation},
        match_format::{BestOf, Scored, WinnerOnly},
        metrics_calculator::{
            Calculator, GamesBalance, GamesBalanceCalculator, GamesDifference,
            HigherGamesDifference, HigherPointsDifference, OldMetrics, PointsBalance,
            PointsBalanceCalculator, PointsDifference,
            buchholz::{
                Buchholz, BuchholzCalculator, BuchholzOfBuchholz, BuchholzOfBuchholzCalculator,
                MoreSonnebornBerger, SonnebornBerger, SonnebornBergerCalculator,
            },
            points::{MorePoints, Points, PointsCalculator},
            provides_metric,
            round_of_elimination::{Eliminates, LaterElimination, RoundOfEliminationCalculator},
            round_of_first_loss::{LaterFirstLoss, RoundOfFirstLoss},
            wins_draws_losses::{FewerLosses, WinsDrawsLosses, WinsDrawsLossesCalculator},
        },
        phase_format::{
            double_elimination::{DoubleElimination, DoubleEliminationRound},
            monster_dyp::MonsterDyp,
            round_robin::RoundRobin,
            single_elimination::{KnockoutRound, SingleElimination},
        },
    },
    score::Score,
    type_map::TypeMapValue,
};

/// The scoring system of chess: a win is worth a point, a draw half of one.
const ONE_HALF: PointsCalculator = PointsCalculator::new(Score::ONE, Score::new(0.5));
/// The classic Sonneborn-Berger weights: a beaten opponent counts fully, an opponent played to
/// a draw counts half.
const FULL_HALF: SonnebornBergerCalculator =
    SonnebornBergerCalculator::new(Score::ONE, Score::new(0.5));
/// Two entrants per ranking, i.e. the rankings a draw pairing the entrants up leaves behind.
const PAIRS: EntrantsPerRanking = EntrantsPerRanking(NonZeroU32::new(2).unwrap());

/// Whether `plugins` fit together in a competition, i.e. what `AddPhase` checks.
fn check(plugins: &PhasePlugins) -> Result<(), Incompatibility> {
    check_in(
        |declaration| Competition::default().declare_phase_context(declaration),
        plugins,
    )
}

/// Like [`check`], for a phase whose context is `context` rather than a competition.
fn check_in(
    context: impl FnOnce(&mut Declaration),
    plugins: &PhasePlugins,
) -> Result<(), Incompatibility> {
    compatibility::check(context, &plugins.plugins())
}

/// The [`Debug`] forms of the metrics the calculators of `plugins` calculate, in the order
/// they run in.
fn metrics(plugins: &PhasePlugins) -> Vec<String> {
    plugins
        .metrics()
        .iter()
        .map(|calculator| format!("{:?}", calculator.default_metric_value()))
        .collect()
}

/// The [`Debug`] form of the default of each metric `M`.
macro_rules! metrics {
    ($($metric:ty),* $(,)?) => {
        vec![$(format!("{:?}", <$metric>::default())),*]
    };
}

/// The plugins of a phase of a competition played in `format`, ranked by `ranking` and
/// calculating the metrics of `metrics`, with the fields of these plugins answered as offered,
/// finished with the plugins they imply as a consumer adding them would.
fn with_implied(
    format: PluginConfig,
    ranking: Vec<PluginConfig>,
    metrics: Vec<PluginConfig>,
) -> Result<PhasePlugins, Vec<Issue>> {
    let mut draft = Competition::default()
        .phase_draft()
        .with("format", format)
        .and_then(|draft| draft.with("ranking", ranking))
        .and_then(|draft| draft.with("metrics", metrics))
        .and_then(Draft::with_defaults)
        .unwrap();
    draft.add_implied();
    draft.build()
}

/// The plugin `P` with nothing answered.
fn plugin<P: PluginType>() -> PluginConfig {
    PluginConfig::of::<P>()
}

/// What can be checked of the labelling guidelines of the module, for every plugin registered
/// and the fields of the composite.
#[test]
fn every_plugin_is_labelled_as_the_guidelines_say() {
    fn check_text(text: &str, what: &str) {
        assert!(
            !text.is_empty()
                && text.trim() == text
                && !text.ends_with(['.', '?', '!'])
                && !text.starts_with(|c: char| c.is_uppercase()),
            "{what} {text:?} is no lowercase phrase without a final period"
        );
    }

    let axes = [
        ("format", AnyAxis::of::<dyn PhaseFormat>()),
        ("criterion", AnyAxis::of::<dyn SliceRanker>()),
        ("calculator", AnyAxis::of::<dyn MetricsCalculator>()),
    ];
    let mut fields: Vec<_> = PhasePlugins::schema().fields().to_vec();
    for (axis_name, axis) in axes {
        let mut labels = Vec::new();
        for entry in axis.entries() {
            let label = entry.label();
            assert!(
                !label.is_empty() && label.trim() == label && !label.contains([',', '.']),
                "the label of {} is {label:?}",
                entry.id()
            );
            assert!(
                !labels.contains(&label),
                "two {axis_name}s are labelled {label:?}"
            );
            labels.push(label);
            if let Some(description) = entry.description() {
                check_text(description, "the description");
            }
            if axis_name == "criterion" {
                let comparative = label.split(' ').next().unwrap();
                assert!(
                    [
                        "more", "fewer", "less", "later", "earlier", "higher", "lower"
                    ]
                    .contains(&comparative),
                    "the criterion {label:?} does not say which way it sorts"
                );
            }
            fields.extend(entry.schema().fields().iter().cloned());
        }
    }
    for field in fields {
        check_text(field.label(), "the field label");
        if let Some(description) = field.description() {
            check_text(description, "the description");
        }
    }
}

#[test]
fn the_field_names_are_those_of_the_schema() {
    let names: Vec<_> = PhasePlugins::schema()
        .fields()
        .iter()
        .map(Field::name)
        .collect();

    assert_eq!(
        names,
        [
            PhasePlugins::FORMAT,
            PhasePlugins::MATCH_FORMAT,
            PhasePlugins::RANKING,
            PhasePlugins::METRICS,
        ]
    );
}

/// A criterion's label says which way it sorts, but not when to choose it, which its
/// description says.
#[test]
fn every_criterion_is_described() {
    for entry in AnyAxis::of::<dyn SliceRanker>().entries() {
        assert!(
            entry.description().is_some(),
            "{} is not described",
            entry.id()
        );
    }
}

#[test]
fn a_calculator_runs_after_the_calculators_of_the_metrics_it_reads() {
    // listed so that every calculator reading a metric comes before the one calculating it
    let plugins = PhasePluginsBuilder::new()
        .add_metric(BuchholzOfBuchholzCalculator)
        .add_metric(FULL_HALF)
        .add_metric(BuchholzCalculator)
        .add_metric(ONE_HALF)
        .add_metric(WinsDrawsLossesCalculator)
        .build()
        .unwrap();

    assert_eq!(
        metrics(&plugins),
        metrics![
            WinsDrawsLosses,
            Points,
            // the Sonneborn-Berger keeps its place before the Buchholz, as neither reads the
            // other
            SonnebornBerger,
            Buchholz,
            BuchholzOfBuchholz,
        ]
    );
}

/// A calculator is the same plugin whatever it is configured with, so a phase cannot score
/// its points two ways.
#[test]
fn a_phase_takes_a_calculator_once() {
    let built = PhasePluginsBuilder::new()
        .add_metric(ONE_HALF)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(PointsCalculator::new(Score::new(3.0), Score::ONE))
        .build();

    assert_eq!(
        built.err(),
        Some(DependencyOrderedSetError::Duplicate { position: 2 })
    );
}

/// A phase saved in an order its calculators cannot run in, e.g. edited by hand, is put into
/// running order when it is loaded, while a saved valid order stays as it is.
#[test]
fn deserialized_calculators_are_put_into_running_order() {
    let ordered = PhasePluginsBuilder::new()
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(ONE_HALF)
        .build()
        .unwrap();
    let mut saved = serde_json::to_value(&ordered).unwrap();
    saved["metrics"].as_array_mut().unwrap().reverse();

    let loaded: PhasePlugins = serde_json::from_value(saved).unwrap();

    assert_eq!(metrics(&loaded), metrics![WinsDrawsLosses, Points]);
    let reloaded: PhasePlugins =
        serde_json::from_value(serde_json::to_value(&loaded).unwrap()).unwrap();
    assert_eq!(metrics(&reloaded), metrics(&loaded));
}

/// Nobody configuring a phase has to know that the Sonneborn-Berger sums the points of the
/// opponents a ranking met, nor that the points are scored from its results: every metric a
/// configured calculator reads is calculated by the one calculator registered for it, as long
/// as it needs no configuration.
#[test]
fn the_metrics_a_configured_calculator_reads_are_added_from_the_catalog() {
    let plugins = with_implied(
        plugin::<RoundRobin>(),
        Vec::new(),
        vec![
            plugin::<SonnebornBergerCalculator>(),
            plugin::<PointsCalculator>(),
        ],
    )
    .unwrap();

    // the added calculators are put into running order as well
    assert_eq!(
        metrics(&plugins),
        metrics![WinsDrawsLosses, Points, SonnebornBerger]
    );
    assert_eq!(check(&plugins), Ok(()));
}

/// A metric is required by a criterion as well as by a calculator, so ranking by one that is
/// not configured calculates it.
#[test]
fn a_metric_the_phase_is_ranked_by_is_added_from_the_catalog() {
    let plugins = with_implied(
        plugin::<SingleElimination>(),
        vec![plugin::<LaterFirstLoss>()],
        Vec::new(),
    )
    .unwrap();

    assert_eq!(metrics(&plugins), metrics![RoundOfFirstLoss]);
    assert_eq!(check(&plugins), Ok(()));
}

/// The difference is calculated from the points scored and conceded, so ranking by it adds
/// both calculators, in running order.
#[test]
fn ranking_by_the_points_difference_adds_its_calculators() {
    let mut draft = Competition::default()
        .phase_draft()
        .with("format", plugin::<RoundRobin>())
        .and_then(|draft| draft.with("match_format", plugin::<Scored>()))
        .and_then(|draft| draft.with("ranking", vec![plugin::<HigherPointsDifference>()]))
        .and_then(|draft| draft.with("metrics", Vec::<PluginConfig>::new()))
        .and_then(Draft::with_defaults)
        .unwrap();
    draft.add_implied();
    let plugins = draft.build().unwrap();

    assert_eq!(metrics(&plugins), metrics![PointsBalance, PointsDifference]);
    assert_eq!(check(&plugins), Ok(()));
}

/// The difference is calculated from the games won and lost, so ranking by it adds both
/// calculators, in running order.
#[test]
fn ranking_by_the_games_difference_adds_its_calculators() {
    let mut draft = Competition::default()
        .phase_draft()
        .with("format", plugin::<RoundRobin>())
        .and_then(|draft| draft.with("match_format", plugin::<BestOf>()))
        .and_then(|draft| draft.with("ranking", vec![plugin::<HigherGamesDifference>()]))
        .and_then(|draft| draft.with("metrics", Vec::<PluginConfig>::new()))
        .and_then(Draft::with_defaults)
        .unwrap();
    draft.add_implied();
    let plugins = draft.build().unwrap();

    assert_eq!(metrics(&plugins), metrics![GamesBalance, GamesDifference]);
    assert_eq!(check(&plugins), Ok(()));
}

/// The points can be scored in many ways, so ranking by them without configuring how is an
/// issue rather than a guess.
#[test]
fn a_metric_whose_calculator_has_to_be_configured_is_not_added() {
    let issues = with_implied(
        plugin::<RoundRobin>(),
        vec![plugin::<MorePoints>()],
        Vec::new(),
    )
    .unwrap_err();

    assert!(
        issues.iter().any(|issue| matches!(
            issue.problem(),
            Problem::Unprovided { fact, providers, .. }
                if *fact == type_name::<Points>()
                    && providers.contains(&PointsCalculator::ID)
        )),
        "got {issues:?}"
    );
}

/// A calculator is found and checked by the fact it declares, so a registered calculator that
/// declares a metric other than the one it calculates would be added for the wrong metric and
/// never for its own.
#[test]
fn every_registered_calculator_provides_the_metric_it_calculates() {
    for entry in AnyAxis::of::<dyn MetricsCalculator>().entries() {
        // every calculator of this crate offers an answer to each of its fields
        let calculator = entry
            .default_config()
            .build::<dyn MetricsCalculator>()
            .unwrap();
        let both: [&dyn Plugin; 2] = [&*calculator, &*calculator];
        // the type of a metric behind a `dyn` is only known by its derived `Debug` form, which
        // starts with the name of the type
        let metric: String = format!("{:?}", calculator.default_metric_value())
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();

        // a calculator providing its metric twice conflicts with itself about that metric
        let Some(Incompatibility::FactProvidedTwice { fact, .. }) =
            compatibility::check(|_| {}, &both).err()
        else {
            panic!("{} does not provide a fact only once", entry.id());
        };
        assert!(
            fact.starts_with("CalculatesMetric<") && fact.ends_with(&format!("::{metric}>")),
            "{} provides {fact} but calculates {metric}",
            entry.id()
        );
    }
}

#[test]
fn a_round_robin_ranked_by_the_sonneborn_berger_fits_with_every_metric_it_reads() {
    let plugins = PhasePluginsBuilder::new()
        .add_ranking(MorePoints)
        .add_ranking(MoreSonnebornBerger)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(ONE_HALF)
        .add_metric(FULL_HALF)
        .build()
        .unwrap();

    assert_eq!(check(&plugins), Ok(()));
}

#[test]
fn a_metric_a_calculator_reads_has_to_be_calculated_as_well() {
    // the Sonneborn-Berger sums the points of the opponents a ranking met
    let plugins = PhasePluginsBuilder::new()
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(FULL_HALF)
        .build()
        .unwrap();

    assert_eq!(
        check(&plugins),
        Err(Incompatibility::FactNotProvided {
            fact: "the points",
            required_by: format!("{FULL_HALF:?}"),
        })
    );
}

#[test]
fn ranking_by_a_metric_nothing_calculates_is_an_error() {
    let plugins = PhasePluginsBuilder::new()
        .add_ranking(MorePoints)
        .add_metric(WinsDrawsLossesCalculator)
        .build()
        .unwrap();

    assert_eq!(
        check(&plugins),
        Err(Incompatibility::FactNotProvided {
            fact: type_name::<Points>(),
            required_by: format!("{MorePoints:?}"),
        })
    );
}

/// The points scored and conceded are only calculated of results with a score.
#[test]
fn points_scored_and_conceded_need_results_with_a_score() {
    let builder = PhasePluginsBuilder::new().add_metric(PointsBalanceCalculator);

    assert!(matches!(
        check(&builder.clone().match_format(WinnerOnly).build().unwrap()),
        Err(Incompatibility::FactNotProvided { .. })
    ));
    assert_eq!(
        check(&builder.match_format(Scored::default()).build().unwrap()),
        Ok(())
    );
}

/// The games won and lost are only calculated of results with games.
#[test]
fn games_won_and_lost_need_results_with_games() {
    let builder = PhasePluginsBuilder::new().add_metric(GamesBalanceCalculator);

    assert!(matches!(
        check(
            &builder
                .clone()
                .match_format(Scored::default())
                .build()
                .unwrap()
        ),
        Err(Incompatibility::FactNotProvided { .. })
    ));
    assert_eq!(
        check(&builder.match_format(BestOf::new(3)).build().unwrap()),
        Ok(())
    );
}

/// A criterion requires its metric whether it compares it in the metric's own order or in one
/// of its own.
#[test]
fn a_criterion_bringing_its_own_order_requires_its_metric_too() {
    let plugins = PhasePluginsBuilder::new()
        .add_ranking(FewerLosses)
        .build()
        .unwrap();

    assert_eq!(
        check(&plugins),
        Err(Incompatibility::FactNotProvided {
            fact: type_name::<WinsDrawsLosses>(),
            required_by: format!("{FewerLosses:?}"),
        })
    );
}

#[test]
fn the_round_of_elimination_needs_a_format_that_knocks_entrants_out() {
    let ranked_by_elimination = PhasePluginsBuilder::new()
        .add_ranking(LaterElimination)
        .add_metric(RoundOfEliminationCalculator);
    let double_elimination = ranked_by_elimination
        .clone()
        .format(DoubleElimination::new())
        .build()
        .unwrap();
    let round_robin = ranked_by_elimination.build().unwrap();

    assert_eq!(check(&double_elimination), Ok(()));
    assert_eq!(
        check(&round_robin),
        Err(Incompatibility::FactNotProvided {
            fact: "an elimination annotation on the matches",
            required_by: format!("{RoundOfEliminationCalculator:?}"),
        })
    );
}

/// The annotation naming the stage of a match is declared like every other one, so that a
/// plugin reading it — a display of the bracket, say — can require it.
#[test]
fn a_bracket_format_provides_the_annotations_it_writes() {
    let single = SingleElimination::new();
    let double = DoubleElimination::new();

    assert!(provides_fact::<MatchAnnotation<KnockoutRound>>(&single));
    assert!(provides_fact::<MatchAnnotation<DoubleEliminationRound>>(
        &double
    ));
    assert!(provides_fact::<MatchAnnotation<Eliminates>>(&double));
    assert!(!provides_fact::<MatchAnnotation<Eliminates>>(&single));
}

#[test]
fn a_monster_dyp_fits_a_competition_whose_rankings_are_single_entrants() {
    let plugins = PhasePluginsBuilder::new()
        .format(MonsterDyp::new(3))
        .build()
        .unwrap();

    assert_eq!(check(&plugins), Ok(()));
}

#[test]
fn a_monster_dyp_cannot_follow_a_draw_that_paired_the_entrants_up() {
    let format = MonsterDyp::new(3);
    let plugins = PhasePluginsBuilder::new()
        .format(format.clone())
        .build()
        .unwrap();
    let pairs = |declaration: &mut Declaration| {
        declaration.provide(PAIRS);
    };

    assert_eq!(
        check_in(pairs, &plugins),
        Err(Incompatibility::ConditionNotMet {
            condition: "rankings of single players",
            required_by: format!("{format:?}"),
            provided: format!("{PAIRS:?}"),
        })
    );
}

/// The metric of [`CircularCalculator`], which is never calculated.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Circular;

#[typetag::serde(name = "tournament_engine::plugins::tests::circular")]
impl TypeMapValue for Circular {}

/// The metric of [`Cycling`], which is never calculated.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Cycled;

#[typetag::serde(name = "tournament_engine::plugins::tests::cycled")]
impl TypeMapValue for Cycled {}

/// A calculator requiring the metric of [`Cycling`], which requires its own metric.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::plugins::tests::circular",
    label = "circular",
    declare_static = Self::declare_static
)]
struct CircularCalculator;

impl CircularCalculator {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<Cycled>>("the cycled metric");
    }
}

/// A calculator requiring the metric of [`CircularCalculator`], which requires its own one.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::plugins::tests::cycling",
    label = "cycling",
    declare_static = Self::declare_static
)]
struct Cycling;

impl Cycling {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<Circular>>("the circular metric");
    }
}

/// Calculates nothing, for the calculators of this module that only exist for what they
/// declare.
macro_rules! calculates_nothing {
    ($calculator:ty, $metric:ty) => {
        impl Calculator for $calculator {
            type Metric = $metric;

            fn after_match(
                &mut self,
                _: &PhaseData,
                _: &Match,
                _: &OldMetrics,
            ) -> Vec<(RankingId, $metric)> {
                Vec::new()
            }

            fn after_bye(
                &mut self,
                _: &PhaseData,
                _: &Bye,
                _: &OldMetrics,
            ) -> Vec<(RankingId, $metric)> {
                Vec::new()
            }
        }

        crate::register_calculator!($calculator);
    };
}

calculates_nothing!(CircularCalculator, Circular);
calculates_nothing!(Cycling, Cycled);

#[test]
fn calculators_requiring_each_other_cannot_be_ordered() {
    let built = PhasePluginsBuilder::new()
        .add_metric(CircularCalculator)
        .add_metric(Cycling)
        .build();
    let saved = serde_json::json!({
        "format": {"type": RoundRobin::ID, "value": null},
        "match_format": {"type": WinnerOnly::ID, "value": null},
        "ranking": [],
        "metrics": [
            {"type": CircularCalculator::ID, "value": null},
            {"type": Cycling::ID, "value": null},
        ],
    });

    assert_eq!(
        built.err(),
        Some(DependencyOrderedSetError::Cyclic(
            Incompatibility::CyclicRequirements {
                plugins: vec![format!("{CircularCalculator:?}"), format!("{Cycling:?}")],
            }
        ))
    );
    assert!(serde_json::from_value::<PhasePlugins>(saved).is_err());
}
