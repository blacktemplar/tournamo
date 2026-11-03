use std::cell::Cell;

use plugin_factory::{
    Plugin, axis,
    compatibility::{Declaration, Fact},
    config::{ConfigError, Path, PluginConfig},
    draft::{Draft, Problem},
    register,
    schema::{Configurable, DependencyOrderedSet},
};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

thread_local! {
    static CONSTRUCTIONS: Cell<usize> = const { Cell::new(0) };
}

#[axis]
trait Tested: Plugin {}

#[derive(Debug)]
struct Dependency;
impl Fact for Dependency {}

#[derive(Debug)]
struct Missing;
impl Fact for Missing {}

#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(id = "construction::counted", label = "counted", declare = Self::declare)]
struct Counted {
    #[serde(deserialize_with = "count")]
    #[plugin(label = "Value")]
    value: i64,
}

fn count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let value = i64::deserialize(deserializer)?;
    CONSTRUCTIONS.with(|count| count.set(count.get() + 1));
    if value < 0 {
        return Err(D::Error::custom("value must not be negative"));
    }
    Ok(value)
}

impl Counted {
    fn declare(&self, declaration: &mut Declaration) {
        match self.value {
            1 => {
                declaration.require::<Dependency>("the dependency");
            }
            2 => {
                declaration.require::<Missing>("the missing fact");
            }
            _ => {}
        }
    }
}

#[register]
impl Tested for Counted {}

#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(id = "construction::wrapper", label = "wrapper")]
struct Wrapper {
    #[plugin(label = "Child")]
    child: Box<dyn Tested>,
    #[plugin(label = "Ready")]
    ready: bool,
}

#[register]
impl Tested for Wrapper {}

#[derive(Debug, Serialize, Deserialize, Plugin)]
#[plugin(id = "construction::provider", label = "provider", declare_static = Self::declare_static)]
struct Provider;

impl Provider {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(Dependency);
    }
}

#[register]
impl Tested for Provider {}

#[derive(Debug, Deserialize, Configurable)]
struct Composite {
    #[plugin(label = "Root")]
    root: Box<dyn Tested>,
}

#[test]
fn an_incomplete_parent_still_evaluates_its_complete_child() {
    let draft = Draft::<Composite>::new(|_| {})
        .with(
            "root",
            PluginConfig::of::<Wrapper>()
                .with("child", PluginConfig::of::<Counted>().with("value", 2)),
        )
        .unwrap();
    CONSTRUCTIONS.with(|count| count.set(0));

    let issues = draft.issues();
    assert!(issues.iter().any(
        |issue| matches!(issue.problem(), Problem::Unprovided { required_by, .. }
        if *required_by == Path::ROOT.field("root").field("child"))
    ));
    CONSTRUCTIONS.with(|count| assert_eq!(count.get(), 1));
}

#[test]
fn a_rejected_child_keeps_its_original_error_path() {
    let draft = Draft::<Composite>::new(|_| {})
        .with(
            "root",
            PluginConfig::of::<Wrapper>()
                .with("child", PluginConfig::of::<Counted>().with("value", -1))
                .with("ready", true),
        )
        .unwrap();

    let issues = draft.build().unwrap_err();
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].path(),
        &Path::ROOT.field("root").field("child").field("value")
    );
    assert!(matches!(
        issues[0].problem(),
        Problem::Invalid(ConfigError::Rejected { .. })
    ));
}

#[derive(Debug, Deserialize, Configurable)]
struct Ordered {
    #[plugin(label = "Plugins")]
    plugins: DependencyOrderedSet<dyn Tested>,
    #[plugin(label = "Other")]
    other: Box<dyn Tested>,
}

#[derive(Debug, Deserialize)]
struct Opaque {
    root: Box<dyn Tested>,
}

impl Configurable for Opaque {
    fn schema() -> plugin_factory::schema::Schema {
        Composite::schema()
    }
}

#[test]
fn a_handwritten_composite_without_plugin_enumeration_is_still_checked() {
    let mut draft = Draft::<Opaque>::new(|_| {})
        .with("root", PluginConfig::of::<Counted>().with("value", 0))
        .unwrap();
    assert_eq!(draft.build().unwrap().root.id(), "construction::counted");

    draft
        .set(&Path::ROOT.field("root").field("value"), 2)
        .unwrap();
    assert!(
        draft
            .build()
            .unwrap_err()
            .iter()
            .any(|issue| matches!(issue.problem(), Problem::Unprovided { .. }))
    );
}

#[derive(Debug, Configurable)]
struct Rejected {
    #[plugin(label = "Root")]
    root: Box<dyn Tested>,
}

impl<'de> Deserialize<'de> for Rejected {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Composite::deserialize(deserializer)?;
        Err(D::Error::custom("the composite rejects these settings"))
    }
}

#[test]
fn compatible_plugins_do_not_bypass_composite_deserialization() {
    let draft = Draft::<Rejected>::new(|_| {})
        .with("root", PluginConfig::of::<Counted>().with("value", 0))
        .unwrap();

    let issues = draft.build().unwrap_err();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].path(), &Path::ROOT);
    assert!(
        matches!(issues[0].problem(), Problem::Invalid(ConfigError::Rejected { reason, .. })
        if reason == "the composite rejects these settings")
    );
}

#[test]
fn a_failed_construction_still_reports_an_independent_siblings_issue() {
    let draft = Draft::<Ordered>::new(|_| {})
        .with(
            "plugins",
            vec![PluginConfig::of::<Counted>().with("value", -1)],
        )
        .unwrap()
        .with("other", PluginConfig::of::<Counted>().with("value", 2))
        .unwrap();

    let issues = draft.build().unwrap_err();
    assert!(issues.iter().any(
        |issue| matches!(issue.problem(), Problem::Invalid(ConfigError::Rejected { path, .. })
        if *path == Path::ROOT.field("plugins").index(0).field("value"))
    ));
    assert!(issues.iter().any(
        |issue| matches!(issue.problem(), Problem::Unprovided { required_by, .. }
        if *required_by == Path::ROOT.field("other"))
    ));
}

#[test]
fn repeated_types_in_different_fields_keep_their_answer_paths() {
    let draft = Draft::<Ordered>::new(|_| {})
        .with(
            "plugins",
            vec![
                PluginConfig::of::<Counted>().with("value", 1),
                PluginConfig::of::<Provider>(),
            ],
        )
        .unwrap()
        .with("other", PluginConfig::of::<Counted>().with("value", 2))
        .unwrap();
    CONSTRUCTIONS.with(|count| count.set(0));

    let issues = draft.build().unwrap_err();
    assert!(issues.iter().any(
        |issue| matches!(issue.problem(), Problem::Unprovided { required_by, .. }
        if *required_by == Path::ROOT.field("other"))
    ));
    assert!(!issues.iter().any(
        |issue| matches!(issue.problem(), Problem::Unprovided { required_by, .. }
        if *required_by == Path::ROOT.field("plugins").index(0))
    ));
    CONSTRUCTIONS.with(|count| assert_eq!(count.get(), 2));
}
