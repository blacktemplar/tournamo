//! Schemas and the configs answering them, tested on an axis of their own so that they do not
//! depend on what the plugins of a program happen to register.

use serde::{Deserialize, Deserializer, Serialize, de::Error};
use serde_json::json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{Plugin, PluginType, axis, catalog::PluginEntry, register};

/// A plugin axis whose plugins are nothing but their configuration, reported as a summary so
/// that a built plugin can be checked without being downcast.
#[axis]
trait Configured: Plugin {
    fn summary(&self) -> String;
}

/// A plugin with a field of every scalar kind and one that may be answered with nothing, all
/// with a default.
#[derive(Debug, PartialEq, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "points",
    label = "points",
    description = "a plugin with a parameter of every scalar kind"
)]
struct Points {
    #[serde(deserialize_with = "non_negative")]
    #[plugin(
        label = "Points for a win",
        description = "what winning a match is worth",
        default = 1.0
    )]
    for_win: f64,
    #[plugin(label = "Rounds", default = 3)]
    rounds: i64,
    #[plugin(label = "Count byes", default = true)]
    counts_byes: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    #[plugin(label = "Points for a bye", default = Some(0.5))]
    for_bye: Option<f64>,
}

fn non_negative<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let points = f64::deserialize(deserializer)?;
    if points < 0.0 {
        return Err(D::Error::custom("the points for a win cannot be negative"));
    }
    Ok(points)
}

#[register]
impl Configured for Points {
    fn summary(&self) -> String {
        let Self {
            for_win,
            rounds,
            counts_byes,
            for_bye,
        } = self;
        let for_bye = for_bye.map_or_else(|| "none".to_owned(), |for_bye| for_bye.to_string());
        format!("points({for_win}, {rounds}, {counts_byes}, {for_bye})")
    }
}

/// A plugin asking for nothing.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "matches",
    label = "matches",
    description = "a plugin without parameters"
)]
struct Matches;

#[register]
impl Configured for Matches {
    fn summary(&self) -> String {
        "matches".to_owned()
    }
}

/// A plugin with a field without a default, so that it has to be configured.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "threshold",
    label = "threshold",
    description = "a plugin that has to be configured"
)]
struct Threshold {
    #[plugin(label = "Matches at least")]
    least: i64,
}

#[register]
impl Configured for Threshold {
    fn summary(&self) -> String {
        format!("threshold({})", self.least)
    }
}

/// A plugin made of other plugins, i.e. one with a field taking one plugin, one taking a list
/// of them and one taking a plugin or nothing.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "ranker",
    label = "ranker",
    description = "a plugin made of other plugins"
)]
struct Ranker {
    #[plugin(label = "Rank by", default = Box::new(Matches))]
    by: Box<dyn Configured>,
    #[plugin(label = "Tie breakers", default)]
    tie_breakers: Vec<Box<dyn Configured>>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[plugin(label = "Fall back to")]
    fallback: Option<Box<dyn Configured>>,
}

/// A plugin with a field taking an unordered list of plugins.
#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(id = "pool", label = "pool", description = "a plugin taking a set")]
struct Pool {
    #[plugin(label = "Members")]
    members: DependencyOrderedSet<dyn Configured>,
}

#[register]
impl Configured for Pool {
    fn summary(&self) -> String {
        let members: Vec<_> = self.members.iter().map(|member| member.summary()).collect();
        format!("pool([{}])", members.join(", "))
    }
}

/// The config of [`Pool`] answering its field with distinct plugins.
fn pool() -> Config {
    Config::new().with(
        "members",
        vec![
            PluginConfig::of::<Matches>(),
            PluginConfig::configured(Points::ID, points()),
        ],
    )
}

#[register]
impl Configured for Ranker {
    fn summary(&self) -> String {
        let tie_breakers: Vec<_> = self
            .tie_breakers
            .iter()
            .map(|tie_breaker| tie_breaker.summary())
            .collect();
        format!(
            "ranker({}, [{}], {})",
            self.by.summary(),
            tie_breakers.join(", "),
            self.fallback
                .as_ref()
                .map_or_else(|| "none".to_owned(), |fallback| fallback.summary())
        )
    }
}

/// The entry of the plugin `P`.
fn entry_of<P: PluginType>() -> &'static PluginEntry {
    AnyAxis::of::<dyn Configured>()
        .entry(P::ID)
        .expect("the plugin is registered")
}

/// The summary of the plugin `P` as `config` configures it.
fn built<P: PluginType>(config: Config) -> Result<String, ConfigError> {
    PluginConfig::configured(P::ID, config)
        .build::<dyn Configured>()
        .map(|plugin| plugin.summary())
}

/// The config of [`Points`] answering every field with its default.
fn points() -> Config {
    Config::new()
        .with("for_win", 1.0)
        .with("rounds", 3)
        .with("counts_byes", true)
        .with("for_bye", 0.5)
}

/// The config of [`Ranker`] answering every field: by matches, without tie breakers and
/// without a fallback.
fn ranker() -> Config {
    Config::new()
        .with("by", PluginConfig::of::<Matches>())
        .with("tie_breakers", json!([]))
        .with("fallback", Value::Null)
}

#[test]
fn a_field_taking_plugins_knows_the_axis_it_takes() {
    let schema = Ranker::schema();
    let kind = FieldKind::plugin::<dyn Configured>();

    assert_eq!(schema.field_named("by").map(Field::kind), Some(&kind));
    assert_eq!(
        schema.field_named("tie_breakers").map(Field::kind),
        Some(&FieldKind::list::<dyn Configured>())
    );
    assert_eq!(
        schema.field_named("fallback").map(Field::kind),
        Some(&FieldKind::optional(kind))
    );
    assert!(schema.field_named("losses").is_none());
    assert_eq!(
        schema.field_named("fallback").unwrap().kind().axis(),
        Some(AnyAxis::of::<dyn Configured>())
    );
    assert_eq!(
        entry_of::<Ranker>().sub_axes(),
        [AnyAxis::of::<dyn Configured>(); 3]
    );
    assert!(entry_of::<Points>().sub_axes().is_empty());
}

#[test]
fn a_field_typed_as_a_set_takes_an_unordered_list() {
    let schema = Pool::schema();

    assert_eq!(
        schema.field_named("members").map(Field::kind),
        Some(&FieldKind::set::<dyn Configured>())
    );
    assert_ne!(
        FieldKind::set::<dyn Configured>(),
        FieldKind::list::<dyn Configured>()
    );
    assert_eq!(
        entry_of::<Pool>().sub_axes(),
        [AnyAxis::of::<dyn Configured>()]
    );
}

#[test]
fn a_set_of_distinct_answers_is_built() {
    assert_eq!(
        built::<Pool>(pool()).as_deref(),
        Ok("pool([matches, points(1, 3, true, 0.5)])")
    );
}

#[test]
fn an_answer_repeated_in_a_set_is_an_error_at_its_second_position() {
    let duplicate = |config| match built::<Pool>(config) {
        Err(ConfigError::Duplicate { path }) => Some(path.to_string()),
        _ => None,
    };

    // a plugin is the same plugin whatever it is configured with
    assert_eq!(
        duplicate(pool().with(
            "members",
            vec![
                PluginConfig::configured(Points::ID, points()),
                PluginConfig::configured(Points::ID, points().with("for_win", 2)),
            ],
        ))
        .as_deref(),
        Some("members[1]")
    );
}

#[test]
fn a_plugin_repeated_in_a_list_is_an_error_even_if_the_config_is_incomplete() {
    let config = pool().with(
        "members",
        vec![
            PluginConfig::of::<Threshold>(),
            PluginConfig::of::<Threshold>(),
        ],
    );

    assert_eq!(
        Pool::schema().walk(&config, &Path::ROOT, false),
        Err(ConfigError::Duplicate {
            path: Path::of("members").index(1),
        })
    );
}

#[test]
fn a_plugin_repeated_in_an_ordered_list_is_an_error_too() {
    let config = ranker().with(
        "tie_breakers",
        vec![
            PluginConfig::of::<Matches>(),
            PluginConfig::configured(Points::ID, points()),
            PluginConfig::of::<Matches>(),
        ],
    );

    assert_eq!(
        built::<Ranker>(config),
        Err(ConfigError::Duplicate {
            path: Path::of("tie_breakers").index(2),
        })
    );
}

#[test]
fn a_field_offering_a_default_still_has_to_be_answered() {
    assert_eq!(
        Points::schema()
            .field_named("rounds")
            .and_then(Field::default_value),
        Some(&json!(3))
    );
    assert_eq!(
        built::<Points>(Config::new()),
        Err(ConfigError::Unanswered {
            path: Path::of("for_win"),
        })
    );
    assert_eq!(
        built::<Points>(points()).as_deref(),
        Ok("points(1, 3, true, 0.5)")
    );
}

#[test]
fn an_answer_is_kept_as_it_is_given() {
    let config = points()
        .with("for_win", 2.5)
        .with("counts_byes", false)
        .with("for_bye", Value::Null);

    assert_eq!(
        built::<Points>(config).as_deref(),
        Ok("points(2.5, 3, false, none)")
    );
}

#[test]
fn a_whole_number_answers_a_decimal_field() {
    assert_eq!(
        built::<Points>(points().with("for_win", 2)).as_deref(),
        Ok("points(2, 3, true, 0.5)")
    );
}

#[test]
fn a_field_without_a_default_has_to_be_answered() {
    assert_eq!(
        built::<Threshold>(Config::new()),
        Err(ConfigError::Unanswered {
            path: Path::of("least"),
        })
    );
    assert_eq!(
        built::<Threshold>(Config::new().with("least", 2)).as_deref(),
        Ok("threshold(2)")
    );
}

#[test]
fn an_answer_to_a_field_the_schema_does_not_have_is_an_error() {
    assert_eq!(
        built::<Points>(Config::new().with("for_loss", 0.0)),
        Err(ConfigError::UnknownField {
            path: Path::of("for_loss"),
        })
    );
}

#[test]
fn an_answer_of_another_kind_than_its_field_is_an_error() {
    assert_eq!(
        built::<Points>(points().with("counts_byes", 1)),
        Err(ConfigError::WrongKind {
            path: Path::of("counts_byes"),
            expected: "yes or no",
            found: "a whole number",
        })
    );
    assert_eq!(
        built::<Points>(points().with("rounds", 2.5)),
        Err(ConfigError::WrongKind {
            path: Path::of("rounds"),
            expected: "a whole number",
            found: "a number",
        })
    );
}

#[test]
fn an_answer_of_another_kind_in_a_list_is_reported_by_its_position() {
    let config = ranker().with("tie_breakers", json!([PluginConfig::of::<Matches>(), true]));

    assert_eq!(
        built::<Ranker>(config),
        Err(ConfigError::WrongKind {
            path: Path::of("tie_breakers").index(1),
            expected: "one plugin",
            found: "yes or no",
        })
    );
}

#[test]
fn an_answer_a_plugin_rejects_is_reported_as_its_field() {
    assert_eq!(
        built::<Points>(points().with("for_win", -1.0)),
        Err(ConfigError::Rejected {
            path: Path::of("for_win"),
            reason: "the points for a win cannot be negative".to_owned(),
        })
    );
}

#[test]
fn a_field_taking_plugins_is_built_through_the_catalog() {
    let config = Config::new()
        .with(
            "by",
            PluginConfig::configured(Points::ID, points().with("for_win", 2.0)),
        )
        .with(
            "tie_breakers",
            vec![
                PluginConfig::of::<Matches>(),
                PluginConfig::of::<Threshold>().with("least", 4),
            ],
        )
        .with("fallback", PluginConfig::of::<Matches>());

    assert_eq!(
        built::<Ranker>(config).as_deref(),
        Ok("ranker(points(2, 3, true, 0.5), [matches, threshold(4)], matches)")
    );
}

#[test]
fn a_plugin_field_offering_a_default_still_has_to_be_answered() {
    assert_eq!(
        built::<Ranker>(Config::new()),
        Err(ConfigError::Unanswered {
            path: Path::of("by"),
        })
    );
}

#[test]
fn a_plugin_field_that_may_be_answered_with_nothing_takes_no_plugin() {
    assert_eq!(
        built::<Ranker>(ranker()).as_deref(),
        Ok("ranker(matches, [], none)")
    );
}

#[test]
fn a_plugin_answering_a_field_that_takes_none_is_an_error() {
    let config = Config::new().with("by", 1);

    assert_eq!(
        built::<Ranker>(config),
        Err(ConfigError::WrongKind {
            path: Path::of("by"),
            expected: "one plugin",
            found: "a whole number",
        })
    );
}

#[test]
fn an_error_in_a_nested_plugin_is_reported_by_the_path_of_its_field() {
    let config = ranker().with(
        "by",
        PluginConfig::configured(Points::ID, points().with("for_win", -1.0)),
    );

    assert_eq!(
        built::<Ranker>(config),
        Err(ConfigError::Rejected {
            path: Path::of("by").field("for_win"),
            reason: "the points for a win cannot be negative".to_owned(),
        })
    );
}

#[test]
fn an_error_in_one_of_several_nested_plugins_is_reported_by_its_position() {
    let config = ranker().with(
        "tie_breakers",
        vec![
            PluginConfig::of::<Matches>(),
            PluginConfig::of::<Threshold>(),
        ],
    );

    assert_eq!(
        built::<Ranker>(config),
        Err(ConfigError::Unanswered {
            path: Path::of("tie_breakers").index(1).field("least"),
        })
    );
}

#[test]
fn an_error_in_a_nested_plugin_answering_an_optional_field_keeps_its_field() {
    let config = ranker().with(
        "fallback",
        PluginConfig::configured(Points::ID, points().with("for_win", -1.0)),
    );

    assert_eq!(
        built::<Ranker>(config),
        Err(ConfigError::Rejected {
            path: Path::of("fallback").field("for_win"),
            reason: "the points for a win cannot be negative".to_owned(),
        })
    );
}

#[test]
fn a_plugin_not_registered_for_the_axis_cannot_answer_a_plugin_field() {
    let config = Config::new().with("by", PluginConfig::new("unregistered"));

    assert_eq!(
        built::<Ranker>(config),
        Err(ConfigError::UnknownPlugin {
            path: Path::of("by"),
        })
    );
}

#[test]
fn a_built_plugin_serializes_as_the_walked_config_it_is_built_from() {
    let config = ranker()
        .with(
            "by",
            PluginConfig::configured(Points::ID, points().with("for_win", 2.0)),
        )
        .with("fallback", PluginConfig::of::<Matches>());
    let walked = Ranker::schema().walk(&config, &Path::ROOT, true).unwrap();
    let built = PluginConfig::configured(Ranker::ID, config)
        .build::<dyn Configured>()
        .unwrap();

    assert_eq!(
        walked.get("fallback"),
        Some(&json!({"type": "matches", "value": null}))
    );
    assert_eq!(
        serde_json::to_value(&built).ok(),
        Some(PluginConfig::configured(Ranker::ID, walked).into())
    );
}

#[test]
fn a_plugin_without_parameters_is_walked_as_a_unit_struct_is_written() {
    let walked = |entry: &PluginEntry, config, complete| entry.walk(&config, &Path::ROOT, complete);

    assert_eq!(
        walked(entry_of::<Matches>(), Config::new(), true),
        Ok(Value::Null)
    );
    // an answer still being edited keeps its config an object
    assert_eq!(
        walked(entry_of::<Matches>(), Config::new(), false),
        Ok(json!({}))
    );
    assert_eq!(
        walked(
            entry_of::<Threshold>(),
            Config::new().with("least", 2),
            true
        ),
        Ok(json!({"least": 2}))
    );
}

#[test]
fn an_unanswered_field_is_no_error_unless_the_config_is_to_be_complete() {
    let config = ranker().with("tie_breakers", vec![PluginConfig::of::<Threshold>()]);

    assert_eq!(
        Ranker::schema()
            .walk(&config, &Path::ROOT, false)
            .map(Value::from),
        Ok(json!({
            "by": {"type": "matches", "value": {}},
            "tie_breakers": [{"type": "threshold", "value": {}}],
            "fallback": null
        }))
    );
    assert_eq!(
        Ranker::schema().walk(&config, &Path::ROOT, true),
        Err(ConfigError::Unanswered {
            path: Path::of("tie_breakers").index(0).field("least"),
        })
    );
}
