use plugin_factory::catalog::AnyAxis;
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{CompetitionStateMachine, EntryMeta};

/// A test over the catalog, so that it covers every template registered.
#[test]
fn every_template_expands_into_phases_added_one_after_the_other() {
    for entry in AnyAxis::of::<dyn Template>().entries() {
        let template = <dyn Template>::of(entry.id())
            .unwrap_or_else(|| panic!("{} is built without fields", entry.id()));
        let mut machine = CompetitionStateMachine::default();
        let mut next = 0;
        let phases = machine
            .current()
            .expand(
                entry.id(),
                &template.phases(),
                || {
                    next += 1;
                    PhaseId::new(Uuid::from_u128(next))
                },
                ToString::to_string,
            )
            .unwrap_or_else(|error| panic!("{} expands: {error}", entry.id()));
        assert_eq!(phases.len(), template.phases().len(), "{}", entry.id());
        for phase in phases {
            machine
                .apply_new(phase.into(), EntryMeta::default())
                .unwrap_or_else(|error| panic!("a phase of {} is added: {error}", entry.id()));
        }
    }
}

#[test]
fn a_template_is_found_by_its_id() {
    assert!(<dyn Template>::of(<GroupsAndKnockout as plugin_factory::PluginType>::ID).is_some());
    assert!(<dyn Template>::of("nothing").is_none());
}
