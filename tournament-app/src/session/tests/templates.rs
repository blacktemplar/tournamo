use plugin_factory::schema::Level;
use tournament_engine::plugins::{
    seeding::SeededAtRandom,
    template::{GroupsAndKnockout, RoundRobinTemplate, Template},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::AddTemplateError,
    form::Editable,
    i18n::{Language, Translator},
    view::SourceRow,
};

fn english() -> Translator {
    Translator::new(Language::English, &[])
}

/// A session with the entrants `names`.
fn with_entrants(names: &[&str]) -> Session {
    let mut session = Session::new("cup");
    session.add_named_entrants(names).unwrap();
    session
}

#[test]
fn the_templates_of_the_engine_are_offered() {
    let templates: Vec<_> = Session::templates()
        .iter()
        .map(|template| template.id)
        .collect();
    assert!(templates.contains(&GroupsAndKnockout::ID));
    assert!(templates.contains(&RoundRobinTemplate::ID));
}

#[test]
fn the_preview_names_the_phases_in_the_language_and_where_they_take_their_entrants_from() {
    let preview = Session::template_preview(
        GroupsAndKnockout::ID,
        &Translator::new(Language::German, &[]),
    )
    .unwrap();
    let names: Vec<_> = preview
        .iter()
        .map(|phase| (phase.name.number, phase.name.name.as_deref()))
        .collect();
    assert_eq!(names, [(1, Some("Gruppenphase")), (2, Some("K.-o.-Phase"))]);
    assert_eq!(
        preview[1].sources,
        [SourceRow {
            phase: Some(preview[0].id),
            from: NonZeroU32::MIN,
            to: NonZeroU32::new(4),
        }]
    );
    assert!(
        preview[0]
            .summary
            .iter()
            .any(|field| field.name == PhasePlugins::DIVISIONS)
    );
}

#[test]
fn a_template_adds_ordinary_phases_undone_and_redone_as_one_action() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);

    let ids = session
        .add_template(GroupsAndKnockout::ID, &english())
        .unwrap();

    let phases = session.phases();
    assert_eq!(phases.iter().map(|phase| phase.id).collect::<Vec<_>>(), ids);
    assert_eq!(phases[0].name.name.as_deref(), Some("Group phase"));
    assert_eq!(phases[1].sources[0].to, NonZeroU32::new(4));
    assert!(matches!(
        session.undo(),
        Ok(Action::AddPhases(names)) if names.len() == 2
    ));
    assert!(session.phases().is_empty());
    assert!(matches!(
        session.redo(),
        Ok(Action::AddPhases(names)) if names.len() == 2
    ));
    assert_eq!(session.phases().len(), 2);
    // each is a phase of its own: the knockout is removed alone
    session.remove_phase(ids[1]).unwrap();
    assert_eq!(session.phases().len(), 1);
}

#[test]
fn a_template_sets_up_only_a_competition_without_phases() {
    let mut session = with_entrants(&["Alice", "Bob"]);
    session
        .add_template(RoundRobinTemplate::ID, &english())
        .unwrap();

    assert!(matches!(
        session.add_template(RoundRobinTemplate::ID, &english()),
        Err(AddTemplateError::HasPhases)
    ));
    assert_eq!(session.phases().len(), 1);
}

/// Every template plays through, whoever the lot draws.
#[test]
fn every_template_plays_through() {
    for template in Session::templates() {
        let mut session = with_entrants(&["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"]);
        let phases = session.add_template(template.id, &english()).unwrap();
        session.start().unwrap();
        for phase in phases {
            if !session.competition().phases()[&phase].is_started() {
                session.start_phase(phase).unwrap();
            }
            while let Some(row) = session.playable(MatchesOf::phase(phase)).first().cloned() {
                session.report(phase, row.id, MatchResult::AWins).unwrap();
            }
        }
        assert!(session.ranking().is_final, "{}", template.id);
    }
}

#[test]
fn an_unknown_template_is_neither_previewed_nor_added() {
    let mut session = with_entrants(&["Alice", "Bob"]);
    assert!(matches!(
        Session::template_preview("nothing", &english()),
        Err(AddTemplateError::UnknownTemplate(id)) if id == "nothing"
    ));
    assert!(matches!(
        session.add_template("nothing", &english()),
        Err(AddTemplateError::UnknownTemplate(_))
    ));
    assert!(<dyn Template>::of("nothing").is_none());
}

/// The phases of a template remember it: the competition is based on it, and what the template
/// sets beyond the basic fields, e.g. the seeding of the knockout, is no difference.
#[test]
fn a_competition_is_based_on_the_template_its_phases_came_from() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    assert_eq!(session.based_on(&english()), None);
    let ids = session
        .add_template(GroupsAndKnockout::ID, &english())
        .unwrap();

    let based_on = session.based_on(&english()).unwrap();
    assert_eq!(based_on.template.id, GroupsAndKnockout::ID);
    assert!(based_on.added.is_empty() && based_on.removed.is_empty());
    for id in ids {
        let form = session.phase_form_to_edit(id).unwrap();
        assert_eq!(form.view_at(Level::Basic).hidden.count, 0);
        assert_eq!(form.view_at(Level::Advanced).hidden.count, 0);
    }
}

/// A hidden setting changed in a phase of a template differs from the template, also after the
/// phase was edited otherwise.
#[test]
fn a_setting_changed_in_a_phase_of_a_template_differs_from_it() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    let ids = session
        .add_template(GroupsAndKnockout::ID, &english())
        .unwrap();
    let mut form = session.phase_form_to_edit(ids[1]).unwrap();
    form.choose(&"seeding".parse().unwrap(), Some(SeededAtRandom::ID))
        .unwrap();
    session.edit_phase(ids[1], &form).unwrap();

    let form = session.phase_form_to_edit(ids[1]).unwrap();
    assert_eq!(form.view_at(Level::Advanced).hidden.count, 1);
    assert_eq!(form.view_at(Level::Basic).hidden.shown_at, Level::Expert);
}

/// A phase added besides those of the template, and one of the template removed, are named.
#[test]
fn the_phases_added_to_a_template_and_removed_from_it_are_named() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    let ids = session
        .add_template(GroupsAndKnockout::ID, &english())
        .unwrap();
    session.remove_phase(ids[1]).unwrap();
    let draft = session.phase_draft(RoundRobin::ID).unwrap();
    let mut form = session.phase_form_with(draft, None).unwrap();
    form.set_name("Finals");
    session.add_phase(&form).unwrap();

    let based_on = session.based_on(&english()).unwrap();
    let added: Vec<_> = based_on
        .added
        .iter()
        .map(|name| name.name.as_deref())
        .collect();
    assert_eq!(added, [Some("Finals")]);
    assert_eq!(
        based_on.removed,
        [RemovedPhase {
            number: 2,
            name: Some("Knockout".to_owned()),
        }]
    );
}

/// A competition opens at the basic settings while it has no phase or was set up by a template,
/// else at the advanced ones its phases were set up at.
#[test]
fn a_competition_opens_at_the_level_it_was_set_up_at() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    assert_eq!(session.opening_level(), Level::Basic);
    session
        .add_template(RoundRobinTemplate::ID, &english())
        .unwrap();
    assert_eq!(session.opening_level(), Level::Basic);

    assert_eq!(Given::new().added().opening_level(), Level::Advanced);
}
