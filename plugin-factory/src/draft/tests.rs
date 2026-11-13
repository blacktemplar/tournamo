//! The draft, tested on axes of its own so that it does not depend on what the plugins of a
//! program happen to register.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{Plugin, PluginType, axis, compatibility::Fact, register, schema::FieldKind};

#[derive(Debug)]
struct Points;

impl Fact for Points {}

#[derive(Debug)]
struct Matches;

impl Fact for Matches {}

#[derive(Debug)]
struct Eliminating;

impl Fact for Eliminating {}

/// How many entrants a ranking stands for, the fact the context of the composite provides.
#[derive(Debug)]
struct EntrantsPerRanking(u32);

impl Fact for EntrantsPerRanking {}

/// How the matches of a phase are scheduled.
#[axis]
trait Format: Plugin {}

/// What is calculated for an entrant.
#[axis]
trait Metric: Plugin {}

/// A format with no settings or declarations.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "round_robin",
    label = "round_robin",
    description = "a plugin of the draft's tests"
)]
struct RoundRobin;

#[register]
impl Format for RoundRobin {}

/// A format with a parameter, providing a fact no metric provides.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "knockout",
    label = "knockout",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct Knockout {
    #[plugin(label = "Third place", default)]
    third_place: bool,
}

impl Knockout {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(Eliminating);
    }
}

#[register]
impl Format for Knockout {}

/// A format providing a fact the context provides.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "doubles",
    label = "doubles",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct Doubles;

impl Doubles {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(EntrantsPerRanking(2));
    }
}

#[register]
impl Format for Doubles {}

/// A format with a condition on the fact the context provides.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "monster",
    label = "monster",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct Monster;

impl Monster {
    fn declare_static(declaration: &mut Declaration) {
        declaration.require_that::<EntrantsPerRanking>(
            "the entrants per ranking",
            "rankings of single players",
            |entrants| entrants.0 == 1,
        );
    }
}

#[register]
impl Format for Monster {}

#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "match_count",
    label = "match_count",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct MatchCount;

impl MatchCount {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(Matches);
    }
}

#[register]
impl Metric for MatchCount {}

/// A metric with a parameter it validates.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "wins",
    label = "wins",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct Wins {
    #[serde(deserialize_with = "non_negative")]
    #[plugin(label = "Points for a win", default = 1.0)]
    for_win: f64,
}

impl Wins {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(Points);
    }
}

fn non_negative<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let points = f64::deserialize(deserializer)?;
    if points < 0.0 {
        return Err(D::Error::custom("the points for a win cannot be negative"));
    }
    Ok(points)
}

#[register]
impl Metric for Wins {}

/// A metric providing what [`Wins`] provides.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "score",
    label = "score",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct Score;

impl Score {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(Points);
    }
}

#[register]
impl Metric for Score {}

/// A metric requiring what other metrics provide.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "buchholz",
    label = "buchholz",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct Buchholz;

impl Buchholz {
    fn declare_static(declaration: &mut Declaration) {
        declaration
            .require::<Points>("points")
            .require::<Matches>("matches");
    }
}

#[register]
impl Metric for Buchholz {}

/// A metric requiring what only a format provides.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "losses",
    label = "losses",
    description = "a plugin of the draft's tests",
    declare_static = Self::declare_static
)]
struct Losses;

impl Losses {
    fn declare_static(declaration: &mut Declaration) {
        declaration.require::<Eliminating>("eliminations");
    }
}

#[register]
impl Metric for Losses {}

/// A metric with a field that does not matter in an elimination.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "draws",
    label = "draws",
    description = "a plugin of the draft's tests"
)]
struct Draws {
    #[plugin(label = "Points for a draw", default = 0.5, irrelevant_with = Eliminating)]
    for_draw: f64,
}

#[register]
impl Metric for Draws {}

/// A metric made of metrics, so that plugins nest.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "combined",
    label = "combined",
    description = "a metric made of metrics"
)]
struct Combined {
    #[plugin(label = "Parts", default)]
    parts: Vec<Box<dyn Metric>>,
}

#[register]
impl Metric for Combined {}

/// The composite: one format, any number of metrics, and a metric or none.
#[derive(Debug, Serialize, Deserialize, Configurable)]
struct Phase {
    #[plugin(label = "Format")]
    format: Box<dyn Format>,
    #[plugin(label = "Metrics", default)]
    metrics: Vec<Box<dyn Metric>>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[plugin(label = "Tie break", default)]
    tie_break: Option<Box<dyn Metric>>,
}

/// The context of a phase of single players.
fn singles(declaration: &mut Declaration) {
    declaration.provide(EntrantsPerRanking(1));
}

/// A draft of a phase of single players.
fn draft() -> Draft<Phase> {
    Draft::new(singles)
}

/// The path written as `text`, e.g. `metrics[1].for_win`.
fn path(text: &str) -> Path {
    text.parse().unwrap()
}

/// The plugin `P` with nothing answered.
fn plugin<P: PluginType>() -> PluginConfig {
    PluginConfig::of::<P>()
}

/// A draft with `answers` set, each at the path written as its first element.
fn answered(answers: &[(&str, Value)]) -> Draft<Phase> {
    let mut draft = draft();
    for (at, answer) in answers {
        draft.set(&path(at), answer.clone()).unwrap();
    }
    draft
}

/// A field is irrelevant while the composite provides the fact its schema names, here the
/// eliminations a knockout provides; it stays answered all the same.
#[test]
fn a_field_is_irrelevant_while_its_fact_is_provided() {
    let mut draft = answered(&[
        ("format", json!(plugin::<RoundRobin>())),
        ("metrics", json!([plugin::<Draws>().with("for_draw", 0.5)])),
    ]);
    let for_draw = path("metrics[0].for_draw");
    assert!(draft.is_relevant(&for_draw).unwrap());
    assert!(
        draft.is_relevant(&path("metrics[0]")).unwrap(),
        "a position"
    );

    draft
        .set(
            &path("format"),
            json!(plugin::<Knockout>().with("third_place", false)),
        )
        .unwrap();

    assert!(!draft.is_relevant(&for_draw).unwrap());
    assert!(draft.is_relevant(&path("format")).unwrap());
    assert_eq!(
        draft.answers().get_at(&for_draw),
        Some(&json!(0.5)),
        "still answered"
    );
    assert!(matches!(
        draft.is_relevant(&path("metrics[1].for_draw")),
        Err(ConfigError::UnknownField { .. })
    ));
}

/// The registered choices at `at`, by id.
fn candidates(draft: &Draft<Phase>, at: &str) -> Vec<&'static str> {
    draft
        .candidates(&path(at))
        .unwrap()
        .into_iter()
        .map(PluginEntry::id)
        .collect()
}

/// The paths of the fields in the tree of the answers of `draft`, each with whether it is
/// answered.
fn paths<T: Configurable>(draft: &Draft<T>) -> Vec<(String, bool)> {
    Tree::of(&T::schema(), draft.answers())
        .questions
        .iter()
        .map(|question| (question.path.to_string(), question.answer.is_some()))
        .collect()
}

#[test]
fn the_tree_holds_the_fields_of_the_composite_and_of_the_plugins_answering_them() {
    let mut draft = draft();

    assert_eq!(
        paths(&draft),
        [
            ("format".to_owned(), false),
            ("metrics".to_owned(), false),
            ("tie_break".to_owned(), false),
        ]
    );
    draft.set(&path("format"), plugin::<Knockout>()).unwrap();
    assert_eq!(
        paths(&draft),
        [
            ("format".to_owned(), true),
            ("format.third_place".to_owned(), false),
            ("metrics".to_owned(), false),
            ("tie_break".to_owned(), false),
        ]
    );
}

#[test]
fn enumerated_fields_carry_the_answers_and_nested_plugins() {
    let draft = draft()
        .with(
            "metrics",
            vec![plugin::<Combined>().with("parts", vec![plugin::<Wins>().with("for_win", 2)])],
        )
        .unwrap()
        .with("tie_break", plugin::<Wins>())
        .unwrap();

    let fields = draft.fields();
    assert_eq!(fields[0].field.name(), "format");
    assert_eq!(fields[0].answer, None);
    assert!(fields[0].plugins().is_empty());

    let combined = &fields[1].plugins()[0];
    assert_eq!(combined.entry.id(), Combined::ID);
    let wins = &combined.fields()[0].plugins()[0];
    assert_eq!(wins.entry.id(), Wins::ID);
    let for_win = &wins.fields()[0];
    assert_eq!(for_win.path, path("metrics[0].parts[0].for_win"));
    assert_eq!(for_win.answer, Some(&json!(2)));
    assert!(for_win.plugins().is_empty());

    let tie_break = &fields[2].plugins()[0];
    assert_eq!(tie_break.entry.id(), Wins::ID);
    assert_eq!(tie_break.fields()[0].path, path("tie_break.for_win"));
    assert_eq!(tie_break.fields()[0].answer, None);
}

#[test]
fn enumeration_after_edits_uses_the_current_plugin_and_list_positions() {
    let mut draft = draft()
        .with("format", plugin::<Knockout>())
        .unwrap()
        .with("metrics", vec![plugin::<Wins>(), plugin::<Combined>()])
        .unwrap();
    let format = draft.fields()[0].path.clone();
    let first_metric = draft.fields()[1].plugins()[0].path.clone();

    draft.unset(&first_metric).unwrap();
    draft.set(&format, plugin::<RoundRobin>()).unwrap();
    draft.set(&path("tie_break"), Value::Null).unwrap();

    let after = draft.fields();
    assert_eq!(after[0].plugins()[0].entry.id(), RoundRobin::ID);
    assert!(after[0].plugins()[0].fields().is_empty());
    let metrics = after[1].plugins();
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].entry.id(), Combined::ID);
    assert_eq!(metrics[0].fields()[0].path, path("metrics[0].parts"));
    assert_eq!(after[2].answer, Some(&Value::Null));
    assert!(after[2].plugins().is_empty());
}

#[test]
fn an_answer_is_set_at_its_path_and_unset_again() {
    let mut draft = answered(&[("metrics", json!([plugin::<Wins>()]))]);

    draft.set(&path("metrics[0].for_win"), 2).unwrap();
    draft
        .set(&path("metrics[1]"), plugin::<MatchCount>())
        .unwrap();
    assert_eq!(
        draft.answers().get("metrics"),
        Some(&json!([
            {"type": "wins", "value": {"for_win": 2}},
            {"type": "match_count", "value": {}},
        ]))
    );

    draft.unset(&path("metrics[0]")).unwrap();
    assert_eq!(
        draft.answers().get("metrics"),
        Some(&json!([{"type": "match_count", "value": {}}]))
    );
    draft.unset(&path("metrics")).unwrap();
    assert_eq!(draft.answers().get("metrics"), None);
}

/// The answers of a draft are taken over by another one, as they are written back after being
/// checked.
#[test]
fn a_draft_takes_over_the_answers_of_another_one() {
    let filled = answered(&[
        (
            "format",
            json!(plugin::<Knockout>().with("third_place", true)),
        ),
        ("metrics", json!([plugin::<MatchCount>()])),
        ("tie_break", Value::Null),
    ]);

    let draft = draft().with_answers(filled.answers()).unwrap();

    assert_eq!(draft.answers(), filled.answers());
    assert!(draft.build().is_ok());
}

/// A composite serializes as the answers it is built from, so that it can be changed in a
/// draft once it is built.
#[test]
fn a_built_composite_has_the_config_it_was_built_from() {
    let draft = answered(&[
        (
            "format",
            json!(plugin::<Knockout>().with("third_place", true)),
        ),
        ("metrics", json!([plugin::<Wins>().with("for_win", 2.0)])),
        ("tie_break", json!(plugin::<MatchCount>())),
    ]);

    let built = draft.build().unwrap();

    assert_eq!(Config::of(&built).as_ref(), Ok(draft.answers()));
}

#[test]
fn answers_that_do_not_fit_the_schema_are_not_taken_over() {
    let answers = Config::new().with("metrics", json!([plugin::<Wins>().with("for_win", true)]));

    assert_eq!(
        draft().with_answers(&answers).err(),
        Some(ConfigError::WrongKind {
            path: path("metrics[0].for_win"),
            expected: "a number",
            found: "yes or no",
        })
    );
}

#[test]
fn a_path_addressing_nothing_is_an_error() {
    let mut draft = answered(&[("metrics", json!([]))]);

    for at in ["format.third_place", "metrics[1]", "rounds", "tie_break[0]"] {
        assert_eq!(
            draft.set(&path(at), 1),
            Err(ConfigError::UnknownField { path: path(at) }),
            "{at}"
        );
    }
    assert_eq!(
        draft.unset(&path("metrics[0]")),
        Err(ConfigError::UnknownField {
            path: path("metrics[0]"),
        })
    );
}

#[test]
fn an_answer_that_is_not_one_its_field_takes_is_not_set() {
    let mut draft = draft();

    assert_eq!(
        draft.set(&path("metrics"), 1),
        Err(ConfigError::WrongKind {
            path: path("metrics"),
            expected: "a list",
            found: "a whole number",
        })
    );
    assert_eq!(
        draft.set(
            &path("metrics"),
            json!([plugin::<Wins>().with("for_win", true)])
        ),
        Err(ConfigError::WrongKind {
            path: path("metrics[0].for_win"),
            expected: "a number",
            found: "yes or no",
        })
    );
    assert_eq!(
        draft.set(&path("format"), plugin::<Wins>()),
        Err(ConfigError::UnknownPlugin {
            path: path("format"),
        })
    );
    assert_eq!(draft.answers(), &Config::new());
}

#[test]
fn a_plugin_already_in_a_list_is_not_set_into_it_again() {
    let mut draft = answered(&[("metrics", json!([plugin::<Wins>(), plugin::<Buchholz>()]))]);
    let before = draft.answers().clone();
    let duplicate = |at: &str| Err(ConfigError::Duplicate { path: path(at) });
    let wins = || plugin::<Wins>().with("for_win", 2);

    // whatever it is configured with, appended or in another position
    assert_eq!(
        draft.set(&path("metrics[2]"), wins()),
        duplicate("metrics[2]")
    );
    assert_eq!(
        draft.set(&path("metrics[1]"), wins()),
        duplicate("metrics[1]")
    );
    assert_eq!(
        draft.set(&path("metrics"), json!([wins(), wins()])),
        duplicate("metrics[1]")
    );
    assert_eq!(draft.answers(), &before);

    // a plugin replacing itself is no repeat
    draft.set(&path("metrics[0]"), wins()).unwrap();
    assert_eq!(
        draft.answers().get("metrics"),
        Some(&json!([wins(), plugin::<Buchholz>()]))
    );
}

#[test]
fn choices_include_plugins_conflicting_with_the_context() {
    assert_eq!(
        candidates(&draft(), "format"),
        ["doubles", "knockout", "monster", "round_robin"]
    );
}

#[test]
fn choices_include_alternative_providers_of_an_already_provided_fact() {
    let draft = answered(&[
        ("format", plugin::<RoundRobin>().into()),
        ("metrics", json!([plugin::<Wins>()])),
        ("tie_break", Value::Null),
    ]);

    for at in ["metrics[0]", "metrics", "metrics[1]"] {
        assert!(candidates(&draft, at).contains(&"score"));
    }
}

#[test]
fn a_plugin_answering_another_position_of_a_list_is_no_candidate_there() {
    let draft = answered(&[
        ("metrics", json!([plugin::<Buchholz>(), plugin::<Wins>()])),
        (
            "tie_break",
            plugin::<Combined>().with("parts", json!([])).into(),
        ),
    ]);
    let ids = |at| candidates(&draft, at);
    let all = [
        "buchholz",
        "combined",
        "draws",
        "losses",
        "match_count",
        "score",
        "wins",
    ];

    // the wins are left out whatever they are configured with
    assert_eq!(
        ids("metrics[2]"),
        ["combined", "draws", "losses", "match_count", "score"]
    );
    // what answers the position itself stays a candidate, and so does what answers another
    // field
    assert_eq!(
        ids("metrics[0]"),
        [
            "buchholz",
            "combined",
            "draws",
            "losses",
            "match_count",
            "score"
        ]
    );
    assert_eq!(ids("metrics"), all);
    assert_eq!(ids("tie_break.parts[0]"), all);
}

#[test]
fn choices_include_plugins_with_missing_requirements() {
    let draft = answered(&[
        ("format", plugin::<RoundRobin>().into()),
        ("tie_break", Value::Null),
    ]);

    assert!(candidates(&draft, "metrics").contains(&"losses"));
}

#[test]
fn a_position_taking_no_plugins_has_no_candidates() {
    let draft = answered(&[("format", plugin::<Knockout>().into())]);

    assert!(candidates(&draft, "format.third_place").is_empty());
}

#[test]
fn nested_plugins_are_addressed_through_the_fields_they_answer() {
    let mut draft = answered(&[
        ("format", plugin::<RoundRobin>().into()),
        ("metrics", json!([plugin::<Combined>()])),
    ]);

    assert!(paths(&draft).contains(&("metrics[0].parts".to_owned(), false)));
    draft
        .set(&path("metrics[0].parts"), json!([plugin::<Wins>()]))
        .unwrap();
    assert!(candidates(&draft, "metrics[0].parts[1]").contains(&"score"));
}

#[test]
fn an_answer_a_plugin_rejects_is_an_issue_at_its_field() {
    let draft = answered(&[("metrics", json!([plugin::<Wins>().with("for_win", -1.0)]))]);
    let rejected = ConfigError::Rejected {
        path: path("metrics[0].for_win"),
        reason: "the points for a win cannot be negative".to_owned(),
    };

    assert_eq!(
        draft.issues(),
        [Issue {
            path: path("metrics[0].for_win"),
            problem: Problem::Invalid(rejected),
        }]
    );
}

#[test]
fn plugins_providing_the_same_fact_are_issues_at_each_of_them() {
    let draft = answered(&[
        ("format", plugin::<RoundRobin>().into()),
        ("metrics", json!([plugin::<Wins>(), plugin::<Score>()])),
        ("tie_break", Value::Null),
    ]);
    let twice = |first: &str, second: &str| {
        Problem::Incompatible(Incompatibility::FactProvidedTwice {
            fact: format!("{Points:?}"),
            first: first.to_owned(),
            second: second.to_owned(),
        })
    };

    assert_eq!(
        draft.issues(),
        [
            Issue {
                path: path("metrics[0]"),
                problem: twice("score (metrics[1])", "wins (metrics[0])"),
            },
            Issue {
                path: path("metrics[1]"),
                problem: twice("wins (metrics[0])", "score (metrics[1])"),
            },
        ]
    );
}

#[test]
fn a_fact_nothing_provides_is_an_issue_at_each_field_that_could_provide_it() {
    let losing = answered(&[
        ("format", plugin::<RoundRobin>().into()),
        ("metrics", json!([plugin::<Losses>()])),
        ("tie_break", Value::Null),
    ]);
    let counting = answered(&[
        ("format", plugin::<RoundRobin>().into()),
        ("metrics", json!([plugin::<Buchholz>(), plugin::<Wins>()])),
        ("tie_break", Value::Null),
    ]);
    let unprovided = |fact, providers: &[&'static str]| Problem::Unprovided {
        fact,
        required_by: path("metrics[0]"),
        providers: providers.to_vec(),
    };

    assert_eq!(
        losing.issues(),
        [Issue {
            path: path("format"),
            problem: unprovided("eliminations", &["knockout"]),
        }]
    );
    assert_eq!(
        counting.issues(),
        [
            Issue {
                path: path("metrics"),
                problem: unprovided("matches", &["match_count"]),
            },
            Issue {
                path: path("tie_break"),
                problem: unprovided("matches", &["match_count"]),
            },
        ]
    );
}

#[test]
fn a_fact_an_unanswered_field_could_provide_is_no_issue() {
    let draft = answered(&[("metrics", json!([plugin::<Losses>()]))]);

    assert_eq!(draft.issues(), []);
}

#[test]
fn a_field_offers_the_default_of_its_schema() {
    let draft = answered(&[("format", plugin::<Knockout>().into())]);

    assert_eq!(
        draft.default_value(&path("format.third_place")),
        Ok(Some(json!(false)))
    );
    assert_eq!(draft.default_value(&path("metrics")), Ok(Some(json!([]))));
    assert_eq!(draft.default_value(&path("format")), Ok(None));
}

/// Answers every unanswered field of `draft` that offers an answer with it, depth first in
/// schema order, as a UI accepting every default does.
fn accept_defaults(draft: &mut Draft<Phase>) {
    while let Some(at) = Tree::of(&Phase::schema(), draft.answers())
        .questions
        .into_iter()
        .find(|question| {
            question.answer.is_none() && draft.default_value(&question.path) != Ok(None)
        })
        .map(|question| question.path)
    {
        let default = draft.default_value(&at).unwrap().unwrap();
        draft.set(&at, default).unwrap();
    }
}

#[test]
fn building_fills_in_no_default() {
    let mut draft = answered(&[("format", plugin::<RoundRobin>().into())]);
    let unanswered = |at: &str| Issue {
        path: path(at),
        problem: Problem::Invalid(ConfigError::Unanswered { path: path(at) }),
    };

    assert_eq!(
        draft.build().err(),
        Some(vec![unanswered("metrics"), unanswered("tie_break")])
    );
    accept_defaults(&mut draft);
    assert_eq!(
        draft.build().map(|phase| format!("{phase:?}")),
        Ok(format!(
            "{:?}",
            Phase {
                format: Box::new(RoundRobin),
                metrics: vec![],
                tie_break: None,
            }
        ))
    );
}

#[test]
fn building_with_an_unanswered_field_fails() {
    let unanswered = |at: &str| Issue {
        path: path(at),
        problem: Problem::Invalid(ConfigError::Unanswered { path: path(at) }),
    };

    assert_eq!(
        draft().build().err(),
        Some(vec![
            unanswered("format"),
            unanswered("metrics"),
            unanswered("tie_break"),
        ])
    );
}

#[test]
fn a_composite_that_is_a_unit_struct_is_built() {
    /// A composite asking for nothing.
    #[derive(Debug, Deserialize, Configurable)]
    struct Nothing;

    assert!(Draft::<Nothing>::new(|_| {}).build().is_ok());
}

#[test]
fn building_with_an_answer_a_plugin_rejects_fails() {
    let draft = answered(&[
        ("format", plugin::<RoundRobin>().into()),
        ("metrics", json!([plugin::<Wins>().with("for_win", -1.0)])),
        ("tie_break", Value::Null),
    ]);

    assert_eq!(
        draft.build().err(),
        Some(vec![Issue {
            path: path("metrics[0].for_win"),
            problem: Problem::Invalid(ConfigError::Rejected {
                path: path("metrics[0].for_win"),
                reason: "the points for a win cannot be negative".to_owned(),
            }),
        }])
    );
}

#[test]
fn building_reports_with_nothing_open() {
    let draft = answered(&[
        ("metrics", json!([plugin::<Losses>()])),
        ("tie_break", Value::Null),
    ]);

    // the unanswered format could still provide the eliminations
    assert_eq!(draft.issues(), []);
    assert_eq!(
        draft.build().err(),
        Some(vec![
            Issue {
                path: path("format"),
                problem: Problem::Invalid(ConfigError::Unanswered {
                    path: path("format"),
                }),
            },
            Issue {
                path: path("format"),
                problem: Problem::Unprovided {
                    fact: "eliminations",
                    required_by: path("metrics[0]"),
                    providers: vec!["knockout"],
                },
            },
        ])
    );
}

#[test]
fn the_implied_plugins_are_added_to_the_answers_and_not_by_building() {
    let mut draft = answered(&[
        (
            "format",
            plugin::<Knockout>().with("third_place", false).into(),
        ),
        ("metrics", json!([plugin::<Buchholz>()])),
        ("tie_break", Value::Null),
    ]);
    let issues = draft.build().expect_err("building adds no plugin");
    let unprovided: Vec<_> = issues
        .iter()
        .filter_map(|issue| match issue.problem() {
            Problem::Unprovided { fact, .. } => Some((issue.path().to_string(), *fact)),
            _ => None,
        })
        .collect();
    assert_eq!(
        unprovided,
        [
            ("metrics".to_owned(), "points"),
            ("tie_break".to_owned(), "points"),
            ("metrics".to_owned(), "matches"),
            ("tie_break".to_owned(), "matches"),
        ]
    );

    draft.add_implied();

    // wins has a field, so score is the only plugin providing the points that is added
    assert_eq!(
        draft.answers().get("metrics"),
        Some(&json!([
            plugin::<Buchholz>(),
            plugin::<Score>(),
            plugin::<MatchCount>()
        ]))
    );
    assert_eq!(
        draft.build().map(|phase| format!("{:?}", phase.metrics)),
        Ok("[Buchholz, Score, MatchCount]".to_owned())
    );
}

#[test]
fn a_composite_answered_in_code_is_built() -> Result<(), ConfigError> {
    let draft = draft()
        .with("format", plugin::<Knockout>().with("third_place", true))?
        .with(
            "metrics",
            json!([
                plugin::<Buchholz>(),
                plugin::<Score>(),
                plugin::<MatchCount>()
            ]),
        )?
        .with("tie_break", Value::Null)?;

    assert_eq!(
        draft.build().map(|phase| format!("{phase:?}")),
        Ok(format!(
            "{:?}",
            Phase {
                format: Box::new(Knockout { third_place: true }),
                metrics: vec![Box::new(Buchholz), Box::new(Score), Box::new(MatchCount)],
                tie_break: None,
            }
        ))
    );
    Ok(())
}

#[test]
fn answering_a_field_in_code_is_checked_as_setting_it() {
    assert_eq!(
        draft().with("metrics", 1).err(),
        Some(ConfigError::WrongKind {
            path: path("metrics"),
            expected: "a list",
            found: "a whole number",
        })
    );
    assert_eq!(
        draft().with("rounds", 1).err(),
        Some(ConfigError::UnknownField {
            path: path("rounds")
        })
    );
}

#[test]
fn the_defaults_do_not_depend_on_other_answers() -> Result<(), ConfigError> {
    let draft = draft()
        .with("format", plugin::<RoundRobin>())?
        .with_defaults()?;

    assert_eq!(
        Value::from(draft.answers().clone()),
        json!({
            "format": {"type": "round_robin", "value": {}},
            "metrics": [],
            "tie_break": null,
        })
    );
    Ok(())
}

#[test]
fn the_defaults_keep_the_answers_and_leave_what_offers_nothing() -> Result<(), ConfigError> {
    let draft = draft()
        .with("metrics", json!([plugin::<Wins>().with("for_win", 3.0)]))?
        .with_defaults()?;

    assert_eq!(
        Value::from(draft.answers().clone()),
        json!({
            "metrics": [{"type": "wins", "value": {"for_win": 3.0}}],
            "tie_break": null,
        })
    );
    Ok(())
}

/// Implied plugins, on axes of their own so that what the answers imply is not changed by the
/// plugins of the other tests.
/// A fact nothing provides yet only counts as still to arrive while a plugin providing it
/// could still be added, tested on axes of their own: a criterion requiring rounds of
/// elimination, which only a calculator requiring a format that eliminates calculates.
mod partial_validation {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[derive(Debug)]
    struct EliminationRounds;

    impl Fact for EliminationRounds {}

    #[axis]
    trait Schedule: Plugin {}

    #[axis]
    trait Criterion: Plugin {}

    #[axis]
    trait Calculator: Plugin {}

    /// A format eliminating nobody.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(id = "everyone", label = "everyone")]
    struct Everyone;

    #[register]
    impl Schedule for Everyone {}

    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "eliminating",
        label = "eliminating",
        declare_static = Self::declare_static
    )]
    struct EliminatingSchedule;

    impl EliminatingSchedule {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Eliminating);
        }
    }

    #[register]
    impl Schedule for EliminatingSchedule {}

    /// The only calculator of the elimination rounds, which needs a format that eliminates.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "elimination_rounds",
        label = "elimination rounds",
        declare_static = Self::declare_static
    )]
    struct EliminationRoundsCalculator;

    impl EliminationRoundsCalculator {
        fn declare_static(declaration: &mut Declaration) {
            declaration
                .provide(EliminationRounds)
                .require::<Eliminating>("eliminations");
        }
    }

    #[register]
    impl Calculator for EliminationRoundsCalculator {}

    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "later_elimination",
        label = "later elimination",
        declare_static = Self::declare_static
    )]
    struct LaterElimination;

    impl LaterElimination {
        fn declare_static(declaration: &mut Declaration) {
            declaration.require::<EliminationRounds>("the elimination rounds");
        }
    }

    #[register]
    impl Criterion for LaterElimination {}

    #[derive(Debug, Serialize, Deserialize, Configurable)]
    struct Ranked {
        #[plugin(label = "format")]
        format: Box<dyn Schedule>,
        #[plugin(label = "ranked by")]
        ranking: Vec<Box<dyn Criterion>>,
        #[plugin(label = "metrics")]
        metrics: Vec<Box<dyn Calculator>>,
    }

    /// A draft with an empty ranking and the format `format`, if any.
    fn ranked(format: Option<PluginConfig>) -> Draft<Ranked> {
        let mut draft = Draft::new(|_| {});
        draft.set(&path("ranking"), json!([])).unwrap();
        if let Some(format) = format {
            draft.set(&path("format"), format).unwrap();
        }
        draft
    }

    #[test]
    fn choices_do_not_depend_on_the_feasibility_of_future_dependencies() {
        for format in [
            None,
            Some(plugin::<Everyone>()),
            Some(plugin::<EliminatingSchedule>()),
        ] {
            let draft = ranked(format);
            let ids: Vec<_> = draft
                .candidates(&path("ranking[0]"))
                .unwrap()
                .into_iter()
                .map(PluginEntry::id)
                .collect();
            assert_eq!(ids, [LaterElimination::ID]);
        }
    }

    #[test]
    fn an_unanswered_field_postpones_missing_facts_without_predicting_a_completion() {
        let unprovided = |format| {
            let mut draft = ranked(Some(format));
            draft
                .set(&path("ranking"), json!([plugin::<LaterElimination>()]))
                .unwrap();
            draft
                .issues()
                .into_iter()
                .filter(|issue| matches!(issue.problem(), Problem::Unprovided { .. }))
                .map(|issue| issue.path().clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(unprovided(plugin::<Everyone>()), []);
        assert_eq!(unprovided(plugin::<EliminatingSchedule>()), []);
    }

    #[test]
    fn an_incompatible_selection_can_be_corrected_but_cannot_be_built() {
        let mut draft = ranked(Some(plugin::<Everyone>()));
        draft
            .set(&path("ranking"), vec![plugin::<LaterElimination>()])
            .unwrap();
        draft
            .set(
                &path("metrics"),
                vec![plugin::<EliminationRoundsCalculator>()],
            )
            .unwrap();
        assert!(!draft.issues().is_empty());
        assert!(draft.build().is_err());
        draft
            .set(&path("format"), plugin::<EliminatingSchedule>())
            .unwrap();
        assert!(draft.issues().is_empty());
        assert!(draft.build().is_ok());
    }
}

mod resolution {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// A fact that implies [`Implied`].
    #[derive(Debug)]
    struct Implying;

    impl Fact for Implying {}

    #[derive(Debug)]
    struct Ambiguous;

    impl Fact for Ambiguous {}

    #[derive(Debug)]
    struct Downstream;

    impl Fact for Downstream {}

    #[derive(Debug)]
    struct Deep;

    impl Fact for Deep {}

    #[derive(Debug)]
    struct Crosswise;

    impl Fact for Crosswise {}

    #[derive(Debug)]
    struct Lone;

    impl Fact for Lone {}

    #[derive(Debug)]
    struct Nested;

    impl Fact for Nested {}

    #[derive(Debug)]
    struct Stranded;

    impl Fact for Stranded {}

    #[derive(Debug)]
    struct Clash;

    impl Fact for Clash {}

    /// A fact [`Clashing`] provides, so that it clashes with a context providing it too.
    #[derive(Debug)]
    struct Given;

    impl Fact for Given {}

    /// A fact required before [`Opening`], whose provider needs what only the plugin
    /// [`Opening`] implies provides.
    #[derive(Debug)]
    struct Late;

    impl Fact for Late {}

    #[derive(Debug)]
    struct Opening;

    impl Fact for Opening {}

    /// A fact two plugins provide, so that it implies none.
    #[derive(Debug)]
    struct Unlocked;

    impl Fact for Unlocked {}

    /// The axis of [`Requiring`], taken by a single slot only.
    #[axis]
    trait Need: Plugin {}

    /// An axis taken by lists of the composites.
    #[axis]
    trait Supply: Plugin {}

    /// Another axis taken by a list of a composite.
    #[axis]
    trait Extra: Plugin {}

    /// An axis taken by a list in a plugin only.
    #[axis]
    trait Inner: Plugin {}

    /// A plugin requiring the facts whose fields are set, so that a composite holding it
    /// requires them.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "requiring",
        label = "requiring",
        description = "a plugin requiring what it is told to",
        declare = Self::declare
    )]
    struct Requiring {
        #[serde(default)]
        #[plugin(label = "Implying")]
        implying: bool,
        #[serde(default)]
        #[plugin(label = "Ambiguous")]
        ambiguous: bool,
        #[serde(default)]
        #[plugin(label = "Downstream")]
        downstream: bool,
        #[serde(default)]
        #[plugin(label = "Deep")]
        deep: bool,
        #[serde(default)]
        #[plugin(label = "Crosswise")]
        crosswise: bool,
        #[serde(default)]
        #[plugin(label = "Lone")]
        lone: bool,
        #[serde(default)]
        #[plugin(label = "Nested")]
        nested: bool,
        #[serde(default)]
        #[plugin(label = "Stranded")]
        stranded: bool,
        #[serde(default)]
        #[plugin(label = "Clash")]
        clash: bool,
        #[serde(default)]
        #[plugin(label = "Late")]
        late: bool,
        #[serde(default)]
        #[plugin(label = "Opening")]
        opening: bool,
    }

    impl Requiring {
        fn declare(&self, declaration: &mut Declaration) {
            if self.implying {
                declaration.require::<Implying>("the implying fact");
            }
            if self.ambiguous {
                declaration.require::<Ambiguous>("the ambiguous fact");
            }
            if self.downstream {
                declaration.require::<Downstream>("the downstream fact");
            }
            if self.deep {
                declaration.require::<Deep>("the deep fact");
            }
            if self.crosswise {
                declaration.require::<Crosswise>("the crosswise fact");
            }
            if self.lone {
                declaration.require::<Lone>("the lone fact");
            }
            if self.nested {
                declaration.require::<Nested>("the nested fact");
            }
            if self.stranded {
                declaration.require::<Stranded>("the stranded fact");
            }
            if self.clash {
                declaration.require::<Clash>("the clash fact");
            }
            // required in this order, so that the late fact is tried first
            if self.late {
                declaration.require::<Late>("the late fact");
            }
            if self.opening {
                declaration.require::<Opening>("the opening fact");
            }
        }
    }

    #[register]
    impl Need for Requiring {}

    /// The only plugin providing [`Lone`], of the axis only a single slot takes.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "alone",
        label = "alone",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Alone;

    impl Alone {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Lone);
        }
    }

    #[register]
    impl Need for Alone {}

    /// The only plugin without fields that provides [`Implying`].
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "implied",
        label = "implied",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Implied;

    impl Implied {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Implying);
        }
    }

    #[register]
    impl Supply for Implied {}

    /// A plugin providing [`Implying`] that has to be configured.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "configured_provider",
        label = "configured provider",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct ConfiguredProvider {
        #[plugin(label = "Rounds")]
        rounds: u32,
    }

    impl ConfiguredProvider {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Implying);
        }
    }

    #[register]
    impl Supply for ConfiguredProvider {}

    fn provides_ambiguous(declaration: &mut Declaration) {
        declaration.provide(Ambiguous);
    }

    /// One of the two plugins providing [`Ambiguous`].
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "one_ambiguous",
        label = "one ambiguous",
        description = "a plugin of the resolution tests",
        declare_static = provides_ambiguous
    )]
    struct OneAmbiguous;

    #[register]
    impl Supply for OneAmbiguous {}

    /// The other plugin providing [`Ambiguous`].
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "other_ambiguous",
        label = "other ambiguous",
        description = "a plugin of the resolution tests",
        declare_static = provides_ambiguous
    )]
    struct OtherAmbiguous;

    #[register]
    impl Supply for OtherAmbiguous {}

    /// The only plugin providing [`Downstream`], requiring what [`Implied`] provides.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "chaining",
        label = "chaining",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Chaining;

    impl Chaining {
        fn declare_static(declaration: &mut Declaration) {
            declaration
                .provide(Downstream)
                .require::<Implying>("the implying fact");
        }
    }

    #[register]
    impl Supply for Chaining {}

    /// The only plugin providing [`Stranded`], requiring what is ambiguous.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "stranding",
        label = "stranding",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Stranding;

    impl Stranding {
        fn declare_static(declaration: &mut Declaration) {
            declaration
                .provide(Stranded)
                .require::<Ambiguous>("the ambiguous fact");
        }
    }

    #[register]
    impl Supply for Stranding {}

    /// The only plugin providing [`Clash`], providing [`Given`] as well.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "clashing",
        label = "clashing",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Clashing;

    impl Clashing {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Clash).provide(Given);
        }
    }

    #[register]
    impl Supply for Clashing {}

    /// The only plugin providing [`Late`], requiring what implies no plugin on its own.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "waiting",
        label = "waiting",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Waiting;

    impl Waiting {
        fn declare_static(declaration: &mut Declaration) {
            declaration
                .provide(Late)
                .require::<Unlocked>("the unlocked fact");
        }
    }

    #[register]
    impl Supply for Waiting {}

    /// The only plugin providing [`Opening`], and one of the two providing [`Unlocked`].
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "unlocking",
        label = "unlocking",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Unlocking;

    impl Unlocking {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Opening).provide(Unlocked);
        }
    }

    #[register]
    impl Supply for Unlocking {}

    /// The other plugin providing [`Unlocked`].
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "also_unlocking",
        label = "also unlocking",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct AlsoUnlocking;

    impl AlsoUnlocking {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Unlocked);
        }
    }

    #[register]
    impl Supply for AlsoUnlocking {}

    /// A plugin providing [`Crosswise`], which [`Deepening`] of another axis provides too.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "crossing",
        label = "crossing",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Crossing;

    impl Crossing {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Crosswise);
        }
    }

    #[register]
    impl Supply for Crossing {}

    /// A plugin holding a list of plugins, which is never added to.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "bundle",
        label = "bundle",
        description = "a plugin of the resolution tests"
    )]
    struct Bundle {
        #[serde(default)]
        #[plugin(label = "Parts")]
        parts: Vec<Box<dyn Inner>>,
    }

    #[register]
    impl Supply for Bundle {}

    /// The only plugin providing [`Deep`], and one of the two providing [`Crosswise`].
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "deepening",
        label = "deepening",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Deepening;

    impl Deepening {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Deep).provide(Crosswise);
        }
    }

    #[register]
    impl Extra for Deepening {}

    /// The only plugin providing [`Nested`], of the axis only [`Bundle`] takes.
    #[derive(Debug, Serialize, Deserialize, Plugin)]
    #[plugin(
        id = "nesting",
        label = "nesting",
        description = "a plugin of the resolution tests",
        declare_static = Self::declare_static
    )]
    struct Nesting;

    impl Nesting {
        fn declare_static(declaration: &mut Declaration) {
            declaration.provide(Nested);
        }
    }

    #[register]
    impl Inner for Nesting {}

    /// A composite taking lists of two axes.
    #[derive(Debug, Serialize, Deserialize, Configurable)]
    struct Resolved {
        #[plugin(label = "Requirements")]
        requirements: Box<dyn Need>,
        #[serde(default)]
        #[plugin(label = "Supplies")]
        supplies: Vec<Box<dyn Supply>>,
        #[serde(default)]
        #[plugin(label = "Extras")]
        extras: Vec<Box<dyn Extra>>,
    }

    /// A composite taking the same axis in two lists.
    #[derive(Debug, Serialize, Deserialize, Configurable)]
    struct Twice {
        #[plugin(label = "Requirements")]
        requirements: Box<dyn Need>,
        #[serde(default)]
        #[plugin(label = "First")]
        first: Vec<Box<dyn Supply>>,
        #[serde(default)]
        #[plugin(label = "Second")]
        second: Vec<Box<dyn Supply>>,
    }

    /// The ids of the plugins in the lists of the composite `C` with `context`, its
    /// requirements requiring the facts named in `requires`, its lists empty and `answers`
    /// set, built once the implied plugins are added, or the facts it reports unprovided, each
    /// with the path it reports it at.
    ///
    /// Checks on the way that adding the implied plugins once more adds nothing.
    fn built<C: Configurable + Serialize>(
        context: impl FnOnce(&mut Declaration),
        requires: &[&str],
        answers: &[(&str, Value)],
    ) -> Result<Vec<String>, Vec<(String, &'static str)>> {
        let requiring = Requiring::schema()
            .fields()
            .iter()
            .fold(plugin::<Requiring>(), |requiring, field| {
                requiring.with(field.name(), requires.contains(&field.name()))
            });
        let mut draft = Draft::<C>::new(context);
        draft.set(&path("requirements"), requiring).unwrap();
        for field in C::schema().fields() {
            if matches!(field.kind(), FieldKind::List { .. }) {
                draft.set(&path(field.name()), json!([])).unwrap();
            }
        }
        for (at, answer) in answers {
            draft.set(&path(at), answer.clone()).unwrap();
        }
        draft.add_implied();
        let added = draft.answers().clone();
        draft.add_implied();
        assert_eq!(
            draft.answers(),
            &added,
            "adding the implied plugins once more adds nothing"
        );
        match draft.build() {
            Ok(built) => {
                let built = serde_json::to_value(built).unwrap();
                Ok(built
                    .as_object()
                    .unwrap()
                    .values()
                    .filter_map(Value::as_array)
                    .flatten()
                    .map(|plugin| plugin["type"].as_str().unwrap().to_owned())
                    .collect())
            }
            Err(issues) => Err(issues
                .into_iter()
                .map(|issue| match issue.problem {
                    Problem::Unprovided { fact, .. } => (issue.path.to_string(), fact),
                    problem => panic!("{problem:?} is not about a fact nothing provides"),
                })
                .collect()),
        }
    }

    #[test]
    fn the_providers_of_what_an_added_plugin_requires_are_added_too() {
        // a plugin with fields does not make the implying fact ambiguous
        assert_eq!(
            built::<Resolved>(|_| {}, &["downstream"], &[]),
            Ok(vec!["chaining".to_owned(), "implied".to_owned()])
        );
    }

    /// The waiting plugin needs the unlocked fact, which two plugins provide, so the late fact
    /// implies nothing when it is tried; the unlocking plugin the opening fact implies provides
    /// the unlocked fact as well, so the late fact implies the waiting plugin once it is tried
    /// again.
    #[test]
    fn a_fact_tried_before_implies_a_plugin_by_what_a_later_addition_provides() {
        assert_eq!(
            built::<Resolved>(|_| {}, &["late"], &[]),
            Err(vec![("supplies".to_owned(), "the late fact")])
        );
        assert_eq!(
            built::<Resolved>(|_| {}, &["late", "opening"], &[]),
            Ok(vec!["unlocking".to_owned(), "waiting".to_owned()])
        );
    }

    #[test]
    fn a_provider_whose_requirement_cannot_be_provided_is_not_added() {
        // the stranding plugin would leave the ambiguous fact unprovided
        assert_eq!(
            built::<Resolved>(|_| {}, &["stranded"], &[]),
            Err(vec![("supplies".to_owned(), "the stranded fact")])
        );
    }

    #[test]
    fn a_provider_that_would_conflict_is_not_added() {
        let given = |declaration: &mut Declaration| {
            declaration.provide(Given);
        };

        assert_eq!(
            built::<Resolved>(|_| {}, &["clash"], &[]),
            Ok(vec!["clashing".to_owned()])
        );
        assert_eq!(
            built::<Resolved>(given, &["clash"], &[]),
            Err(vec![("supplies".to_owned(), "the clash fact")])
        );
    }

    #[test]
    fn a_provider_is_added_to_the_list_taking_its_axis() {
        assert_eq!(
            built::<Resolved>(|_| {}, &["deep"], &[]),
            Ok(vec!["deepening".to_owned()])
        );
    }

    #[test]
    fn a_fact_plugins_of_two_lists_provide_is_left_unprovided() {
        assert_eq!(
            built::<Resolved>(|_| {}, &["crosswise"], &[]),
            Err(vec![
                ("supplies".to_owned(), "the crosswise fact"),
                ("extras".to_owned(), "the crosswise fact"),
            ])
        );
    }

    #[test]
    fn nothing_is_added_for_a_fact_the_context_provides() {
        let context = |declaration: &mut Declaration| {
            declaration.provide(Implying);
        };

        assert_eq!(
            built::<Resolved>(context, &["implying"], &[]),
            Ok(Vec::new())
        );
    }

    #[test]
    fn an_ambiguous_fact_is_left_unprovided_and_the_others_provided() {
        assert_eq!(
            built::<Resolved>(|_| {}, &["ambiguous", "implying"], &[]),
            Err(vec![("supplies".to_owned(), "the ambiguous fact")])
        );
    }

    #[test]
    fn a_single_slot_is_never_added_to() {
        assert_eq!(
            built::<Resolved>(|_| {}, &["lone"], &[]),
            Err(vec![("requirements".to_owned(), "the lone fact")])
        );
    }

    #[test]
    fn a_list_of_a_plugin_is_never_added_to() {
        assert_eq!(
            built::<Resolved>(
                |_| {},
                &["nested"],
                &[(
                    "supplies",
                    json!([plugin::<Bundle>().with("parts", json!([]))])
                )]
            ),
            Err(vec![
                ("supplies".to_owned(), "the nested fact"),
                ("supplies[0].parts".to_owned(), "the nested fact"),
            ])
        );
    }

    #[test]
    fn an_axis_two_lists_take_is_never_added_to() {
        assert_eq!(
            built::<Twice>(|_| {}, &["implying"], &[]),
            Err(vec![
                ("first".to_owned(), "the implying fact"),
                ("second".to_owned(), "the implying fact"),
            ])
        );
    }
}
