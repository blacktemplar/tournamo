use tournament_engine::plugins::template::{GroupsAndKnockout, RoundRobinTemplate, Template};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::AddTemplateError,
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
