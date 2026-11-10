//! Tournament suggestions stay separate from schema defaults and compatibility validation.

use serde::{Deserialize, Serialize};
use tournament_engine::{
    Competition, PhasePlugins, PhaseView,
    plugin_factory::{
        Plugin,
        config::{Path, PluginConfig},
        register,
    },
    plugins::{
        metrics_calculator::{LaterElimination, PointsCalculator, RoundOfEliminationCalculator},
        phase_format::{PhaseFormat, PhaseSuggestions, Schedule},
    },
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// A third-party format can suggest configured plugins without any consumer knowing its type.
#[derive(Debug, Clone, Deserialize, Serialize, Plugin)]
#[plugin(id = "suggestions_test::custom", label = "custom format")]
struct Custom {
    #[plugin(label = "points for a win")]
    points: u32,
}

#[register]
impl PhaseFormat for Custom {
    fn suggestions(&self) -> PhaseSuggestions {
        PhaseSuggestions {
            // Deliberately incompatible: this format provides no elimination annotation.
            ranking: vec![PluginConfig::of::<LaterElimination>()],
            metrics: vec![
                PluginConfig::of::<RoundOfEliminationCalculator>(),
                PluginConfig::of::<PointsCalculator>().with("points_for_win", self.points),
            ],
        }
    }

    fn create_new_matches(&mut self, _: &PhaseView<'_>) -> Schedule {
        Schedule::default()
    }
}

#[test]
fn suggestions_use_the_configured_format_and_leave_conflicts_for_validation() {
    let draft = Competition::default()
        .phase_draft()
        .with("format", PluginConfig::of::<Custom>().with("points", 7))
        .unwrap();
    let mut draft = PhasePlugins::with_suggestions(draft).unwrap();

    assert_eq!(
        draft
            .answers()
            .get_at(&Path::ROOT.field("metrics").index(1).field("points_for_win")),
        Some(&7.into())
    );
    assert_eq!(
        draft.answers().get_at(
            &Path::ROOT
                .field("metrics")
                .index(1)
                .field("points_for_draw")
        ),
        Some(&0.5.into())
    );
    draft.add_implied();
    assert!(draft.build().is_err());
    assert!(!draft.issues().is_empty());
}

#[test]
fn an_incomplete_format_defers_suggestions_until_its_settings_are_answered() {
    let draft = Competition::default()
        .phase_draft()
        .with("format", PluginConfig::of::<Custom>())
        .unwrap();
    let mut draft = PhasePlugins::with_suggestions(draft).unwrap();

    assert!(draft.answers().get("ranking").is_none());
    assert!(draft.answers().get("metrics").is_none());
    assert!(draft.issues().is_empty());
    assert!(draft.build().is_err());

    draft
        .set(&Path::ROOT.field("format").field("points"), 7)
        .unwrap();
    let draft = PhasePlugins::with_suggestions(draft).unwrap();
    assert!(
        !draft
            .answers()
            .get("ranking")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn accepting_schema_defaults_does_not_apply_phase_suggestions() {
    let draft = Competition::default()
        .phase_draft()
        .with_defaults()
        .unwrap();

    assert!(draft.answers().get("ranking").is_none());
    assert!(draft.answers().get("metrics").is_none());
}

#[test]
fn filling_unanswered_settings_preserves_an_explicit_empty_ranking() {
    let draft = Competition::default()
        .phase_draft()
        .with("ranking", Vec::<PluginConfig>::new())
        .unwrap();
    let mut draft = PhasePlugins::with_suggestions(draft).unwrap();

    assert!(
        draft
            .answers()
            .get("ranking")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !draft
            .answers()
            .get("metrics")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    draft.add_implied();
    assert!(draft.build().is_ok());
}
