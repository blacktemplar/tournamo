//! The converters bringing a file of an older [`Format`] to the current one, on its untyped JSON,
//! one format at a time: [`File::read`](super::File::read) converts a file of an older format
//! before reading it into the current types, which read only the current layout.
//!
//! A change of what is saved raises [`Format::CURRENT`] and adds the converter from the format
//! before to [`CONVERTERS`]; without it, the crate does not build. A converter
//!
//! - writes literal JSON of its target format. It never calls the engine's serialization or
//!   types, since those keep changing; a later converter takes it from there.
//! - changes only what it recognises and leaves everything else as it is, so that damaged input
//!   fails later, in the typed read, with the usual error. It never panics, whatever the JSON.
//! - is never changed once released, like a golden save.

use serde_json::Value;

use super::Format;

/// Brings what a file of one format holds to the next format.
struct Converter {
    /// One entry of the log, as the JSON of a `LogEntry`.
    log_entry: fn(&mut Value),
    /// The competition stored next to the log.
    competition: fn(&mut Value),
}

/// `CONVERTERS[n]` converts format `n + 1` to format `n + 2`.
const CONVERTERS: &[Converter] = &[Converter {
    log_entry: v2::log_entry,
    competition: v2::competition,
}];

const _: () = assert!(
    CONVERTERS.len() as u32 == Format::CURRENT.0 - Format::FIRST.0,
    "every format after the first needs its converter"
);

/// Brings `file`, the JSON of a file of the format `from`, to [`Format::CURRENT`]: every entry of
/// its `"log"` and its `"competition"`, if any, and sets its `"format"`.
pub(crate) fn upgrade(file: &mut Value, from: Format) {
    for converter in converters(from) {
        if let Some(Value::Array(log)) = file.get_mut("log") {
            log.iter_mut().for_each(converter.log_entry);
        }
        if let Some(competition) = file.get_mut("competition") {
            (converter.competition)(competition);
        }
    }
    if let Some(file) = file.as_object_mut() {
        file.insert("format".to_owned(), Format::CURRENT.0.into());
    }
}

/// Brings `competition`, the JSON of a competition as a file of the format `from` holds it, to
/// [`Format::CURRENT`].
pub fn upgrade_competition(competition: &mut Value, from: Format) {
    for converter in converters(from) {
        (converter.competition)(competition);
    }
}

/// The converters a file of the format `from` goes through, in order.
fn converters(from: Format) -> impl Iterator<Item = &'static Converter> {
    CONVERTERS.iter().skip((from.0 - Format::FIRST.0) as usize)
}

/// Format 1 to 2 (0.4.0): a report holds the decision of its match and what was entered for it,
/// where it held the result; a phase has a match format; a match holds its contest and tallies,
/// where it held its result and annotations.
mod v2 {
    use serde_json::{Map, Value, json};

    pub(super) fn log_entry(entry: &mut Value) {
        let transition = if entry.pointer("/kind/Apply").is_some() {
            "/kind/Apply"
        } else {
            "/kind/Replace/with"
        };
        let Some(transition) = entry.pointer_mut(transition) else {
            return;
        };
        let convert: fn(&mut Map<String, Value>) =
            match transition.get("type").and_then(Value::as_str) {
                Some("tournament_engine::report_match_result") => report,
                Some("tournament_engine::add_phase") => add_phase,
                _ => return,
            };
        if let Some(Value::Object(value)) = transition.get_mut("value") {
            convert(value);
        }
    }

    /// A `ReportMatchResult`: the result reported becomes its decision, with nothing entered.
    fn report(report: &mut Map<String, Value>) {
        if !report.contains_key("decision")
            && let Some(result) = report.remove("match_result")
        {
            report.insert(
                "decision".to_owned(),
                json!({ "result": result, "reason": null }),
            );
        }
        report.entry("values").or_insert_with(|| json!([]));
        report.entry("parts").or_insert_with(|| json!([]));
    }

    /// An `AddPhase`: its plugins get the match format of a phase that had none.
    fn add_phase(add_phase: &mut Map<String, Value>) {
        if let Some(Value::Object(plugins)) = add_phase.get_mut("plugins") {
            plugins.entry("match_format").or_insert_with(winner_only);
        }
    }

    pub(super) fn competition(competition: &mut Value) {
        let Some(Value::Array(phases)) = competition.get_mut("phases") else {
            return;
        };
        for phase in phases.iter_mut().filter_map(Value::as_object_mut) {
            phase.entry("match_format").or_insert_with(winner_only);
            if let Some(Value::Array(matches)) = phase
                .get_mut("data")
                .and_then(|data| data.get_mut("matches"))
            {
                matches
                    .iter_mut()
                    .filter_map(Value::as_object_mut)
                    .for_each(contest);
            }
        }
    }

    /// A match: its result and annotations become its contest, with nothing entered, and it has
    /// no tallies.
    fn contest(r#match: &mut Map<String, Value>) {
        if r#match.contains_key("contest") {
            return;
        }
        let decision = match r#match.remove("result") {
            None | Some(Value::Null) => Value::Null,
            Some(result) => json!({ "result": result, "reason": null }),
        };
        let annotations = r#match.remove("annotations").unwrap_or_else(|| json!([]));
        r#match.insert(
            "contest".to_owned(),
            json!({
                "decision": decision,
                "values": [],
                "parts": [],
                "annotations": annotations,
            }),
        );
        r#match.entry("tallies").or_insert_with(|| json!([]));
    }

    /// The match format of a phase that had none, as every phase had before: who won, nothing
    /// more.
    fn winner_only() -> Value {
        json!({ "type": "tournament_engine::winner_only", "value": null })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// `entry`, an entry of the log of a file of the first format, as the current format holds it.
    fn upgraded_entry(entry: Value) -> Value {
        let mut file = json!({ "log": [entry], "people": [] });
        upgrade(&mut file, Format::FIRST);
        file["log"][0].take()
    }

    /// `competition`, held by a file of the first format, as the current format holds it.
    fn upgraded_competition(mut competition: Value) -> Value {
        upgrade_competition(&mut competition, Format::FIRST);
        competition
    }

    /// A competition of one phase, with `phase` added to what a phase holds.
    fn competition_of(phase: Value) -> Value {
        let mut competition = json!({
            "entrants": [],
            "phases": [{
                "id": "phase",
                "phase_format": { "type": "tournament_engine::round_robin", "value": null },
                "ranker": [],
                "metrics_calculators": [],
                "data": { "rankings": [], "matches": [], "byes": [], "seed": [] },
            }],
            "started": true,
            "seed": [],
        });
        let phase_of = competition["phases"][0].as_object_mut().unwrap();
        phase_of.extend(phase.as_object().unwrap().clone());
        competition
    }

    #[test]
    fn a_reported_result_becomes_a_decision_with_nothing_entered() {
        let report = json!({
            "id": "report",
            "kind": { "Apply": {
                "type": "tournament_engine::report_match_result",
                "value": { "phase_id": "phase", "match_id": "match", "match_result": "AWins" },
            } },
        });

        assert_eq!(
            upgraded_entry(report),
            json!({
                "id": "report",
                "kind": { "Apply": {
                    "type": "tournament_engine::report_match_result",
                    "value": {
                        "phase_id": "phase",
                        "match_id": "match",
                        "decision": { "result": "AWins", "reason": null },
                        "values": [],
                        "parts": [],
                    },
                } },
            })
        );
    }

    #[test]
    fn a_transition_replacing_another_is_converted_too() {
        let replace = json!({
            "id": "replace",
            "kind": { "Replace": {
                "target": "report",
                "with": {
                    "type": "tournament_engine::report_match_result",
                    "value": { "phase_id": "phase", "match_id": "match", "match_result": "Draw" },
                },
            } },
        });

        assert_eq!(
            upgraded_entry(replace),
            json!({
                "id": "replace",
                "kind": { "Replace": {
                    "target": "report",
                    "with": {
                        "type": "tournament_engine::report_match_result",
                        "value": {
                            "phase_id": "phase",
                            "match_id": "match",
                            "decision": { "result": "Draw", "reason": null },
                            "values": [],
                            "parts": [],
                        },
                    },
                } },
            })
        );
    }

    #[test]
    fn an_undo_and_other_transitions_stay_as_they_are() {
        let undo = json!({ "id": "undo", "kind": { "Undo": "report" } });
        let add_entrant = json!({
            "id": "entrant",
            "kind": { "Apply": {
                "type": "tournament_engine::add_entrant",
                "value": { "entrant": { "id": "alice", "annotations": [] } },
            } },
        });

        assert_eq!(upgraded_entry(undo.clone()), undo);
        assert_eq!(upgraded_entry(add_entrant.clone()), add_entrant);
    }

    #[test]
    fn a_phase_added_without_a_match_format_gets_winner_only() {
        let add_phase = |plugins: Value| {
            json!({
                "id": "phase",
                "kind": { "Apply": {
                    "type": "tournament_engine::add_phase",
                    "value": { "id": "phase", "plugins": plugins },
                } },
            })
        };
        let round_robin = json!({ "type": "tournament_engine::round_robin", "value": null });
        let winner_only = json!({ "type": "tournament_engine::winner_only", "value": null });
        let scored =
            json!({ "type": "tournament_engine::scored", "value": { "winning_score": 21 } });

        assert_eq!(
            upgraded_entry(add_phase(json!({
                "format": round_robin,
                "ranking": [],
                "metrics": [],
            }))),
            add_phase(json!({
                "format": round_robin,
                "match_format": winner_only,
                "ranking": [],
                "metrics": [],
            }))
        );
        let with_match_format = add_phase(json!({
            "format": round_robin,
            "match_format": scored,
            "ranking": [],
            "metrics": [],
        }));
        assert_eq!(upgraded_entry(with_match_format.clone()), with_match_format);
    }

    #[test]
    fn a_match_holds_its_result_and_annotations_as_its_contest() {
        let semifinal =
            json!({ "type": "tournament_engine::knockout_round", "value": { "RoundOf": 4 } });
        let competition = competition_of(json!({
            "data": { "rankings": [], "byes": [], "seed": [], "matches": [
                { "id": "decided", "result": "BWins", "annotations": [semifinal] },
                { "id": "open", "result": null, "annotations": [] },
                { "id": "without annotations", "result": "Draw" },
            ] },
        }));

        let upgraded = upgraded_competition(competition);

        assert_eq!(
            upgraded["phases"][0]["data"]["matches"],
            json!([
                {
                    "id": "decided",
                    "contest": {
                        "decision": { "result": "BWins", "reason": null },
                        "values": [],
                        "parts": [],
                        "annotations": [semifinal],
                    },
                    "tallies": [],
                },
                {
                    "id": "open",
                    "contest": { "decision": null, "values": [], "parts": [], "annotations": [] },
                    "tallies": [],
                },
                {
                    "id": "without annotations",
                    "contest": {
                        "decision": { "result": "Draw", "reason": null },
                        "values": [],
                        "parts": [],
                        "annotations": [],
                    },
                    "tallies": [],
                },
            ])
        );
    }

    #[test]
    fn a_match_holding_its_contest_stays_as_it_is() {
        let contest = json!({
            "id": "decided",
            "contest": {
                "decision": { "result": "AWins", "reason": "retired" },
                "values": [],
                "parts": [],
                "annotations": [],
            },
            "tallies": [],
        });
        let competition = competition_of(json!({
            "data": { "rankings": [], "byes": [], "seed": [], "matches": [contest] },
        }));

        let upgraded = upgraded_competition(competition);

        assert_eq!(upgraded["phases"][0]["data"]["matches"][0], contest);
    }

    #[test]
    fn a_phase_without_a_match_format_gets_winner_only() {
        let scored =
            json!({ "type": "tournament_engine::scored", "value": { "winning_score": 21 } });

        let upgraded = upgraded_competition(competition_of(json!({})));
        let kept = upgraded_competition(competition_of(json!({ "match_format": scored })));

        assert_eq!(
            upgraded["phases"][0]["match_format"],
            json!({ "type": "tournament_engine::winner_only", "value": null })
        );
        assert_eq!(kept["phases"][0]["match_format"], scored);
    }

    #[test]
    fn a_file_of_the_first_format_comes_out_in_the_current_one() {
        let mut with_format = json!({ "format": 1, "log": [], "people": [] });
        let mut without = json!({ "log": [], "people": [] });

        upgrade(&mut with_format, Format::FIRST);
        upgrade(&mut without, Format::FIRST);

        assert_eq!(with_format["format"], json!(Format::CURRENT.0));
        assert_eq!(without["format"], json!(Format::CURRENT.0));
    }

    #[test]
    fn the_competition_of_a_file_is_converted_with_its_log() {
        let mut file = json!({ "log": [], "people": [], "competition": competition_of(json!({})) });

        upgrade(&mut file, Format::FIRST);

        assert_eq!(
            file["competition"]["phases"][0]["match_format"],
            json!({ "type": "tournament_engine::winner_only", "value": null })
        );
    }

    #[test]
    fn a_competition_of_the_current_format_is_not_converted() {
        let mut competition = competition_of(json!({}));
        let unchanged = competition.clone();

        upgrade_competition(&mut competition, Format::CURRENT);

        assert_eq!(competition, unchanged);
    }

    /// What a converter does not recognise is left for the typed read to refuse.
    #[test]
    fn json_of_an_unexpected_shape_stays_as_it_is() {
        let shapes = [
            json!(3),
            json!({ "log": 3, "people": [], "competition": "none" }),
            json!({ "log": [3, [], { "kind": 3 }, { "kind": { "Apply": 3 } }], "people": [] }),
            json!({ "log": [{ "kind": { "Apply": { "type": 3, "value": {} } } }], "people": [] }),
            json!({ "log": [{ "kind": { "Replace": { "with": [] } } }], "people": [] }),
            json!({ "log": [{ "kind": { "Apply": {
                "type": "tournament_engine::report_match_result",
                "value": "AWins",
            } } }], "people": [] }),
            json!({ "log": [{ "kind": { "Apply": {
                "type": "tournament_engine::add_phase",
                "value": { "plugins": [] },
            } } }], "people": [] }),
            json!({ "log": [], "people": [], "competition": { "phases": 3 } }),
            json!({ "log": [], "people": [], "competition": { "phases": [3, {
                "match_format": null,
                "data": 3,
            }] } }),
            json!({ "log": [], "people": [], "competition": { "phases": [{
                "match_format": null,
                "data": { "matches": ["a match", 3, null] },
            }] } }),
        ];

        for shape in shapes {
            let mut upgraded = shape.clone();
            upgrade(&mut upgraded, Format::FIRST);

            if let Some(upgraded) = upgraded.as_object_mut() {
                assert_eq!(upgraded.remove("format"), Some(json!(Format::CURRENT.0)));
            }
            assert_eq!(upgraded, shape);
        }
    }
}
