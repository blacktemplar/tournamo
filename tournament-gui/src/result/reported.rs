//! The result of a finished match as it was reported, read-only.

use dioxus::prelude::*;
use tournament_app::{
    i18n::PluginText,
    result_form::{PartView, ResultView},
    view::MatchRow,
};
use tournament_engine::{EnteredValue, MatchResult, plugins::Phrase};

use super::with_parts_won;
use crate::{
    capitalized,
    competition::Ctx,
    components::dialog::{Dialog, DialogDescription, DialogTitle},
    focus::focus_later,
    i18n::t,
    team_name,
};

/// The button showing the result of the finished match `row` as it was reported, and the dialog
/// showing it, read-only: a row of each of its parts, e.g. its games, with their values and who
/// won them, then the match's own values and result. The button has no text: it covers the
/// nearest positioned element around it, the match's card or pairing, so that a click anywhere on
/// it opens the dialog, and is named for screen readers. The stylesheet underlines the score
/// shown with it, as a link, which tells touch, without hover, that the match opens.
#[component]
pub fn ResultDetails(row: MatchRow) -> Element {
    let ctx = use_context::<Ctx>();
    // the result while the dialog is open
    let mut shown = use_signal(|| None::<ResultView>);
    let tr = ctx.translator.read();
    let id = row.id;
    let (a, b) = (team_name(&tr, &row.a), team_name(&tr, &row.b));
    rsx! {
        button {
            class: "match-details",
            r#type: "button",
            aria_label: t!(tr, "result-details-of", number = row.number),
            onclick: move |_| {
                shown.set(ctx.session.peek().reported_result(id));
                focus_later(&["#result-details button"]);
            },
        }
        Dialog {
            open: Some(shown.read().is_some()),
            on_open_change: move |open: bool| {
                if !open {
                    shown.set(None);
                }
            },
            DialogTitle { {t!(tr, "result-title", number = row.number)} }
            DialogDescription {
                {t!(tr, "result-details-description", a = a.as_str(), b = b.as_str())}
            }
            if let Some(view) = shown() {
                div { id: "result-details", class: "dialog-form",
                    ReportedPart {
                        part: view.root,
                        match_format: view.match_format,
                        a: a.clone(),
                        b: b.clone(),
                    }
                    div { class: "dialog-buttons",
                        button { r#type: "button", onclick: move |_| shown.set(None),
                            {tr.get("close")}
                        }
                    }
                }
            }
        }
    }
}

/// The part `part` of a finished match between `a` and `b` as it was reported: a table of its
/// parts, each with the values of each side and who won it, then its own values and its result.
/// Parts of its parts are not shown, as no match format has them yet.
#[component]
fn ReportedPart(part: PartView, match_format: &'static str, a: String, b: String) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let level_name = |phrase: &Phrase| {
        capitalized(&tr.plugin_phrase(match_format, PluginText::Level(phrase.key()), phrase))
    };
    let field_label = |label: &Phrase| {
        capitalized(&tr.plugin_phrase(match_format, PluginText::EnteredValue(label.key()), label))
    };
    let said = |result: Option<MatchResult>| match result {
        Some(MatchResult::AWins) => t!(tr, "wins", team = a.as_str()),
        Some(MatchResult::BWins) => t!(tr, "wins", team = b.as_str()),
        Some(MatchResult::Draw) => tr.get("draw"),
        None => tr.get("result-open"),
    };
    let value = |value: Option<EnteredValue>| match value {
        Some(EnteredValue::Count(count)) => tr.number(count.into()),
        Some(EnteredValue::Decimal(decimal)) => tr.number(decimal.get()),
        None => "–".to_owned(),
    };
    let parts = part.parts_table();
    // each side's value in a column of its own, named by the side, and by the field if there are
    // several; without values, who won a part is said in a column of its own
    let (heading, headers, result_column) = match &parts {
        Some(table) => {
            let several = table.fields.len() > 1;
            let column = |label: &Phrase, name: &str| {
                if several {
                    format!("{} {name}", field_label(label))
                } else {
                    name.to_owned()
                }
            };
            let headers: Vec<String> = table
                .fields
                .iter()
                .flat_map(|label| [column(label, &a), column(label, &b)])
                .collect();
            (level_name(&table.name), headers, table.fields.is_empty())
        }
        None => (String::new(), Vec::new(), false),
    };
    // each part's name, its values in the columns' order, each marked if it is of the winner, who
    // won it and its warnings
    let rows: Vec<ReportedRow> = parts
        .iter()
        .flat_map(|table| {
            table.rows.iter().map(|row| {
                let name = t!(
                    tr,
                    "part-numbered",
                    name = level_name(&table.name),
                    number = row.number
                );
                let won = |result| row.result == Some(result);
                let values = row
                    .values
                    .iter()
                    .flat_map(|values| {
                        [
                            (value(values.a), won(MatchResult::AWins)),
                            (value(values.b), won(MatchResult::BWins)),
                        ]
                    })
                    .collect();
                let warnings = row
                    .warnings
                    .iter()
                    .map(|warning| {
                        t!(
                            tr,
                            "part-warning",
                            part = name.as_str(),
                            warning = tr.sentence(warning)
                        )
                    })
                    .collect();
                ReportedRow {
                    result: said(row.result),
                    name,
                    values,
                    warnings,
                }
            })
        })
        .collect();
    let warnings: Vec<String> = rows
        .iter()
        .flat_map(|row| row.warnings.clone())
        .chain(part.warnings.iter().map(|warning| tr.sentence(warning)))
        .collect();
    let name = level_name(&part.name);
    let outcome = with_parts_won(&tr, said(part.result), part.parts_won);
    rsx! {
        if parts.is_some() {
            table { class: "reported-parts",
                thead {
                    tr {
                        th { scope: "col", {heading} }
                        for header in headers {
                            th { scope: "col", class: "value", {header} }
                        }
                        if result_column {
                            th { scope: "col", {tr.get("result-column")} }
                        }
                    }
                }
                tbody {
                    for ReportedRow { name, result, values, .. } in rows {
                        tr {
                            th { scope: "row",
                                {name}
                                // the winner's values are marked, which screen readers may not
                                // read
                                if !result_column {
                                    span { class: "visually-hidden", ", {result}" }
                                }
                            }
                            for (value, won) in values {
                                td { class: if won { "value won" } else { "value" }, {value} }
                            }
                            if result_column {
                                td { {result} }
                            }
                        }
                    }
                }
            }
        }
        for field in &part.fields {
            p { class: "reported-field",
                {
                    t!(
                        tr, "reported-value", value = field_label(&field.label), a = a.as_str(), b =
                        b.as_str(), score = tr.scores(&[field.values])
                    )
                }
            }
        }
        p { class: "reported-outcome",
            {t!(tr, "result-is", part = name.as_str(), result = outcome.as_str())}
        }
        for warning in warnings {
            p { class: "warnings", {warning} }
        }
    }
}

/// A part of a finished match as a row of [`ReportedPart`].
struct ReportedRow {
    /// e.g. Game 2
    name: String,
    /// who won it
    result: String,
    /// in the order of the columns, each marked if it is of the winner
    values: Vec<(String, bool)>,
    warnings: Vec<String>,
}
