use plugin_factory::catalog::AnyAxis;
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::CompetitionStateMachine;

/// A test over the catalog, so that it covers every preset registered.
#[test]
fn every_preset_expands_with_its_defaults_into_phases_added_one_after_the_other() {
    for entry in AnyAxis::of::<dyn Preset>().entries() {
        let choice = PresetChoice::draft_of(entry.id())
            .unwrap_or_else(|error| panic!("{} offers what it takes: {error}", entry.id()))
            .build()
            .unwrap_or_else(|issues| panic!("{} builds with its defaults: {issues:?}", entry.id()));
        let mut machine = CompetitionStateMachine::default();
        let mut next = 0;
        let phases = machine
            .current()
            .expand(
                &choice,
                || {
                    next += 1;
                    PhaseId::new(Uuid::from_u128(next))
                },
                ToString::to_string,
            )
            .unwrap_or_else(|error| panic!("{} expands: {error}", entry.id()));
        assert_eq!(
            phases.len(),
            choice.preset().phases().len(),
            "{}",
            entry.id()
        );
        for phase in phases {
            machine
                .apply_new(phase.into())
                .unwrap_or_else(|error| panic!("a phase of {} is added: {error}", entry.id()));
        }
    }
}
