use tournament_engine::plugins::{
    teams::RandomPairs,
    template::{GroupsAndKnockout, MonsterDypTemplate, RoundRobinTemplate},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    form::Editable as _,
    i18n::{Language, Translator},
    session::{Dyp, MatchFormats},
};

/// A session of four entrants set up by the template `id`.
fn set_up(id: &str) -> Session {
    let mut session = Session::new("cup");
    session
        .add_named_entrants(&["Alice", "Bob", "Carol", "Dave"])
        .unwrap();
    session
        .add_template(id, &Translator::new(Language::English, &[]))
        .unwrap();
    session
}

/// The id of the plugin answering the field `field` of each phase of `session`, in order.
fn answered(session: &Session, field: &str) -> Vec<String> {
    session
        .competition()
        .phases()
        .values()
        .map(|phase| {
            PluginConfig::from_answer(phase.config().get(field).unwrap())
                .unwrap()
                .plugin()
                .to_owned()
        })
        .collect()
}

/// The form of the match format of every phase of `session`, which have the same.
fn same(session: &Session) -> PhaseForm {
    match session.match_formats() {
        Some(MatchFormats::Same(form)) => *form,
        Some(MatchFormats::Mixed) => panic!("the match formats are mixed"),
        None => panic!("there is no phase"),
    }
}

#[test]
fn the_match_format_is_set_on_every_phase_as_one_action() {
    let mut session = set_up(GroupsAndKnockout::ID);
    let mut form = same(&session);
    let match_format = ConfigPath::ROOT.field(PhasePlugins::MATCH_FORMAT);

    form.choose(&match_format, Some(BestOf::ID)).unwrap();
    assert!(session.set_match_format(&form).unwrap());

    assert_eq!(
        answered(&session, PhasePlugins::MATCH_FORMAT),
        [BestOf::ID, BestOf::ID]
    );
    assert!(matches!(
        session.undo(),
        Ok(Action::EditPhases(phases)) if phases.len() == 2
    ));
    assert!(matches!(
        session.match_formats(),
        Some(MatchFormats::Same(_))
    ));
    assert!(
        !session.set_match_format(&same(&session)).unwrap(),
        "nothing changes"
    );
}

#[test]
fn phases_of_different_match_formats_are_mixed() {
    let mut session = set_up(GroupsAndKnockout::ID);
    let knockout = session.phases()[1].id;
    let mut form = session.phase_form_to_edit(knockout).unwrap();
    form.choose(
        &ConfigPath::ROOT.field(PhasePlugins::MATCH_FORMAT),
        Some(Scored::ID),
    )
    .unwrap();
    session.edit_phase(knockout, &form).unwrap();

    assert!(matches!(session.match_formats(), Some(MatchFormats::Mixed)));
    assert!(Session::new("cup").match_formats().is_none());
}

#[test]
fn the_phases_taking_the_registration_are_played_by_drawn_pairs() {
    let mut session = set_up(RoundRobinTemplate::ID);
    assert_eq!(session.dyp(), Some(Dyp::Off));

    assert!(session.set_dyp(true).unwrap());

    assert_eq!(session.dyp(), Some(Dyp::On));
    assert_eq!(answered(&session, PhasePlugins::TEAMS), [RandomPairs::ID]);
    session.set_dyp(false).unwrap();
    assert_eq!(session.dyp(), Some(Dyp::Off));
}

/// The phases after those taking the registration keep the teams they are sent.
#[test]
fn drawn_pairs_go_on_to_the_phases_after() {
    let mut session = set_up(GroupsAndKnockout::ID);

    session.set_dyp(true).unwrap();

    assert_eq!(
        answered(&session, PhasePlugins::TEAMS)[0],
        RandomPairs::ID,
        "the groups draw the pairs"
    );
    assert_ne!(answered(&session, PhasePlugins::TEAMS)[1], RandomPairs::ID);
    assert_eq!(session.dyp(), Some(Dyp::On));
}

/// A Monster DYP draws pairs of its own.
#[test]
fn drawn_pairs_are_not_offered_for_a_monster_dyp() {
    assert_eq!(set_up(MonsterDypTemplate::ID).dyp(), None);
    assert_eq!(Session::new("cup").dyp(), None);
}
