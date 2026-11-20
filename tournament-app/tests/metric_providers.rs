//! A metric several calculators provide: the form of a phase asks which one, keeps the choice and
//! what was entered for each. No metric of the engine has several, so the plugins are this test's
//! own, registered in its own catalog.

use plugin_factory::{Plugin, PluginType as _};
use serde::{Deserialize, Serialize};
use tournament_app::{
    form::{Editable as _, FormView, Input, MetricRow, PhaseForm},
    session::Session,
};
use tournament_engine::{
    Bye, DivisionData, Match, RankingId, Score, metric,
    plugins::{
        metrics_calculator::{Calculator, Metric, MetricId as _, OldMetrics, provides_metric},
        ranker::{Comparison, RankingCriterion, requires_metric},
    },
    register_calculator, register_criterion,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// A score of a ranking, which two calculators calculate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Rating(Score);

impl Metric for Rating {
    const LABEL: &'static str = "rating";
    const DESCRIPTION: Option<&'static str> = None;
}

metric!(Rating, id = "test::rating");

/// Calculates the [`Rating`] one way, with a field.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "test::elo",
    label = "Elo rating",
    declare_static = provides_metric::<Self>
)]
struct Elo {
    #[plugin(label = "k-factor", min = 1, max = 100, default = 20)]
    k: u32,
}

/// Calculates the [`Rating`] another way, with a field too, so that neither is implied.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "test::glicko",
    label = "Glicko rating",
    declare_static = provides_metric::<Self>
)]
struct Glicko {
    #[plugin(label = "rating deviation", min = 1, max = 500, default = 350)]
    deviation: u32,
}

macro_rules! calculates_nothing {
    ($calculator:ty) => {
        impl Calculator for $calculator {
            type Metric = Rating;

            fn after_match(
                &mut self,
                _: &DivisionData,
                _: &Match,
                _: &OldMetrics,
            ) -> Vec<(RankingId, Rating)> {
                Vec::new()
            }

            fn after_bye(
                &mut self,
                _: &DivisionData,
                _: &Bye,
                _: &OldMetrics,
            ) -> Vec<(RankingId, Rating)> {
                Vec::new()
            }
        }

        register_calculator!($calculator);
    };
}

calculates_nothing!(Elo);
calculates_nothing!(Glicko);

/// Ranks by the [`Rating`].
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "test::higher_rating",
    label = "higher rating",
    description = "by the rating",
    declare_static = requires_metric::<Self>
)]
struct HigherRating;

impl RankingCriterion for HigherRating {
    type Metric = Rating;

    fn compare(&self, first: &Rating, second: &Rating) -> Comparison {
        Comparison::greater_is_better(first.0.cmp(&second.0))
    }
}

register_criterion!(HigherRating);

fn new_form() -> PhaseForm {
    Session::new("cup").phase_form().unwrap()
}

/// The row of the rating.
#[track_caller]
fn rating(view: &FormView) -> MetricRow {
    let metrics = "metrics".parse().unwrap();
    let Some(Input::List(list)) = view
        .fields
        .iter()
        .find(|field| field.path == metrics)
        .map(|field| &field.input)
    else {
        panic!("no list of metrics");
    };
    list.metrics
        .iter()
        .find(|row| row.metric.id == Rating::ID)
        .cloned()
        .expect("the rating is not in the list of metrics")
}

/// The id of the calculator of the rating, if one is chosen.
fn provider(form: &PhaseForm) -> Option<&'static str> {
    rating(&form.view()).item.map(|item| item.plugin.id)
}

/// The k-factor of the Elo rating as the form shows it.
fn k_factor(form: &PhaseForm) -> Option<i64> {
    let item = rating(&form.view()).item?;
    item.fields.iter().find_map(|field| match field.input {
        Input::Integer { value, .. } if field.name == "k" => value,
        _ => None,
    })
}

#[test]
fn a_metric_of_several_calculators_waits_for_the_choice_of_one() {
    let mut form = new_form();

    form.add(&"ranking".parse().unwrap(), HigherRating::ID)
        .unwrap();

    let view = form.view();
    let row = rating(&view);
    assert_eq!(row.item, None);
    let providers: Vec<_> = row.providers.iter().map(|option| option.id).collect();
    assert_eq!(providers, [Elo::ID, Glicko::ID]);
    assert!(!view.complete);

    form.choose_provider(Rating::ID, Glicko::ID).unwrap();

    assert_eq!(provider(&form), Some(Glicko::ID));
    assert!(form.view().complete, "{:?}", form.view().issues);
}

/// Switching to another calculator and back brings back what was entered for the first.
#[test]
fn a_calculator_chosen_again_comes_back_as_it_was() {
    let mut form = new_form();
    form.add_metric(Rating::ID).unwrap();
    form.choose_provider(Rating::ID, Elo::ID).unwrap();
    let k = rating(&form.view()).item.unwrap().path.field("k");
    form.enter(&k, "32").unwrap();

    form.choose_provider(Rating::ID, Glicko::ID).unwrap();
    assert_eq!(provider(&form), Some(Glicko::ID));
    form.choose_provider(Rating::ID, Elo::ID).unwrap();

    assert_eq!(k_factor(&form), Some(32));
}

/// Removing a metric and adding it again brings back the calculator chosen for it.
#[test]
fn a_metric_added_again_is_calculated_as_chosen_before() {
    let mut form = new_form();
    form.add_metric(Rating::ID).unwrap();
    form.choose_provider(Rating::ID, Glicko::ID).unwrap();

    form.remove_metric(Rating::ID).unwrap();
    form.add_metric(Rating::ID).unwrap();

    assert_eq!(provider(&form), Some(Glicko::ID));
}

#[test]
fn a_calculator_of_another_metric_is_not_chosen() {
    let mut form = new_form();
    form.add_metric(Rating::ID).unwrap();

    assert!(
        form.choose_provider(Rating::ID, "tournament_engine::points")
            .is_err()
    );
    assert_eq!(provider(&form), None);
}

/// A phase calculates a metric by one calculator at most.
#[test]
fn a_phase_calculating_a_metric_twice_is_refused() {
    let mut form = new_form();
    let metrics = "metrics".parse().unwrap();
    form.add(&metrics, Elo::ID).unwrap();

    form.add(&metrics, Glicko::ID).unwrap();

    assert!(!form.view().complete);
}
