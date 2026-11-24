use plugin_factory::schema::Level;
use tournament_engine::plugins::{
    seeding::{GroupWinnersApart, SeededAtRandom, SeededInOrder},
    teams::RandomPairs,
    template::{GroupsAndKnockout, MonsterDypTemplate, RoundRobinTemplate, Template},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::AddTemplateError,
    form::{Editable, Mark},
    i18n::{Language, Translator},
    session::{Dyp, MatchFormats},
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

/// The forms of a template's phases are named in the language, and a later one knows the
/// phases before it, which are not added yet, so that it takes places of them.
#[test]
fn the_forms_of_a_template_know_the_phases_before_them() {
    let session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    let template = session
        .template_form(
            GroupsAndKnockout::ID,
            &Translator::new(Language::German, &[]),
        )
        .unwrap();
    let names: Vec<_> = (0..2).map(|index| template.name(index).name).collect();
    assert_eq!(
        names,
        [
            Some("Gruppenphase".to_owned()),
            Some("K.-o.-Phase".to_owned())
        ]
    );

    let knockout = template.forms()[1].entry_view();
    assert_eq!(knockout.rows[0].of, Some(template.ids()[0]));
    assert_eq!(knockout.rows[0].to, "4");
    assert!(
        knockout
            .choices
            .iter()
            .any(|choice| choice.phase == Some(template.ids()[0]))
    );
    assert!(template.complete());
    assert_eq!(
        session.competition().phases().len(),
        0,
        "nothing is added yet"
    );
}

/// What is changed in the forms before adding is what is added, and the forms after a changed
/// one follow it, e.g. a knockout takes places of three groups.
#[test]
fn the_phases_of_a_template_are_added_as_their_forms_were_changed() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave", "Eve", "Frank"]);
    let mut template = session
        .template_form(GroupsAndKnockout::ID, &english())
        .unwrap();
    let knockout = template.revision(1);
    let mut groups = template.forms()[0].clone();
    groups.enter(&"divisions".parse().unwrap(), "3").unwrap();
    template.replace(0, groups);
    assert!(
        template.revision(1) > knockout,
        "the knockout knows the groups"
    );

    let ids = session.add_template_form(&template).unwrap();
    assert_eq!(
        session.competition().phases()[&ids[0]]
            .plugins()
            .divisions()
            .get(),
        3
    );
    assert!(session.based_on(&english()).is_some());
    session.undo().unwrap();
    assert!(session.competition().phases().is_empty());
}

/// The match format and drawn pairs are set on the forms at once, the pairs on those taking the
/// registration, which a later phase then takes as pairs.
#[test]
fn the_settings_of_the_whole_competition_are_set_on_the_forms() {
    let session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    let mut template = session
        .template_form(GroupsAndKnockout::ID, &english())
        .unwrap();
    assert!(matches!(template.match_formats(), MatchFormats::Same(_)));
    assert_eq!(template.dyp(), Some(Dyp::Off));

    assert!(template.set_dyp(true).unwrap());
    assert_eq!(template.dyp(), Some(Dyp::On));
    assert_eq!(
        template.forms()[1].entrants_per_qualifier(),
        NonZeroU32::new(2),
        "the knockout takes the pairs"
    );

    let mut best_of = template.forms()[0].clone();
    best_of
        .choose(&"match_format".parse().unwrap(), Some(BestOf::ID))
        .unwrap();
    assert!(template.set_match_format(&best_of).unwrap());
    let MatchFormats::Same(form) = template.match_formats() else {
        panic!("one match format");
    };
    assert_eq!(
        form.draft().answers().get(PhasePlugins::MATCH_FORMAT),
        best_of.draft().answers().get(PhasePlugins::MATCH_FORMAT)
    );
}

/// A phase that cannot be played as its form stands keeps the template from being added, e.g. a
/// Monster DYP of drawn pairs, which draws pairs of its own.
#[test]
fn a_template_with_a_phase_that_cannot_be_played_is_not_added() {
    let mut session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    let mut template = session
        .template_form(MonsterDypTemplate::ID, &english())
        .unwrap();
    let mut pairs = template.forms()[0].clone();
    pairs
        .choose(&"teams".parse().unwrap(), Some(RandomPairs::ID))
        .unwrap();
    template.replace(0, pairs);
    assert!(!template.complete());

    assert!(matches!(
        session.add_template_form(&template),
        Err(AddTemplateError::Invalid(phase)) if phase.number == 1
    ));
    assert!(session.competition().phases().is_empty());
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
fn an_unknown_template_is_neither_set_up_nor_added() {
    let mut session = with_entrants(&["Alice", "Bob"]);
    assert!(matches!(
        session.template_form("nothing", &english()),
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

/// The knockout of the template of groups and a knockout, as a form before it is added, whose
/// template overrides its seeding: the groups kept apart.
fn knockout() -> PhaseForm {
    let session = with_entrants(&["Alice", "Bob", "Carol", "Dave"]);
    let template = session
        .template_form(GroupsAndKnockout::ID, &english())
        .unwrap();
    template.forms()[1].clone()
}

fn seeding(form: &PhaseForm) -> Option<String> {
    form.draft()
        .answers()
        .get(PhasePlugins::SEEDING)
        .and_then(PluginConfig::from_answer)
        .map(|config| config.plugin().to_owned())
}

fn format() -> ConfigPath {
    ConfigPath::ROOT.field(PhasePlugins::FORMAT)
}

/// What a template overrides stays as the format changes while it fits and matters, follows what
/// the field offers once it does not, and comes back with a format it matters to.
#[test]
fn a_setting_of_a_template_stays_while_it_matters_to_the_format() {
    let mut knockout = knockout();
    assert_eq!(seeding(&knockout).as_deref(), Some(GroupWinnersApart::ID));

    knockout
        .choose(&format(), Some(DoubleElimination::ID))
        .unwrap();
    assert_eq!(seeding(&knockout).as_deref(), Some(GroupWinnersApart::ID));

    // random rounds read no seeding
    knockout.choose(&format(), Some(RandomRounds::ID)).unwrap();
    assert_eq!(seeding(&knockout).as_deref(), Some(SeededInOrder::ID));
    assert_eq!(knockout.view_at(Level::Basic).hidden.count, 0);

    knockout
        .choose(&format(), Some(SingleElimination::ID))
        .unwrap();
    assert_eq!(seeding(&knockout).as_deref(), Some(GroupWinnersApart::ID));
    assert_eq!(knockout.view_at(Level::Basic).hidden.count, 0);
}

/// A setting the operator changed stays as it is when the format changes, and is said to differ
/// from the template.
#[test]
fn a_setting_changed_by_hand_stays_as_the_format_changes() {
    let mut knockout = knockout();
    knockout
        .choose(
            &ConfigPath::ROOT.field(PhasePlugins::SEEDING),
            Some(SeededAtRandom::ID),
        )
        .unwrap();

    knockout
        .choose(&format(), Some(DoubleElimination::ID))
        .unwrap();

    assert_eq!(seeding(&knockout).as_deref(), Some(SeededAtRandom::ID));
    let hidden = knockout.view_at(Level::Basic).hidden;
    assert_eq!((hidden.count, hidden.shown_at), (1, Level::Expert));
    assert!(hidden.of_template);
}

/// What followed a new format is named where the view shows it; where it hides it, only the
/// levels showing it are said, with the lowest to switch to.
#[test]
fn what_followed_a_new_format_is_said_as_far_as_the_level_shows_it() {
    let mut knockout = knockout();
    knockout
        .choose(&format(), Some(DoubleElimination::ID))
        .unwrap();
    let tr = english();
    let said = |level| {
        let followed = knockout.view_at(level).followed.unwrap();
        let said: Vec<_> = followed
            .said
            .iter()
            .map(|text| tr.sentence(text).replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        (followed.at, said, followed.show)
    };

    assert_eq!(
        said(Level::Basic),
        (
            Some(format()),
            vec![
                "The advanced settings were adjusted to the format double elimination.".to_owned()
            ],
            Some(Level::Advanced)
        )
    );
    assert_eq!(
        said(Level::Expert),
        (
            Some(format()),
            vec![
                "Adjusted to the format double elimination: order of the standings and metrics."
                    .to_owned()
            ],
            None
        )
    );

    knockout
        .set_bool(&"format.grand_final_reset".parse().unwrap(), false)
        .unwrap();
    assert_eq!(
        knockout.view().followed,
        None,
        "nothing followed the last change"
    );
}

/// A phase of a template marks what differs from the template: its format, a setting changed.
#[test]
fn what_differs_from_the_template_is_marked() {
    let mut knockout = knockout();
    assert!(!knockout.view().marked);

    knockout
        .choose(
            &ConfigPath::ROOT.field(PhasePlugins::SEEDING),
            Some(SeededAtRandom::ID),
        )
        .unwrap();
    knockout
        .choose(&format(), Some(DoubleElimination::ID))
        .unwrap();

    let view = knockout.view();
    let mark = |name: &str| {
        view.fields
            .iter()
            .find(|field| field.name == name)
            .and_then(|field| field.mark)
    };
    assert_eq!(mark(PhasePlugins::SEEDING), Some(Mark::Differs));
    assert_eq!(mark(PhasePlugins::FORMAT), Some(Mark::Differs));
    assert_eq!(mark(PhasePlugins::RANKING), None, "it followed the format");
}
