use plugin_factory::config::Path as ConfigPath;
use tournament_engine::plugins::preset::{Dyp, GroupsThenKnockout, PresetChoice};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::AddPresetError,
    form::{Editable as _, Input, PresetForm},
    i18n::{Language, Translator},
    view::SourceRow,
};

fn english() -> Translator {
    Translator::new(Language::English, &[])
}

/// The path of the field `name` of the preset of a form.
fn field(name: &str) -> ConfigPath {
    ConfigPath::ROOT.field(PresetChoice::PRESET).field(name)
}

/// A form of groups then a knockout in `groups` groups.
fn groups(groups: u32) -> PresetForm {
    let mut form = Session::preset_form(GroupsThenKnockout::ID).unwrap();
    form.enter(&field("groups"), &groups.to_string()).unwrap();
    form
}

/// A session with the entrants `names`.
fn with_entrants(names: &[&str]) -> Session {
    let mut session = Session::new("cup");
    session.add_named_entrants(names).unwrap();
    session
}

#[test]
fn the_presets_of_the_engine_are_offered() {
    let presets: Vec<_> = Session::presets().iter().map(|preset| preset.id).collect();
    assert!(presets.contains(&GroupsThenKnockout::ID));
    assert!(presets.contains(&Dyp::ID));
}

#[test]
fn a_preset_form_shows_the_fields_of_the_preset() {
    let view = groups(3).view();
    let names: Vec<_> = view.fields.iter().map(|field| field.name).collect();
    assert_eq!(names, ["groups", "advancing", PresetChoice::MATCH_FORMAT]);
    assert!(matches!(
        view.fields[0].input,
        Input::Integer { value: Some(3), .. }
    ));
    assert!(view.complete);
}

#[test]
fn the_preview_names_the_phases_in_the_language_and_where_they_take_their_entrants_from() {
    let preview = groups(3)
        .preview(&Translator::new(Language::German, &[]))
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
            to: NonZeroU32::new(6),
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
fn a_number_not_taken_keeps_the_preset_from_being_previewed_and_added() {
    let mut session = with_entrants(&["Alice", "Bob"]);
    let mut form = groups(2);
    form.enter(&field("groups"), "0").unwrap();

    assert!(!form.view().complete);
    assert!(form.preview(&english()).is_err());
    assert!(matches!(
        session.add_preset(&form, &english()),
        Err(AddPresetError::Incomplete)
    ));
    assert!(session.competition().phases().is_empty());
}

#[test]
fn a_preset_adds_ordinary_phases_undone_and_redone_as_one_action() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);

    let ids = session.add_preset(&groups(2), &english()).unwrap();

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
fn the_phases_of_a_preset_are_played_in_the_match_format_chosen_in_its_form() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    let mut form = groups(2);
    let match_format = ConfigPath::ROOT.field(PresetChoice::MATCH_FORMAT);

    form.choose(&match_format, Some(BestOf::ID)).unwrap();
    form.enter(&match_format.field("games"), "5").unwrap();

    assert!(form.view().complete);
    for id in session.add_preset(&form, &english()).unwrap() {
        let phase = session.phase_form_to_edit(id).unwrap();
        let answer = phase
            .draft()
            .answers()
            .get(PhasePlugins::MATCH_FORMAT)
            .and_then(PluginConfig::from_answer)
            .unwrap();
        assert_eq!(answer.plugin(), BestOf::ID);
        assert_eq!(answer.config().get("games"), Some(&5.into()));
    }
}

#[test]
fn a_preset_sets_up_only_a_competition_without_phases() {
    let mut session = with_entrants(&["Alice", "Bob"]);
    session.add_preset(&groups(1), &english()).unwrap();

    assert!(matches!(
        session.add_preset(&groups(1), &english()),
        Err(AddPresetError::HasPhases)
    ));
    assert_eq!(session.phases().len(), 2);
}

#[test]
fn a_dyp_plays_through() {
    let mut session = with_entrants(&["A", "B", "C", "D", "E", "F", "G", "H"]);
    let form = Session::preset_form(Dyp::ID).unwrap();
    session.add_preset(&form, &english()).unwrap();
    session.start().unwrap();
    while let Some(row) = session.playable(session.all()).first().cloned() {
        session
            .report(session.first(), row.id, MatchResult::AWins)
            .unwrap();
    }
    let ranking = session.ranking();
    assert!(ranking.is_final);
    // four pairs
    assert_eq!(ranking.rows.len(), 4);
}

#[test]
fn an_unknown_preset_has_no_form() {
    assert!(matches!(
        Session::preset_form("nothing"),
        Err(crate::error::PresetFormError::UnknownPreset(id)) if id == "nothing"
    ));
}
