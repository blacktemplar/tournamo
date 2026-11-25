//! What the tables of standings of a phase show: by rule what a table is ranked by and what that
//! is calculated from, otherwise what the operator said.

use plugin_factory::{Phrase, Plugin, PluginType, catalog::AnyAxis, compatibility::Declaration};
use serde::{Deserialize, Serialize};
use tournament_engine::{
    AddEntrant, AddPhase, Bye, Columns, CompetitionStateMachine, CompetitionStateMachineExt,
    DivisionData, Entrant, EntrantId, EntryMeta, Match, MatchResult, Phase, PhaseId, PhasePlugins,
    PhasePluginsBuilder, RankingId, ReportMatchResult, Score, Seed, SetCompetitionSeed,
    StartCompetition, Table,
    entities::columns::{Rule, by_rule},
    metric, plugin_factory,
    plugins::{
        facts::CalculatesMetric,
        metrics_calculator::{
            BuchholzCalculator, BuchholzOfBuchholzCalculator, Calculator, Metric, MetricEntry,
            MetricsCalculator, MoreBuchholzOfBuchholz, MorePoints, MorePointsPerMatch, OldMetrics,
            Points, PointsCalculator, PointsPerMatchCalculator, WinsDrawsLossesCalculator,
            provides_metric,
        },
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    register_calculator, register_criterion,
    undoable_state_machine::SessionHistory,
    uuid::Uuid,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const WDL: &str = WinsDrawsLossesCalculator::ID;
const POINTS: &str = PointsCalculator::ID;
const PER_MATCH: &str = PointsPerMatchCalculator::ID;
const BUCHHOLZ: &str = BuchholzCalculator::ID;
const OF_BUCHHOLZ: &str = BuchholzOfBuchholzCalculator::ID;

fn phase_id() -> PhaseId {
    PhaseId::new(Uuid::from_u128(1))
}

/// Plugins of two groups calculating every metric of this file, in running order: the wins, draws
/// and losses, the points, the points per match, the Buchholz score and the Buchholz of Buchholz
/// score; ranked by nothing yet.
fn calculating_all() -> PhasePluginsBuilder {
    PhasePluginsBuilder::new()
        .divisions(2)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(PointsCalculator::new(Score::ONE, Score::new(0.5)))
        .add_metric(PointsPerMatchCalculator)
        .add_metric(BuchholzCalculator)
        .add_metric(BuchholzOfBuchholzCalculator)
}

/// Groups ranked by points, compared across by points per match.
fn groups() -> PhasePlugins {
    calculating_all()
        .add_ranking(MorePoints)
        .add_across(MorePointsPerMatch)
        .build()
        .unwrap()
}

/// A competition of four entrants with `phase` added.
fn with_phase(phase: AddPhase) -> CompetitionStateMachine {
    let mut machine = CompetitionStateMachine::default();
    machine
        .apply_new(
            SetCompetitionSeed::new(Seed::new(*b"columns-test")).into(),
            EntryMeta::default(),
        )
        .unwrap();
    for entrant in 0..4 {
        let entrant = Entrant::new(EntrantId::new(Uuid::from_u128(entrant)));
        machine
            .apply_new(AddEntrant::new(entrant).into(), EntryMeta::default())
            .unwrap();
    }
    machine
        .apply_new(phase.into(), EntryMeta::default())
        .unwrap();
    machine
}

fn phase(machine: &CompetitionStateMachine) -> &Phase {
    &machine.current().phases()[&phase_id()]
}

/// The ids of the metrics `table` of the phase of `machine` shows, in order.
fn shown(machine: &CompetitionStateMachine, table: Table) -> Vec<&'static str> {
    phase(machine)
        .shown(table)
        .into_iter()
        .map(|metric| metric.id())
        .collect()
}

/// `columns` with `table` showing `calculator` or not.
fn showing(mut columns: Columns, calculator: &str, table: Table, shown: bool) -> Columns {
    columns.set(calculator, table, Some(shown));
    columns
}

#[test]
fn a_table_shows_what_it_is_ranked_by_and_what_that_is_calculated_from() {
    let machine = with_phase(AddPhase::new(phase_id(), groups()));

    // not the points per match, which only the comparison ranks by, nor the Buchholz scores,
    // which nothing ranks by
    assert_eq!(shown(&machine, Table::Division), [POINTS, WDL]);
    assert_eq!(shown(&machine, Table::Across), [PER_MATCH, WDL, POINTS]);
}

#[test]
fn a_table_shows_what_its_metric_is_calculated_from_however_far_back() {
    let plugins = calculating_all()
        .add_ranking(MoreBuchholzOfBuchholz)
        .build()
        .unwrap();
    let machine = with_phase(AddPhase::new(phase_id(), plugins));

    assert_eq!(
        phase(&machine).rule(Table::Division),
        [
            (OF_BUCHHOLZ, Rule::RankedBy(MoreBuchholzOfBuchholz::ID)),
            (WDL, Rule::CalculatedFrom(POINTS)),
            (POINTS, Rule::CalculatedFrom(BUCHHOLZ)),
            (BUCHHOLZ, Rule::CalculatedFrom(OF_BUCHHOLZ)),
        ]
    );
    assert_eq!(
        shown(&machine, Table::Division),
        [OF_BUCHHOLZ, WDL, POINTS, BUCHHOLZ]
    );
    // compared by nothing, the comparison shows nothing
    assert_eq!(shown(&machine, Table::Across), [""; 0]);
}

#[test]
fn the_rule_ranks_by_the_criteria_in_order_and_leaves_out_what_is_not_calculated() {
    let criteria = [MorePointsPerMatch::ID, MorePoints::ID, "unknown"];

    assert_eq!(
        by_rule(&criteria, &[WDL, POINTS, PER_MATCH]),
        [
            (PER_MATCH, Rule::RankedBy(MorePointsPerMatch::ID)),
            (POINTS, Rule::RankedBy(MorePoints::ID)),
            (WDL, Rule::CalculatedFrom(PER_MATCH)),
        ]
    );
    // a metric not calculated is neither ranked by nor calculated from
    assert_eq!(
        by_rule(&criteria, &[POINTS]),
        [(POINTS, Rule::RankedBy(MorePoints::ID))]
    );
}

#[test]
fn the_operator_shows_or_hides_a_metric_in_either_table() {
    let columns = [
        (BUCHHOLZ, Table::Division, true),
        (WDL, Table::Division, false),
        (OF_BUCHHOLZ, Table::Across, true),
        (POINTS, Table::Across, false),
    ]
    .into_iter()
    .fold(Columns::new(), |columns, (calculator, table, shown)| {
        showing(columns, calculator, table, shown)
    });
    let machine = with_phase(AddPhase::new(phase_id(), groups()).with_columns(columns));

    // what a criterion ranks by first, then the others in running order
    assert_eq!(shown(&machine, Table::Division), [POINTS, BUCHHOLZ]);
    assert_eq!(
        shown(&machine, Table::Across),
        [PER_MATCH, WDL, OF_BUCHHOLZ]
    );
}

#[test]
fn what_the_operator_said_about_a_metric_not_calculated_is_dropped() {
    let plugins = PhasePluginsBuilder::new()
        .add_ranking(MorePoints)
        .add_metric(WinsDrawsLossesCalculator)
        .add_metric(PointsCalculator::new(Score::ONE, Score::new(0.5)))
        .build()
        .unwrap();
    let columns = showing(
        showing(Columns::new(), BUCHHOLZ, Table::Division, true),
        WDL,
        Table::Division,
        false,
    );

    let added = AddPhase::new(phase_id(), plugins).with_columns(columns);

    assert_eq!(
        added.columns(),
        &showing(Columns::new(), WDL, Table::Division, false)
    );
}

#[test]
fn columns_keep_only_what_differs_from_the_rule() {
    let mut columns = showing(Columns::new(), POINTS, Table::Division, false);
    columns.set(POINTS, Table::Across, Some(true));
    columns.set(POINTS, Table::Division, None);

    assert_eq!(columns.get(POINTS).across, Some(true));
    columns.set(POINTS, Table::Across, None);

    assert!(columns.is_empty());
}

#[test]
fn the_columns_of_a_phase_with_results_change_without_a_conflict() {
    let mut machine = with_phase(AddPhase::new(phase_id(), groups()));
    machine
        .apply_new(StartCompetition::default().into(), EntryMeta::default())
        .unwrap();
    let division = &phase(&machine).divisions()[0];
    let first = division.data().matches().values().next().unwrap().id();
    machine
        .apply_new(
            ReportMatchResult::new(phase_id(), first, MatchResult::AWins).into(),
            EntryMeta::default(),
        )
        .unwrap();
    let mut history = SessionHistory::new();
    let columns = showing(Columns::new(), WDL, Table::Division, false);

    machine
        .reconfigure_phase(
            &mut history,
            AddPhase::new(phase_id(), groups()).with_columns(columns.clone()),
            EntryMeta::default(),
        )
        .unwrap()
        .expect("the columns changed");

    assert_eq!(phase(&machine).columns(), &columns);
    assert_eq!(shown(&machine, Table::Division), [POINTS]);
    let division = &phase(&machine).divisions()[0];
    assert!(division.data().matches()[&first].is_finished());
    // the same columns again change nothing
    assert!(matches!(
        machine.reconfigure_phase(
            &mut history,
            AddPhase::new(phase_id(), groups()).with_columns(columns),
            EntryMeta::default()
        ),
        Ok(None)
    ));
}

#[test]
fn columns_round_trip_in_json_and_postcard() {
    let mut columns = showing(Columns::new(), BUCHHOLZ, Table::Division, true);
    columns.set(WDL, Table::Across, Some(false));
    let machine = with_phase(AddPhase::new(phase_id(), groups()).with_columns(columns.clone()));

    let json: CompetitionStateMachine =
        serde_json::from_str(&serde_json::to_string(&machine).unwrap()).unwrap();
    let binary: CompetitionStateMachine =
        postcard::from_bytes(&postcard::to_stdvec(&machine).unwrap()).unwrap();
    let snapshot: tournament_engine::Competition =
        postcard::from_bytes(&postcard::to_stdvec(machine.current()).unwrap()).unwrap();

    assert_eq!(phase(&json).columns(), &columns);
    assert_eq!(phase(&binary).columns(), &columns);
    assert_eq!(snapshot.phases()[&phase_id()].columns(), &columns);
    assert_eq!(
        serde_json::to_value(&columns).unwrap(),
        serde_json::json!([
            [BUCHHOLZ, { "division": true, "across": null }],
            [WDL, { "division": null, "across": false }],
        ])
    );
}

/// Columns saved before metrics had ids of their own name each metric by the id of its
/// calculator: the calculators of then calculate a metric of their own id for good.
#[test]
fn columns_saved_by_the_ids_of_calculators_name_their_metrics() {
    let calculators = AnyAxis::of::<dyn MetricsCalculator>();
    for id in [
        "tournament_engine::buchholz",
        "tournament_engine::buchholz_of_buchholz",
        "tournament_engine::games_balance",
        "tournament_engine::games_difference",
        "tournament_engine::points",
        "tournament_engine::points_balance",
        "tournament_engine::points_difference",
        "tournament_engine::points_per_match",
        "tournament_engine::round_of_elimination",
        "tournament_engine::round_of_first_loss",
        "tournament_engine::sonneborn_berger",
        "tournament_engine::wins_draws_losses",
    ] {
        let calculator = calculators.entry(id).unwrap();
        assert_eq!(
            MetricEntry::of_calculator(calculator).map(|metric| metric.id()),
            Some(id)
        );
    }
}

/// A metric of a third party that is not meant to be shown: the points, doubled.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Doubled(Score);

impl Metric for Doubled {
    const LABEL: &'static str = "doubled points";
    const DESCRIPTION: Option<&'static str> = None;
}

metric!(Doubled, id = "test::doubled");

#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "test::more_doubled",
    label = "more doubled points",
    declare_static = requires_metric::<Self>
)]
struct MoreDoubled;

impl RankingCriterion for MoreDoubled {
    type Metric = Doubled;

    fn compare(&self, first: &Doubled, second: &Doubled) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

register_criterion!(MoreDoubled);

/// Calculates the [`Doubled`] points, offering no column.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "test::doubled",
    label = "doubled points",
    declare_static = DoubledCalculator::declare_static
)]
struct DoubledCalculator;

impl DoubledCalculator {
    fn declare_static(declaration: &mut Declaration) {
        provides_metric::<Self>(declaration);
        declaration.require::<CalculatesMetric<Points>>(Phrase::new("points", "the points"));
    }
}

impl Calculator for DoubledCalculator {
    type Metric = Doubled;

    fn after_match(
        &mut self,
        _: &DivisionData,
        _: &Match,
        _: &OldMetrics,
    ) -> Vec<(RankingId, Doubled)> {
        Vec::new()
    }

    fn after_bye(
        &mut self,
        _: &DivisionData,
        _: &Bye,
        _: &OldMetrics,
    ) -> Vec<(RankingId, Doubled)> {
        Vec::new()
    }
}

register_calculator!(DoubledCalculator);

#[test]
fn a_calculator_without_a_column_is_not_shown_but_what_it_is_calculated_from_is() {
    let plugins = calculating_all()
        .add_metric(DoubledCalculator)
        .add_ranking(MoreDoubled)
        .build()
        .unwrap();
    let mut columns = Columns::new();
    columns.set(DoubledCalculator::ID, Table::Across, Some(true));
    let machine = with_phase(AddPhase::new(phase_id(), plugins).with_columns(columns));

    assert_eq!(
        phase(&machine).rule(Table::Division)[0].0,
        DoubledCalculator::ID
    );
    assert_eq!(shown(&machine, Table::Division), [WDL, POINTS]);
    assert_eq!(shown(&machine, Table::Across), [""; 0]);
}
