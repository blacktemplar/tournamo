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
    /// One of the people, as the JSON of a `Person`.
    person: fn(&mut Value),
}

/// `CONVERTERS[n]` converts format `n + 1` to format `n + 2`.
const CONVERTERS: &[Converter] = &[
    Converter {
        log_entry: v2::log_entry,
        competition: v2::competition,
        person: |_| {},
    },
    Converter {
        log_entry: v3::log_entry,
        competition: |_| {},
        person: |_| {},
    },
    Converter {
        log_entry: v4::log_entry,
        competition: v4::competition,
        person: |_| {},
    },
    Converter {
        log_entry: v5::log_entry,
        competition: v5::competition,
        person: v5::person,
    },
];

const _: () = assert!(
    CONVERTERS.len() as u32 == Format::CURRENT.0 - Format::FIRST.0,
    "every format after the first needs its converter"
);

/// Brings `file`, the JSON of a file of the format `from`, to [`Format::CURRENT`]: every entry of
/// its `"log"`, every one of its `"people"` and its `"competition"`, if any, and sets its
/// `"format"`.
pub(crate) fn upgrade(file: &mut Value, from: Format) {
    for converter in converters(from) {
        if let Some(Value::Array(log)) = file.get_mut("log") {
            log.iter_mut().for_each(converter.log_entry);
        }
        if let Some(Value::Array(people)) = file.get_mut("people") {
            people.iter_mut().for_each(converter.person);
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

/// Format 2 to 3: a transition is written as `{<variant>: <transition>}`, where it was
/// `{"type": <tag>, "value": <transition>}`.
mod v3 {
    use serde_json::{Map, Value};

    pub(super) fn log_entry(entry: &mut Value) {
        let transition = if entry.pointer("/kind/Apply").is_some() {
            "/kind/Apply"
        } else {
            "/kind/Replace/with"
        };
        let Some(Value::Object(transition)) = entry.pointer_mut(transition) else {
            return;
        };
        if transition.len() != 2 || !transition.contains_key("value") {
            return;
        }
        let variant = match transition.get("type").and_then(Value::as_str) {
            Some("tournament_engine::set_competition_seed") => "SetCompetitionSeed",
            Some("tournament_engine::add_entrant") => "AddEntrant",
            Some("tournament_engine::remove_entrant") => "RemoveEntrant",
            Some("tournament_engine::add_phase") => "AddPhase",
            Some("tournament_engine::start_competition") => "StartCompetition",
            Some("tournament_engine::report_match_result") => "ReportMatchResult",
            _ => return,
        };
        let value = transition.remove("value").unwrap_or_default();
        *transition = Map::from_iter([(variant.to_owned(), value)]);
    }
}

/// Format 3 to 4: a phase holds its plugins as one `"plugins"`, as a phase is added with them,
/// where it held its `"phase_format"`, `"match_format"`, `"ranker"` and `"metrics_calculators"`;
/// every pairing of a knockout's bracket holds the number of its match.
mod v4 {
    use serde_json::{Map, Value};

    /// The tags of the formats holding a bracket.
    const KNOCKOUTS: [&str; 2] = [
        "tournament_engine::single_elimination",
        "tournament_engine::double_elimination",
    ];

    /// An `AddPhase`: the bracket of its format, if any, is numbered. A phase is added before it
    /// is played, so it has none in practice.
    pub(super) fn log_entry(entry: &mut Value) {
        let transition = if entry.pointer("/kind/Apply").is_some() {
            "/kind/Apply/AddPhase/plugins/format"
        } else {
            "/kind/Replace/with/AddPhase/plugins/format"
        };
        if let Some(format) = entry.pointer_mut(transition) {
            number_bracket(format);
        }
    }

    pub(super) fn competition(competition: &mut Value) {
        let Some(Value::Array(phases)) = competition.get_mut("phases") else {
            return;
        };
        for phase in phases.iter_mut().filter_map(Value::as_object_mut) {
            plugins(phase);
            if let Some(format) = phase.get_mut("plugins").and_then(|p| p.get_mut("format")) {
                number_bracket(format);
            }
        }
    }

    /// A phase holding its plugins one by one holds them as its `"plugins"`.
    fn plugins(phase: &mut Map<String, Value>) {
        const RENAMED: [(&str, &str); 4] = [
            ("phase_format", "format"),
            ("match_format", "match_format"),
            ("ranker", "ranking"),
            ("metrics_calculators", "metrics"),
        ];
        if phase.contains_key("plugins") || !RENAMED.iter().all(|(old, _)| phase.contains_key(*old))
        {
            return;
        }
        let plugins = RENAMED
            .into_iter()
            .filter_map(|(old, new)| Some((new.to_owned(), phase.remove(old)?)))
            .collect();
        phase.insert("plugins".to_owned(), Value::Object(plugins));
    }

    /// The bracket of `format`, the JSON of a knockout format, gets the number of each pairing's
    /// match: the pairings that can be matches, i.e. whose sides a bye cannot take, are numbered
    /// by round, and within a round in the order they were added.
    fn number_bracket(format: &mut Value) {
        if !format
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|tag| KNOCKOUTS.contains(&tag))
        {
            return;
        }
        let Some(Value::Array(pairings)) = format.pointer_mut("/value/bracket/pairings") else {
            return;
        };
        if let Some(numbers) = numbers(pairings) {
            for (pairing, number) in pairings.iter_mut().zip(numbers) {
                if let Value::Object(pairing) = pairing {
                    pairing.insert("number".to_owned(), number.into());
                }
            }
        }
    }

    /// Who takes a side of a pairing that has no seat yet.
    #[derive(Clone, Copy)]
    enum Advance {
        WinnerOf(usize),
        LoserOf(usize),
    }

    /// The number of each of `pairings`, `None` for a pairing that is never a match; `None` if a
    /// pairing is not as expected or already numbered.
    fn numbers(pairings: &[Value]) -> Option<Vec<Option<u64>>> {
        let mut from: Vec<[Option<Advance>; 2]> = vec![[None; 2]; pairings.len()];
        let mut rounds = Vec::with_capacity(pairings.len());
        for (id, pairing) in pairings.iter().enumerate() {
            let pairing = pairing.as_object()?;
            if pairing.contains_key("number") {
                return None;
            }
            rounds.push(pairing.get("round")?.as_u64()?);
            for (to, advance) in [
                ("winner_to", Advance::WinnerOf(id)),
                ("loser_to", Advance::LoserOf(id)),
            ] {
                match pairing.get(to)? {
                    Value::Null => {}
                    slot => {
                        let to = usize::try_from(slot.get("pairing")?.as_u64()?).ok()?;
                        let side = match slot.get("side")?.as_str()? {
                            "A" => 0,
                            "B" => 1,
                            _ => return None,
                        };
                        // a pairing takes only the winners and losers of pairings before it
                        if to <= id {
                            return None;
                        }
                        *from.get_mut(to)?.get_mut(side)? = Some(advance);
                    }
                }
            }
        }
        // whether only a bye can take each side of each pairing
        let mut byes: Vec<[bool; 2]> = Vec::with_capacity(pairings.len());
        for (pairing, from) in pairings.iter().zip(&from) {
            let Value::Array(seats) = pairing.get("seats")? else {
                return None;
            };
            let [a, b] = &seats[..] else {
                return None;
            };
            let mut sides = [false; 2];
            for (side, seat) in [a, b].into_iter().enumerate() {
                sides[side] = match (seat, from[side]) {
                    (Value::Null, Some(Advance::LoserOf(before))) => byes[before].contains(&true),
                    (Value::Null, Some(Advance::WinnerOf(before))) => byes[before] == [true; 2],
                    (Value::Null, None) => false,
                    (seat, _) => seat == "Bye",
                };
            }
            byes.push(sides);
        }
        let mut matches: Vec<usize> = (0..pairings.len())
            .filter(|&id| byes[id] == [false; 2])
            .collect();
        // stable, so that the pairings of a round keep the order they were added in
        matches.sort_by_key(|&id| rounds[id]);
        let mut numbers = vec![None; pairings.len()];
        for (id, number) in matches.into_iter().zip(1..) {
            numbers[id] = Some(number);
        }
        Some(numbers)
    }
}

/// Format 4 to 5: a phase holds its divisions, each with its own copy of the phase's plugins,
/// once it has started: what a phase held as its `"plugins"` and `"data"` is its only division,
/// and the phase keeps the plugins, the `"seed"` of its data and whether it has `"started"`, as
/// the competition has. A phase has a `"name"` (none) and `"sources"` (the whole registration),
/// as an `AddPhase` has, and no `"columns"`, so that its tables of standings show what the rule
/// shows. A started competition holds its `"lots"`, the order its phase started in, as its
/// `"registration"` too, and every ranking its `"lot"`, its initial unique rank; the competition
/// holds the `"highest_number"` of a match or of a pairing of a bracket, a division the
/// `"number_base"` 0. The plugins of a phase, of a division and of an `AddPhase` keep the teams as
/// they come in (`"teams"`), are played in one division (`"divisions"`), dealt back and forth
/// (`"draw"`), seeded in order (`"seeding"`) and compared across divisions by nothing
/// (`"across"`). The points of a ranking, saved as a number, are its `"total"` and the points of
/// its matches (`"of_matches"`), its wins and draws as its division's points calculator scores
/// them. The start, saved as `null`, gives no `"order"`. A person has no `"rating"`.
mod v5 {
    use serde_json::{Map, Value, json};

    /// An `AddPhase`, applied or replacing another, gets no name, the whole registration as its
    /// source and no columns; a `StartCompetition` gets an empty order.
    pub(super) fn log_entry(entry: &mut Value) {
        let transition = if entry.pointer("/kind/Apply").is_some() {
            "/kind/Apply"
        } else {
            "/kind/Replace/with"
        };
        let Some(Value::Object(transition)) = entry.pointer_mut(transition) else {
            return;
        };
        if let Some(Value::Object(add_phase)) = transition.get_mut("AddPhase") {
            entry_of_phase(add_phase);
            if let Some(Value::Object(plugins)) = add_phase.get_mut("plugins") {
                one_division(plugins);
            }
        }
        if let Some(start @ Value::Null) = transition.get_mut("StartCompetition") {
            *start = json!({ "order": [] });
        }
    }

    /// Plugins without the fields of divisions keep their teams as they come in, are played in
    /// one division, dealt back and forth, seeded in order, compared across divisions by nothing.
    fn one_division(plugins: &mut Map<String, Value>) {
        if plugins.contains_key("divisions") {
            return;
        }
        plugins.insert(
            "teams".to_owned(),
            json!({ "type": "tournament_engine::keep", "value": null }),
        );
        plugins.insert("divisions".to_owned(), 1.into());
        plugins.insert(
            "draw".to_owned(),
            json!({ "type": "tournament_engine::snake", "value": null }),
        );
        plugins.insert(
            "seeding".to_owned(),
            json!({ "type": "tournament_engine::seeded_in_order", "value": null }),
        );
        plugins.insert("across".to_owned(), json!([]));
    }

    pub(super) fn person(person: &mut Value) {
        if let Some(person) = person.as_object_mut() {
            person.entry("rating").or_insert(Value::Null);
        }
    }

    pub(super) fn competition(competition: &mut Value) {
        let Some(started) = competition.get("started").and_then(Value::as_bool) else {
            return;
        };
        let Some(Value::Array(phases)) = competition.get_mut("phases") else {
            return;
        };
        let mut lots = None;
        let mut highest_number = 0;
        let mut converted = false;
        for phase in phases.iter_mut().filter_map(Value::as_object_mut) {
            if let Some(rankings) = phase
                .get_mut("data")
                .and_then(|data| data.get_mut("rankings"))
                .and_then(Value::as_array_mut)
            {
                lots = lots.or_else(|| started.then(|| lots_of(rankings)));
            }
            highest_number = highest_number.max(highest_number_of(phase));
            converted |= divisions(phase, started);
            if let Some(Value::Array(divisions)) = phase.get_mut("divisions") {
                divisions.iter_mut().for_each(split_points);
            }
        }
        if converted && let Some(competition) = competition.as_object_mut() {
            let lots = competition
                .entry("lots")
                .or_insert_with(|| lots.unwrap_or_default().into())
                .clone();
            competition.entry("registration").or_insert(lots);
            competition
                .entry("highest_number")
                .or_insert_with(|| highest_number.into());
        }
    }

    /// The entrants of `rankings` by initial unique rank, i.e. by lot; every ranking gets its
    /// initial unique rank as its lot.
    fn lots_of(rankings: &mut [Value]) -> Vec<Value> {
        let mut lots: Vec<(u64, Vec<Value>)> = Vec::new();
        for ranking in rankings.iter_mut().filter_map(Value::as_object_mut) {
            let Some(rank) = ranking.get("initial_unique_rank").cloned() else {
                continue;
            };
            if let Some(Value::Array(entrants)) = ranking.get("entrants") {
                lots.push((rank.as_u64().unwrap_or(u64::MAX), entrants.clone()));
            }
            ranking.entry("lot").or_insert(rank);
        }
        lots.sort_by_key(|(rank, _)| *rank);
        lots.into_iter()
            .flat_map(|(_, entrants)| entrants)
            .collect()
    }

    /// The highest number of a match of `phase`, or of a pairing of its bracket, 0 if it has
    /// none.
    fn highest_number_of(phase: &Map<String, Value>) -> u64 {
        fn numbers(of: Option<&Value>) -> impl Iterator<Item = u64> + '_ {
            of.and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|numbered| numbered.get("number").and_then(Value::as_u64))
        }
        let matches = phase.get("data").and_then(|data| data.get("matches"));
        let pairings = phase
            .get("plugins")
            .and_then(|plugins| plugins.pointer("/format/value/bracket/pairings"));
        numbers(matches).chain(numbers(pairings)).max().unwrap_or(0)
    }

    /// A phase holding its plugins and data holds them as its only division if `started`, else
    /// none. Returns whether it was converted.
    fn divisions(phase: &mut Map<String, Value>, started: bool) -> bool {
        let Some(seed) = phase.get("data").and_then(|data| data.get("seed")).cloned() else {
            return false;
        };
        if phase.contains_key("divisions") || !phase.contains_key("plugins") {
            return false;
        }
        let data = phase.remove("data").unwrap_or_default();
        if let Some(Value::Object(plugins)) = phase.get_mut("plugins") {
            one_division(plugins);
        }
        let divisions = if started {
            json!([{ "plugins": phase["plugins"].clone(), "data": data, "number_base": 0 }])
        } else {
            json!([])
        };
        entry_of_phase(phase);
        phase.insert("seed".to_owned(), seed);
        phase.insert("started".to_owned(), started.into());
        phase.insert("divisions".to_owned(), divisions);
        true
    }

    /// The points of every ranking of `division` are held as their total and the points of its
    /// matches, which the points calculator of the division gives for its wins and draws.
    fn split_points(division: &mut Value) {
        const POINTS: &str = "tournament_engine::points";
        fn of_type<'a>(values: &'a [Value], tag: &str) -> Option<&'a Value> {
            values
                .iter()
                .find(|value| value.get("type").and_then(Value::as_str) == Some(tag))
        }
        let Some(calculator) = division
            .pointer("/plugins/metrics")
            .and_then(Value::as_array)
            .and_then(|metrics| of_type(metrics, POINTS))
        else {
            return;
        };
        let worth = |result: &str| {
            calculator
                .pointer(&format!("/value/points_for_{result}"))
                .and_then(Value::as_f64)
        };
        let (Some(win), Some(draw)) = (worth("win"), worth("draw")) else {
            return;
        };
        let Some(Value::Array(rankings)) = division.pointer_mut("/data/rankings") else {
            return;
        };
        for metrics in rankings
            .iter_mut()
            .filter_map(|ranking| ranking.get_mut("metrics"))
            .filter_map(Value::as_array_mut)
        {
            let record = of_type(metrics, "tournament_engine::wins_draws_losses");
            let count = |of: &str| {
                record
                    .and_then(|record| record.pointer(&format!("/value/{of}")))
                    .and_then(Value::as_u64)
            };
            let (Some(wins), Some(draws)) = (count("wins"), count("draws")) else {
                continue;
            };
            let Some(points) = metrics
                .iter_mut()
                .find(|metric| metric.get("type").and_then(Value::as_str) == Some(POINTS))
                .and_then(|metric| metric.get_mut("value"))
            else {
                continue;
            };
            if let Some(total) = points.as_f64() {
                // as the engine adds them up
                let of_matches = win * wins as f64 + draw * draws as f64;
                *points = json!({ "total": total, "of_matches": of_matches });
            }
        }
    }

    /// A phase, or an `AddPhase`, without a name, sources and columns gets no name, the whole
    /// registration and no columns.
    fn entry_of_phase(phase: &mut Map<String, Value>) {
        if phase.contains_key("sources") || !phase.contains_key("plugins") {
            return;
        }
        phase.entry("name").or_insert(Value::Null);
        phase.insert(
            "sources".to_owned(),
            json!([{ "of": "Registration", "from": 1, "to": null }]),
        );
        phase.entry("columns").or_insert_with(|| json!([]));
    }
}

#[cfg(test)]
mod tests {
    use plugin_factory::{
        PluginType as _,
        config::{Path, PluginConfig},
    };
    use serde_json::json;
    use tournament_engine::{
        Competition, CompetitionStateMachine, MatchResult, PhasePlugins,
        plugins::{
            metrics_calculator::{MorePoints, PointsCalculator},
            phase_format::{DoubleElimination, DoubleEliminationV1, SingleElimination},
        },
    };
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        file::File,
        people::People,
        session::{Session, tests::Given},
    };

    /// `entry`, an entry of the log, as `log_entry`, the converter of one format, leaves it.
    fn converted_by(log_entry: fn(&mut Value), mut entry: Value) -> Value {
        log_entry(&mut entry);
        entry
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
            converted_by(v2::log_entry, report),
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
            converted_by(v2::log_entry, replace),
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

        assert_eq!(converted_by(v2::log_entry, undo.clone()), undo);
        assert_eq!(
            converted_by(v2::log_entry, add_entrant.clone()),
            add_entrant
        );
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
            converted_by(
                v2::log_entry,
                add_phase(json!({
                    "format": round_robin,
                    "ranking": [],
                    "metrics": [],
                }))
            ),
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
        assert_eq!(
            converted_by(v2::log_entry, with_match_format.clone()),
            with_match_format
        );
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
            upgraded["phases"][0]["divisions"][0]["data"]["matches"],
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

        assert_eq!(
            upgraded["phases"][0]["divisions"][0]["data"]["matches"][0],
            contest
        );
    }

    #[test]
    fn a_phase_without_a_match_format_gets_winner_only() {
        let scored =
            json!({ "type": "tournament_engine::scored", "value": { "winning_score": 21 } });

        let upgraded = upgraded_competition(competition_of(json!({})));
        let kept = upgraded_competition(competition_of(json!({ "match_format": scored })));

        assert_eq!(
            upgraded["phases"][0]["plugins"]["match_format"],
            json!({ "type": "tournament_engine::winner_only", "value": null })
        );
        assert_eq!(kept["phases"][0]["plugins"]["match_format"], scored);
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
            file["competition"]["phases"][0]["plugins"]["match_format"],
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

    /// A knockout of four entrants with both semifinals reported, and so its final scheduled.
    fn knockout_before_its_final() -> Session {
        let mut session = Session::new("cup");
        for name in ["Alice", "Bob", "Carol", "Dave"] {
            session.add_entrant(name).unwrap();
        }
        let draft = session.phase_draft(SingleElimination::ID).unwrap();
        session
            .add_phase(&crate::session::tests::form(draft.clone()))
            .unwrap();
        session.start().unwrap();
        for _ in 0..2 {
            let semifinal = session.playable(session.all())[0].id;
            session
                .report(session.first(), semifinal, MatchResult::AWins)
                .unwrap();
        }
        session
    }

    /// A double elimination of five entrants, so with byes in both brackets, with the first three
    /// matches reported: of its first version, the only one older formats knew.
    fn double_elimination_under_way() -> Session {
        let mut session = Session::new("cup");
        for name in ["Alice", "Bob", "Carol", "Dave", "Eve"] {
            session.add_entrant(name).unwrap();
        }
        let mut draft = session.phase_draft(DoubleElimination::ID).unwrap();
        let format = Path::ROOT
            .field(PhasePlugins::FORMAT)
            .field("grand_final_reset");
        let reset = draft.answers().get_at(&format).cloned().unwrap();
        draft
            .set(
                &Path::ROOT.field(PhasePlugins::FORMAT),
                PluginConfig::of::<DoubleEliminationV1>().with("grand_final_reset", reset),
            )
            .unwrap();
        session
            .add_phase(&crate::session::tests::form(draft.clone()))
            .unwrap();
        session.start().unwrap();
        for _ in 0..3 {
            let next = session.playable(session.all())[0].id;
            session
                .report(session.first(), next, MatchResult::BWins)
                .unwrap();
        }
        session
    }

    /// `plugins`, keeping their teams, played in one division, dealt in order, as plugins of format
    /// 4 are: without the fields of teams and divisions.
    fn without_divisions(plugins: &mut Value) {
        let plugins = plugins.as_object_mut().unwrap();
        assert_eq!(
            plugins.remove("teams"),
            Some(json!({ "type": "tournament_engine::keep", "value": null }))
        );
        assert_eq!(plugins.remove("divisions"), Some(json!(1)));
        assert_eq!(
            plugins.remove("draw"),
            Some(json!({ "type": "tournament_engine::snake", "value": null }))
        );
        assert_eq!(
            plugins.remove("seeding"),
            Some(json!({ "type": "tournament_engine::seeded_in_order", "value": null }))
        );
        assert_eq!(plugins.remove("across"), Some(json!([])));
    }

    /// `competition`, as this version writes it, as a file of format 4 holds it: a phase holds the
    /// plugins and data of its only division, or else its plugins and the data of a phase not
    /// started.
    fn as_of_format_4(competition: &mut Value) {
        let competition_of = competition.as_object_mut().unwrap();
        let lots = competition_of.remove("lots").unwrap();
        assert_eq!(competition_of.remove("registration"), Some(lots));
        competition_of.remove("highest_number").unwrap();
        for phase in competition["phases"].as_array_mut().unwrap() {
            let phase = phase.as_object_mut().unwrap();
            assert_eq!(phase.remove("name"), Some(Value::Null));
            assert_eq!(
                phase.remove("sources"),
                Some(json!([{ "of": "Registration", "from": 1, "to": null }]))
            );
            let seed = phase.remove("seed").unwrap();
            let started = phase.remove("started").unwrap();
            let Some(Value::Array(divisions)) = phase.remove("divisions") else {
                panic!("a phase holds its divisions");
            };
            assert_eq!(started, json!(!divisions.is_empty()));
            let data = match <[Value; 1]>::try_from(divisions) {
                Ok([mut division]) => {
                    assert_eq!(division["data"]["seed"], seed);
                    assert_eq!(division["number_base"], json!(0));
                    for ranking in division["data"]["rankings"].as_array_mut().unwrap() {
                        let ranking = ranking.as_object_mut().unwrap();
                        assert_eq!(
                            ranking.remove("lot"),
                            ranking.get("initial_unique_rank").cloned()
                        );
                        for metric in ranking["metrics"].as_array_mut().unwrap() {
                            if metric["type"] == "tournament_engine::points" {
                                metric["value"] = metric["value"]["total"].take();
                            }
                        }
                    }
                    phase.insert("plugins".to_owned(), division["plugins"].take());
                    division["data"].take()
                }
                Err(divisions) => {
                    assert!(divisions.is_empty(), "a phase of format 4 has one division");
                    json!({
                        "rankings": [],
                        "ranking_ids_ordered_by_unique_rank": [],
                        "matches": [],
                        "byes": [],
                        "seed": seed,
                    })
                }
            };
            phase.insert("data".to_owned(), data);
            without_divisions(&mut phase["plugins"]);
        }
    }

    /// `competition`, as this version writes it, as a file of format 3 holds it: its phases hold
    /// their plugins one by one, and the pairings of a bracket hold no number. Returns how many
    /// pairings had one.
    fn as_of_format_3(competition: &mut Value) -> usize {
        as_of_format_4(competition);
        let mut numbered = 0;
        for phase in competition["phases"].as_array_mut().unwrap() {
            let phase = phase.as_object_mut().unwrap();
            let Some(Value::Object(mut plugins)) = phase.remove("plugins") else {
                panic!("a phase holds its plugins");
            };
            let pairings = plugins["format"].pointer_mut("/value/bracket/pairings");
            for pairing in pairings.and_then(Value::as_array_mut).into_iter().flatten() {
                let number = pairing.as_object_mut().unwrap().remove("number");
                numbered += usize::from(number.unwrap().is_number());
            }
            for (old, new) in [
                ("phase_format", "format"),
                ("match_format", "match_format"),
                ("ranker", "ranking"),
                ("metrics_calculators", "metrics"),
            ] {
                phase.insert(old.to_owned(), plugins.remove(new).unwrap());
            }
            assert!(
                plugins.is_empty(),
                "a phase of format 3 has no other plugins"
            );
        }
        numbered
    }

    /// `file`, as this version writes it, as a file of the first format holds it: its transitions
    /// are written as their type and value, its reports name their result, its phases have no
    /// match format, and its matches hold their result and annotations. Panics if `file` holds
    /// anything the first format cannot.
    fn as_of_the_first_format(file: &mut Value) {
        let winner_only = json!({ "type": "tournament_engine::winner_only", "value": null });
        file["format"] = json!(1);
        as_of_format_3(&mut file["competition"]);
        let mut reports = 0;
        for entry in file["log"].as_array_mut().unwrap() {
            let Some(transition) = entry.pointer_mut("/kind/Apply") else {
                continue;
            };
            let tagged = transition.as_object().unwrap();
            assert_eq!(tagged.len(), 1, "a transition is its variant and value");
            let (variant, value) = tagged.iter().next().unwrap();
            let tag = match variant.as_str() {
                "SetCompetitionSeed" => "tournament_engine::set_competition_seed",
                "AddEntrant" => "tournament_engine::add_entrant",
                "RemoveEntrant" => "tournament_engine::remove_entrant",
                "AddPhase" => "tournament_engine::add_phase",
                "StartCompetition" => "tournament_engine::start_competition",
                "ReportMatchResult" => "tournament_engine::report_match_result",
                variant => panic!("format 2 has no transition {variant}"),
            };
            *transition = json!({ "type": tag, "value": value.clone() });
            match transition["type"].as_str().unwrap() {
                "tournament_engine::report_match_result" => {
                    let report = transition["value"].as_object_mut().unwrap();
                    let decision = report.remove("decision").unwrap();
                    assert_eq!(decision["reason"], Value::Null);
                    assert_eq!(report.remove("values"), Some(json!([])));
                    assert_eq!(report.remove("parts"), Some(json!([])));
                    report.insert("match_result".to_owned(), decision["result"].clone());
                    reports += 1;
                }
                "tournament_engine::add_phase" => {
                    let plugins = transition["value"]["plugins"].as_object_mut().unwrap();
                    assert_eq!(plugins.remove("match_format"), Some(winner_only.clone()));
                }
                _ => {}
            }
        }
        let mut results = Vec::new();
        for phase in file["competition"]["phases"].as_array_mut().unwrap() {
            let phase = phase.as_object_mut().unwrap();
            assert_eq!(phase.remove("match_format"), Some(winner_only.clone()));
            for r#match in phase["data"]["matches"].as_array_mut().unwrap() {
                let r#match = r#match.as_object_mut().unwrap();
                assert_eq!(r#match.remove("tallies"), Some(json!([])));
                let contest = r#match.remove("contest").unwrap();
                assert_eq!([&contest["values"], &contest["parts"]], [&json!([]); 2]);
                let result = match &contest["decision"] {
                    Value::Null => Value::Null,
                    decision => {
                        assert_eq!(decision["reason"], Value::Null);
                        decision["result"].clone()
                    }
                };
                results.push(result.clone());
                r#match.insert("result".to_owned(), result);
                r#match.insert("annotations".to_owned(), contest["annotations"].clone());
            }
        }
        assert!(reports > 0, "the log reports results");
        assert!(results.contains(&Value::Null) && results.iter().any(Value::is_string));
    }

    /// A file of the first format is read into the competition it holds, both by replaying its log
    /// and as the competition it holds next to the log.
    #[test]
    fn a_file_of_the_first_format_is_read_into_the_competition_it_holds() {
        let session = knockout_before_its_final();
        let mut file: Value = serde_json::from_str(&session.export()).unwrap();
        as_of_the_first_format(&mut file);

        let read = File::<CompetitionStateMachine, People>::read(&file.to_string()).unwrap();
        let mut held = file["competition"].take();
        upgrade_competition(&mut held, Format::FIRST);
        let held: Competition = serde_json::from_value(held).unwrap();

        let competition = serde_json::to_value(session.competition()).unwrap();
        assert_eq!(
            serde_json::to_value(read.log.current()).unwrap(),
            competition
        );
        assert_eq!(serde_json::to_value(held).unwrap(), competition);
    }

    /// A competition of format 3 holding a bracket with byes in both of its brackets is read into
    /// the competition it was, its pairings numbered as the bracket numbers them.
    #[test]
    fn a_competition_of_format_3_is_read_with_its_plugins_and_its_bracket_numbered() {
        let session = double_elimination_under_way();
        let competition = serde_json::to_value(session.competition()).unwrap();
        let mut held = competition.clone();
        let numbered = as_of_format_3(&mut held);

        upgrade_competition(&mut held, Format(3));

        assert!(numbered > 0, "the bracket numbers its pairings");
        // the phase keeps its plugins with the state of the bracket until it is read
        assert_eq!(
            held["phases"][0]["divisions"],
            competition["phases"][0]["divisions"]
        );
        let read: Competition = serde_json::from_value(held).unwrap();
        assert_eq!(serde_json::to_value(read).unwrap(), competition);
    }

    /// A started competition of format 4 is read into the competition it was: its phase holds what
    /// it held as its only division, and keeps its plugins without the state of the bracket.
    #[test]
    fn a_started_competition_of_format_4_is_read_with_its_phase_as_one_division() {
        let session = knockout_before_its_final();
        let competition = serde_json::to_value(session.competition()).unwrap();
        let mut held = competition.clone();
        as_of_format_4(&mut held);
        let bracket = "/phases/0/plugins/format/value/bracket";
        assert!(
            held.pointer(bracket)
                .is_some_and(|bracket| !bracket.is_null())
        );

        upgrade_competition(&mut held, Format(4));

        assert_eq!(
            held["phases"][0]["divisions"],
            competition["phases"][0]["divisions"]
        );
        let read: Competition = serde_json::from_value(held).unwrap();
        assert_eq!(serde_json::to_value(read).unwrap(), competition);
    }

    /// The points of a ranking of format 4, a number, are read as their total and the points of
    /// its matches.
    #[test]
    fn the_points_of_format_4_are_read_with_those_of_the_matches() {
        let mut session = Given::new()
            .entrants(&["Alice", "Bob", "Carol"])
            .ranking(vec![PluginConfig::of::<MorePoints>()])
            .metrics(vec![
                PluginConfig::of::<PointsCalculator>()
                    .with("points_for_win", 3.0)
                    .with("points_for_draw", 1.0)
                    .with("points_for_bye", 2.0),
            ])
            .started();
        for result in [MatchResult::AWins, MatchResult::Draw] {
            let r#match = session.playable(session.all())[0].id;
            session.report(session.first(), r#match, result).unwrap();
        }
        let competition = serde_json::to_value(session.competition()).unwrap();
        let mut held = competition.clone();
        as_of_format_4(&mut held);
        let points = |competition: &Value| {
            let rankings = competition["phases"][0]["divisions"][0]["data"]["rankings"]
                .as_array()
                .unwrap();
            rankings
                .iter()
                .flat_map(|ranking| ranking["metrics"].as_array().unwrap())
                .filter(|metric| metric["type"] == "tournament_engine::points")
                .map(|metric| metric["value"].clone())
                .collect::<Vec<_>>()
        };
        // a bye was reached, so the points of some ranking are more than those of its matches
        assert!(
            points(&competition)
                .iter()
                .any(|points| points["total"] != points["of_matches"])
        );

        upgrade_competition(&mut held, Format(4));

        assert_eq!(points(&held), points(&competition));
        let read: Competition = serde_json::from_value(held).unwrap();
        assert_eq!(serde_json::to_value(read).unwrap(), competition);
    }

    /// A competition of format 4 that has not started is read with its phase holding no division.
    #[test]
    fn a_competition_of_format_4_not_started_is_read_with_its_phase_without_divisions() {
        let mut session = Session::new("cup");
        for name in ["Alice", "Bob"] {
            session.add_entrant(name).unwrap();
        }
        let draft = session.phase_draft(SingleElimination::ID).unwrap();
        session
            .add_phase(&crate::session::tests::form(draft.clone()))
            .unwrap();
        let competition = serde_json::to_value(session.competition()).unwrap();
        let mut held = competition.clone();
        as_of_format_4(&mut held);

        upgrade_competition(&mut held, Format(4));

        assert_eq!(held, competition);
        assert_eq!(held["phases"][0]["divisions"], json!([]));
        let read: Competition = serde_json::from_value(held).unwrap();
        assert_eq!(serde_json::to_value(read).unwrap(), competition);
    }

    /// A phase added, or added in place of another, takes the whole registration, without a name,
    /// shows what the rule shows, and is played in one division.
    #[test]
    fn a_phase_added_gets_no_name_the_whole_registration_no_columns_and_one_division_in_format_5() {
        let round_robin = json!({ "type": "tournament_engine::round_robin", "value": null });
        let added = json!({ "id": "phase", "plugins": { "format": round_robin } });
        let converted = json!({
            "id": "phase",
            "name": null,
            "sources": [{ "of": "Registration", "from": 1, "to": null }],
            "columns": [],
            "plugins": {
                "format": round_robin,
                "teams": { "type": "tournament_engine::keep", "value": null },
                "divisions": 1,
                "draw": { "type": "tournament_engine::snake", "value": null },
                "seeding": { "type": "tournament_engine::seeded_in_order", "value": null },
                "across": [],
            },
        });
        let apply =
            |phase: &Value| json!({ "id": "add", "kind": { "Apply": { "AddPhase": phase } } });
        let replace = |phase: &Value| {
            json!({ "id": "edit", "kind": { "Replace": {
                "target": "add",
                "with": { "AddPhase": phase },
            } } })
        };

        assert_eq!(
            converted_by(v5::log_entry, apply(&added)),
            apply(&converted)
        );
        assert_eq!(
            converted_by(v5::log_entry, replace(&added)),
            replace(&converted)
        );
        // a phase with sources already stays as it is
        assert_eq!(
            converted_by(v5::log_entry, apply(&converted)),
            apply(&converted)
        );
    }

    /// A started competition of format 4 gets its lots in the order its phase started in, as its
    /// registration too, and the highest number of a match or a pairing of a bracket.
    #[test]
    fn a_started_competition_of_format_4_gets_its_lots_and_highest_number() {
        let session = double_elimination_under_way();
        let competition = serde_json::to_value(session.competition()).unwrap();
        let mut held = competition.clone();
        as_of_format_4(&mut held);

        upgrade_competition(&mut held, Format(4));

        assert_eq!(held["lots"], competition["lots"]);
        assert_eq!(held["registration"], competition["lots"]);
        assert_eq!(held["highest_number"], competition["highest_number"]);
        let matches = competition["phases"][0]["divisions"][0]["data"]["matches"]
            .as_array()
            .unwrap();
        // the bracket keeps numbers for matches not scheduled yet
        assert!(
            held["highest_number"].as_u64().unwrap()
                > matches
                    .iter()
                    .map(|m| m["number"].as_u64().unwrap())
                    .max()
                    .unwrap()
        );
    }

    /// The start of format 4, which gave no order, gives an empty one; one with an order stays.
    #[test]
    fn the_start_gets_an_empty_order_in_format_5() {
        let apply = |start: Value| json!({ "id": "start", "kind": { "Apply": { "StartCompetition": start } } });
        let ordered = json!({ "order": [["alice"]] });

        assert_eq!(
            converted_by(v5::log_entry, apply(Value::Null)),
            apply(json!({ "order": [] }))
        );
        assert_eq!(
            converted_by(v5::log_entry, apply(ordered.clone())),
            apply(ordered)
        );
    }

    /// A person of format 4 has no rating; one rated already keeps it.
    #[test]
    fn a_person_gets_no_rating_in_format_5() {
        let converted = |mut person: Value| {
            v5::person(&mut person);
            person
        };

        assert_eq!(
            converted(json!({ "id": "alice", "name": "Alice" })),
            json!({ "id": "alice", "name": "Alice", "rating": null })
        );
        let rated = json!({ "id": "alice", "name": "Alice", "rating": 1500 });
        assert_eq!(converted(rated.clone()), rated);
        assert_eq!(converted(json!("damaged")), json!("damaged"));
    }

    /// A phase of an unexpected shape, or holding its divisions already, stays as it is.
    #[test]
    fn a_phase_of_an_unexpected_shape_stays_as_it_is_in_format_5() {
        let plugins =
            json!({ "format": { "type": "tournament_engine::round_robin", "value": null } });
        let shapes = [
            json!({ "phases": [{ "plugins": plugins, "data": {} }], "started": true }),
            json!({ "phases": [{ "plugins": plugins, "data": { "seed": [] } }] }),
            json!({ "phases": [{ "data": { "seed": [] } }], "started": true }),
            json!({ "phases": [{
                "plugins": plugins,
                "data": { "seed": [] },
                "divisions": [],
            }], "started": false }),
            json!({ "phases": [3], "started": true }),
            // points without the wins, draws and losses they were scored for
            json!({ "phases": [{ "divisions": [{
                "plugins": { "metrics": [{
                    "type": "tournament_engine::points",
                    "value": { "points_for_win": 1.0, "points_for_draw": 0.5 },
                }] },
                "data": { "rankings": [{ "metrics": [{
                    "type": "tournament_engine::points",
                    "value": 3.0,
                }] }] },
            }] }], "started": true }),
        ];

        for shape in shapes {
            let mut upgraded = shape.clone();
            upgrade_competition(&mut upgraded, Format(4));

            assert_eq!(upgraded, shape);
        }
    }

    /// A bracket of an unexpected shape, or already numbered, stays as it is.
    #[test]
    fn a_bracket_of_an_unexpected_shape_stays_as_it_is_in_format_4() {
        let knockout = |bracket: Value| {
            json!({ "phases": [{ "plugins": { "format": {
                "type": "tournament_engine::single_elimination",
                "value": { "third_place_match": false, "bracket": bracket },
            } } }] })
        };
        let pairing = |seats: Value, winner_to: Value| {
            json!({
                "round": 1,
                "annotations": [],
                "seats": seats,
                "match": null,
                "winner_to": winner_to,
                "loser_to": null,
                "advances_only_if_won_by": null,
            })
        };
        let seated = json!([{ "Ranking": "a" }, { "Ranking": "b" }]);
        let mut numbered = pairing(seated.clone(), Value::Null);
        numbered["number"] = json!(1);
        let shapes = [
            knockout(Value::Null),
            knockout(json!({ "pairings": 3, "in_play": [] })),
            knockout(json!({ "pairings": [3], "in_play": [] })),
            knockout(json!({ "pairings": [numbered], "in_play": [] })),
            knockout(json!({ "pairings": [pairing(json!([null]), Value::Null)], "in_play": [] })),
            knockout(json!({ "pairings": [pairing(
                seated.clone(),
                json!({ "pairing": 0, "side": "A" }),
            )], "in_play": [] })),
            knockout(json!({ "pairings": [pairing(
                seated,
                json!({ "pairing": 7, "side": "A" }),
            )], "in_play": [] })),
            json!({ "phases": [{ "plugins": { "format": {
                "type": "a format of another crate",
                "value": { "bracket": { "pairings": [pairing(json!([]), Value::Null)] } },
            } } }] }),
        ];

        for shape in shapes {
            let mut upgraded = shape.clone();
            upgrade_competition(&mut upgraded, Format(3));

            assert_eq!(upgraded, shape);
        }
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
            json!({ "log": [{ "kind": { "Apply": { "type": "a tag" } } }], "people": [] }),
            json!({ "log": [{ "kind": { "Apply": { "type": "a tag", "value": null } } }], "people": [] }),
            json!({ "log": [{ "kind": { "Apply": {
                "type": "a tag",
                "value": null,
                "more": null,
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

    /// A transition of format 1 whose value is of an unexpected shape is left for the typed read to
    /// refuse.
    #[test]
    fn a_transition_of_an_unexpected_shape_stays_as_it_is_in_format_2() {
        for transition in [
            json!({ "type": "tournament_engine::report_match_result", "value": "AWins" }),
            json!({ "type": "tournament_engine::add_phase", "value": { "plugins": [] } }),
        ] {
            let entry = json!({ "id": "entry", "kind": { "Apply": transition } });

            assert_eq!(converted_by(v2::log_entry, entry.clone()), entry);
        }
    }

    #[test]
    fn a_transition_is_written_as_its_tag_and_value_in_format_3() {
        let add_entrant =
            |transition: Value| json!({ "id": "entrant", "kind": { "Apply": transition } });
        let replace = |with: Value| {
            json!({
                "id": "replace",
                "kind": { "Replace": { "target": "phase", "with": with } },
            })
        };
        let entrant = json!({ "entrant": { "id": "alice", "annotations": [] } });
        let start = json!({ "type": "tournament_engine::start_competition", "value": null });

        assert_eq!(
            converted_by(
                v3::log_entry,
                add_entrant(json!({ "type": "tournament_engine::add_entrant", "value": entrant }))
            ),
            add_entrant(json!({ "AddEntrant": entrant }))
        );
        assert_eq!(
            converted_by(v3::log_entry, replace(start)),
            replace(json!({ "StartCompetition": null }))
        );
        let undo = json!({ "id": "undo", "kind": { "Undo": "entrant" } });
        assert_eq!(converted_by(v3::log_entry, undo.clone()), undo);
    }
}
