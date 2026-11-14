use std::collections::BTreeSet;

use plugin_factory::{
    PluginType as _,
    catalog::{AnyAxis, PluginEntry},
    config::PluginConfig,
    schema::{Configurable as _, Schema},
};
use tournament_engine::{
    EnteredValue, PhasePlugins, Score, Sides,
    plugins::{
        Phrase,
        match_format::MatchFormat,
        metrics_calculator::{
            Cell, GamesBalance, GamesBalanceCalculator, MetricsCalculator, PointsBalance,
            PointsBalanceCalculator, PointsCalculator, RoundOfElimination,
            RoundOfEliminationCalculator, RoundOfFirstLoss, RoundOfFirstLossCalculator,
            ToCell as _, WinsDrawsLosses, WinsDrawsLossesCalculator,
        },
        phase_format::{
            DoubleElimination, DoubleEliminationRound, KnockoutRound, PhaseFormat, RoundRobin,
            SingleElimination, Stage,
        },
        ranker::SliceRanker,
    },
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use crate::{
    error::{AddEntrantError, EditPhaseError, NoPhase, ReportError, UndoReportError},
    result_form::PartView,
    session::Session,
};

#[test]
fn a_language_is_found_by_its_tag_whatever_the_region() {
    assert_eq!(Language::from_tag("de-AT"), Some(Language::German));
    assert_eq!(Language::from_tag("EN"), Some(Language::English));
    assert_eq!(Language::from_tag("fr"), None);
}

#[test]
fn the_first_language_spoken_of_those_preferred_is_chosen() {
    assert_eq!(
        Language::negotiate(["fr-FR", "de-CH", "en-US"]),
        Language::German
    );
    assert_eq!(Language::negotiate(["fr-FR"]), Language::English);
    assert_eq!(Language::negotiate([]), Language::English);
}

#[test]
fn a_text_is_said_in_the_language_with_its_variables() {
    let translator = Translator::new(Language::German, &[]);

    assert_eq!(
        translator.text(&Text::new("error-duplicate-name").with("name", "Alice")),
        "es gibt schon einen Teilnehmer namens „\u{2068}Alice\u{2069}“",
        "a value is isolated from the text around it"
    );
    assert_eq!(
        translator.text(&Text::new("error-no-match-numbered")),
        "es gibt kein Spiel \u{2068}{$number}\u{2069}",
        "a variable not set shows as such"
    );
}

#[test]
fn a_sentence_begins_with_a_capital_letter_and_ends_with_a_full_stop() {
    let resources: Resources =
        |_| "fragment = übrig\nvariable = { $name } fehlt\nquestion = wer?\nended = { $reason }\n";
    let translator = Translator::new(Language::German, &[resources]);

    assert_eq!(translator.sentence(&Text::new("fragment")), "Übrig.");
    assert_eq!(
        translator.sentence(&Text::new("variable").with("name", "alice")),
        "\u{2068}Alice\u{2069} fehlt."
    );
    assert_eq!(translator.sentence(&Text::new("question")), "Wer?");
    assert_eq!(
        translator.sentence(&Text::new("ended").with("reason", "done.")),
        "Done."
    );
}

#[test]
fn a_message_missing_in_the_language_is_said_in_english() {
    let resources: Resources = |language| match language {
        Language::English => "greeting = hello",
        Language::German => "",
    };

    let translator = Translator::new(Language::German, &[resources]);

    assert_eq!(translator.get("greeting"), "hello");
    assert_eq!(
        translator.get("unknown"),
        "unknown",
        "a key nowhere is shown"
    );
}

#[test]
fn the_messages_of_a_consumer_come_after_those_of_the_app() {
    let resources: Resources = |language| match language {
        Language::English => "greeting = hello",
        Language::German => "greeting = hallo",
    };

    let translator = Translator::new(Language::German, &[resources]);

    assert_eq!(translator.get("greeting"), "hallo");
    assert_eq!(
        translator.get("error-no-phase"),
        "das Turnier hat noch keine Phase"
    );
}

#[test]
fn an_error_is_said_in_the_language() {
    let translator = Translator::new(Language::German, &[]);

    assert_eq!(
        translator.message(&ReportError::DrawNotAllowed),
        "ein Spiel dieses Formats kann nicht unentschieden enden"
    );
}

/// A message naming several matches says so in the plural.
#[test]
fn several_matches_are_named_in_the_plural() {
    let translator = Translator::new(Language::German, &[]);

    assert_eq!(
        translator
            .message(&UndoReportError::DependedOnBy(vec![3, 4]))
            .replace(['\u{2068}', '\u{2069}'], ""),
        "die Ergebnisse der Spiele 3, 4 hängen von diesem ab; zuerst müssen die Spiele 3, 4 wieder \
         geöffnet werden"
    );
}

/// An error's `Display` is its message in English, without the marks isolating its values.
#[test]
fn an_error_displays_its_english_message() {
    assert_eq!(
        AddEntrantError::DuplicateName("Alice".to_owned()).to_string(),
        "there already is an entrant named “Alice”"
    );
    assert_eq!(
        UndoReportError::DependedOnBy(vec![3, 4, 10]).to_string(),
        "the results of matches 3, 4, 10 depend on this one; reopen those matches first"
    );
    assert_eq!(
        EditPhaseError::NoPhase(NoPhase).to_string(),
        "the competition has no phase yet"
    );
}

#[test]
fn the_messages_of_the_app_are_in_every_language() {
    check_languages(|language| language.resources()[0]).unwrap();
}

/// The app's messages come first, and a key given twice keeps its first message.
#[test]
fn a_check_finds_messages_of_the_app() {
    let resources: Resources = |_| "phase = Phase\n";

    assert_eq!(
        check(resources).unwrap_err(),
        [
            "en: phase is a message of the app",
            "de: phase is a message of the app"
        ]
    );
}

#[test]
fn a_check_finds_messages_missing_given_twice_or_with_other_attributes() {
    let resources: Resources = |language| match language {
        Language::English => "a = a\nb = b\nc =\n    .label = c\n",
        Language::German => "a = a\na = a\nc = c\nd = d\n",
    };

    assert_eq!(
        check(resources).unwrap_err(),
        [
            "de: a is given twice",
            "de: b is missing",
            r#"de: c has the attributes {} rather than {"label"}"#,
            "de: d is not in English",
        ]
    );
}

#[test]
fn a_plugin_is_said_in_the_language_or_else_as_its_code_says() {
    let german = Translator::new(Language::German, &[]);

    assert_eq!(
        german.plugin(
            "tournament_engine::round_robin",
            PluginText::Label,
            "round robin"
        ),
        "Jeder gegen jeden"
    );
    assert_eq!(
        german.plugin(
            "tournament_engine::points",
            PluginText::Field("points_for_bye"),
            "points for a bye"
        ),
        "Punkte für ein Freilos"
    );
    assert_eq!(
        german.plugin("my_crate::swiss", PluginText::Label, "Swiss"),
        "Swiss"
    );
    assert_eq!(
        Translator::new(Language::English, &[]).plugin(
            "tournament_engine::round_robin",
            PluginText::Label,
            "round robin"
        ),
        "round robin"
    );
}

#[test]
fn a_stage_is_said_in_the_language_with_its_numbers_or_else_as_its_code_says() {
    let german = Translator::new(Language::German, &[]);
    let stage = |format, stage| StageName { format, stage };

    assert_eq!(
        german.stage(&stage(
            SingleElimination::ID,
            KnockoutRound::RoundOf(16).into()
        )),
        "Achtelfinale"
    );
    assert_eq!(
        german.stage(&stage(
            DoubleElimination::ID,
            DoubleEliminationRound::Losers {
                round: 3,
                rounds: 4
            }
            .into()
        )),
        "Verliererrunde \u{2068}3\u{2069}"
    );
    assert_eq!(
        german.stage(&stage("my_crate::swiss", Stage::new("playoff", "Playoff"))),
        "Playoff"
    );
    assert_eq!(
        Translator::new(Language::English, &[]).stage(&stage(
            SingleElimination::ID,
            KnockoutRound::RoundOf(16).into()
        )),
        "Round of 16"
    );
}

#[test]
fn a_cell_is_said_in_the_language_or_else_as_its_code_says() {
    let german = Translator::new(Language::German, &[]);

    assert_eq!(
        german.cell(
            RoundOfFirstLossCalculator::ID,
            &RoundOfFirstLoss(NonZeroU32::new(3)).to_cell()
        ),
        "Runde \u{2068}3\u{2069}"
    );
    assert_eq!(
        german.cell(
            "my_crate::elo",
            &Cell::Text(Phrase::new("rating", "1500 Elo"))
        ),
        "1500 Elo"
    );
    assert_eq!(
        german.cell(PointsCalculator::ID, &Cell::Number(Score::new(2.5))),
        "2,5"
    );
    assert_eq!(
        Translator::new(Language::English, &[])
            .cell(PointsCalculator::ID, &Cell::Number(Score::new(2.5))),
        "2.5"
    );
    let signed = |number| german.cell("my_crate::difference", &Cell::Signed(Score::new(number)));
    assert_eq!(signed(2.5), "+2,5");
    assert_eq!(signed(-3.0), "\u{2212}3");
    assert_eq!(signed(0.0), "0");
}

/// The plugins of the engine, of every axis, except those only its tests use.
fn engine_plugins() -> Vec<&'static PluginEntry> {
    [
        AnyAxis::of::<dyn PhaseFormat>(),
        AnyAxis::of::<dyn SliceRanker>(),
        AnyAxis::of::<dyn MetricsCalculator>(),
        AnyAxis::of::<dyn MatchFormat>(),
    ]
    .iter()
    .flat_map(AnyAxis::entries)
    .filter(|entry| {
        entry.id().starts_with("tournament_engine::") && !entry.id().contains("::test_utils::")
    })
    .collect()
}

/// What is translated of `entry`, as the attributes of its message.
fn translatable(entry: &PluginEntry) -> BTreeSet<String> {
    let mut attributes = BTreeSet::from([PluginText::Label.attribute()]);
    if entry.description().is_some() {
        attributes.insert(PluginText::Description.attribute());
    }
    attributes.extend(translatable_fields(&entry.schema()));
    attributes.extend(
        stages(entry.id())
            .iter()
            .map(|stage| PluginText::Stage(stage.key()).attribute()),
    );
    attributes.extend(
        phrases_in_cells(entry.id())
            .iter()
            .map(|phrase| PluginText::Cell(phrase.key()).attribute()),
    );
    if AnyAxis::of::<dyn MatchFormat>()
        .entries()
        .any(|format| format.id() == entry.id())
    {
        attributes.extend(phrases_of_rules(entry.id()));
    }
    attributes
}

/// What the result of a match entered by the match format `id` names, as attributes: found from a
/// form to enter it, in a round robin of two whose results are entered by the match format with
/// their defaults.
fn phrases_of_rules(id: &str) -> BTreeSet<String> {
    fn collect(part: &PartView, attributes: &mut BTreeSet<String>) {
        attributes.insert(PluginText::Level(part.name.key()).attribute());
        for field in &part.fields {
            attributes.insert(PluginText::EnteredValue(field.label.key()).attribute());
        }
        for (_, label) in &part.reasons {
            attributes.insert(PluginText::Reason(label.key()).attribute());
        }
        for part in &part.parts {
            collect(part, attributes);
        }
    }
    let mut session = Session::new("cup");
    for name in ["Alice", "Bob"] {
        session.add_entrant(name).unwrap();
    }
    let draft = session
        .phase_draft(RoundRobin::ID)
        .unwrap()
        .with("match_format", PluginConfig::new(id))
        .and_then(|draft| draft.with_defaults())
        .unwrap();
    session.add_phase(&draft).unwrap();
    session.start().unwrap();
    let r#match = session
        .competition()
        .phases()
        .values()
        .next()
        .unwrap()
        .data();
    let id = r#match.matches().values().next().unwrap().id();
    let mut attributes = BTreeSet::new();
    collect(
        &session.result_form(id).unwrap().view().root,
        &mut attributes,
    );
    attributes
}

/// A phrase of every key the column of the calculator `id` shows, none for a plugin that is no
/// such calculator. They cannot be found from the plugin, so the metrics showing them are listed
/// here.
fn phrases_in_cells(id: &str) -> Vec<Phrase> {
    let cells = if id == RoundOfFirstLossCalculator::ID {
        vec![
            RoundOfFirstLoss(NonZeroU32::new(1)).to_cell(),
            RoundOfFirstLoss(None).to_cell(),
        ]
    } else if id == RoundOfEliminationCalculator::ID {
        vec![
            RoundOfElimination(NonZeroU32::new(1)).to_cell(),
            RoundOfElimination(None).to_cell(),
        ]
    } else if id == WinsDrawsLossesCalculator::ID {
        vec![WinsDrawsLosses::default().to_cell()]
    } else if id == PointsBalanceCalculator::ID {
        vec![PointsBalance::default().to_cell()]
    } else if id == GamesBalanceCalculator::ID {
        vec![GamesBalance::default().to_cell()]
    } else {
        Vec::new()
    };
    cells
        .into_iter()
        .filter_map(|cell| match cell {
            Cell::Text(phrase) => Some(phrase),
            Cell::Number(_) | Cell::Signed(_) | Cell::Round(_) => None,
        })
        .collect()
}

/// A stage of every key the format `id` names, none for a plugin that is no such format. The
/// stages cannot be found from the plugin, so the formats naming them are listed here.
fn stages(id: &str) -> Vec<Stage> {
    if id == SingleElimination::ID {
        [
            KnockoutRound::RoundOf(2),
            KnockoutRound::RoundOf(4),
            KnockoutRound::RoundOf(8),
            KnockoutRound::RoundOf(16),
            KnockoutRound::ThirdPlace,
        ]
        .map(Stage::from)
        .into()
    } else if id == DoubleElimination::ID {
        [
            DoubleEliminationRound::Winners {
                round: 1,
                rounds: 2,
            },
            DoubleEliminationRound::Winners {
                round: 2,
                rounds: 2,
            },
            DoubleEliminationRound::Losers {
                round: 1,
                rounds: 2,
            },
            DoubleEliminationRound::Losers {
                round: 2,
                rounds: 2,
            },
            DoubleEliminationRound::GrandFinal,
            DoubleEliminationRound::GrandFinalReset,
        ]
        .map(Stage::from)
        .into()
    } else {
        Vec::new()
    }
}

/// What is translated of the fields of `schema`, as attributes.
fn translatable_fields(schema: &Schema) -> BTreeSet<String> {
    let mut attributes = BTreeSet::new();
    for field in schema.fields() {
        attributes.insert(PluginText::Field(field.name()).attribute());
        if field.description().is_some() {
            attributes.insert(PluginText::FieldDescription(field.name()).attribute());
        }
    }
    attributes
}

/// The fields of a phase itself are translated as well, in the message [`PHASE`].
#[test]
fn every_plugin_of_the_engine_is_translated_and_nothing_else() {
    let calculators: Vec<_> = AnyAxis::of::<dyn MetricsCalculator>()
        .entries()
        .map(PluginEntry::id)
        .collect();
    let expected: BTreeMap<String, BTreeSet<String>> = engine_plugins()
        .into_iter()
        .map(|entry| (PluginText::key(entry.id()), translatable(entry)))
        .chain([(
            PHASE.to_owned(),
            translatable_fields(&PhasePlugins::schema()),
        )])
        .collect();
    for language in Language::ALL {
        if language == Language::English {
            continue;
        }
        let resource = parser::parse(language.resources()[1]).unwrap();
        let mut translated = BTreeMap::new();
        for entry in resource.body {
            if let ast::Entry::Message(message) = entry {
                let mut attributes: BTreeSet<_> = message
                    .attributes
                    .iter()
                    .map(|attribute| attribute.id.name.to_owned())
                    .collect();
                let key = message.id.name.to_owned();
                // only a calculator has a column, and not every calculator: the column cannot be
                // checked without the plugin
                if calculators.iter().any(|id| PluginText::key(id) == key) {
                    attributes.remove(&PluginText::Column.attribute());
                }
                assert!(
                    translated.insert(key.clone(), attributes).is_none(),
                    "{key} is given twice"
                );
            }
        }
        assert_eq!(translated, expected, "{}", language.tag());
    }
}

#[test]
fn scores_are_written_as_the_language_writes_their_numbers() {
    let translator = Translator::new(Language::German, &[]);
    let scores = [
        Sides::new(Some(EnteredValue::Count(21)), Some(EnteredValue::Count(17))),
        Sides::new(Some(EnteredValue::Decimal(Score::new(1.5))), None),
    ];

    assert_eq!(translator.scores(&scores), "21–17 1,5–-");
}

#[test]
fn the_parts_won_go_before_the_scores_of_a_match() {
    let translator = Translator::new(Language::English, &[]);
    let count = |count| Some(EnteredValue::Count(count));
    let scores = [
        Sides::new(count(21), count(17)),
        Sides::new(count(19), count(21)),
        Sides::new(count(21), count(15)),
    ];

    assert_eq!(
        translator.match_scores(Some(Sides::new(2, 1)), &scores),
        "2–1 (21–17 19–21 21–15)"
    );
    assert_eq!(translator.match_scores(None, &scores[..1]), "21–17");
    assert_eq!(
        translator.match_scores(Some(Sides::new(1, 0)), &[]),
        "1–0",
        "games decided without a score"
    );
}
