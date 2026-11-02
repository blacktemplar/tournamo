//! The cross table of a phase whose teams stay the same, e.g. a round robin: a row and a column
//! per team in the order of the standings, each cell the matches of the row's team against the
//! column's, seen from the row, and last what the row's results add up to. It only shows them;
//! results are reported in the list of matches.

use dioxus::prelude::*;
use tournament_app::view::{CrossMatch, Outcome};

use crate::{competition::Ctx, team_name};

/// The cross table, if the phase has one.
#[component]
pub fn CrossTable() -> Element {
    let ctx = use_context::<Ctx>();
    let Some(table) = ctx.session.read().cross_table() else {
        return rsx! {};
    };
    let tr = ctx.translator.read();
    let names: Vec<String> = table
        .rows
        .iter()
        .map(|row| team_name(&tr, &row.entrants))
        .collect();
    rsx! {
        section { class: "card", aria_labelledby: "cross-table",
            h2 { id: "cross-table", {tr.get("cross-table")} }
            // scrolls sideways if its columns do not fit, the names staying in sight
            div {
                class: "scroll",
                role: "group",
                tabindex: "0",
                aria_labelledby: "cross-table",
                table { id: "cross-table-grid", class: "cross-table",
                    thead {
                        tr {
                            th { class: "name", scope: "col", {tr.get("name")} }
                            // the number of the row, its name said to screen readers
                            for (index , name) in names.iter().enumerate() {
                                th { scope: "col",
                                    span { aria_hidden: "true", "{index + 1}" }
                                    span { class: "visually-hidden", "{name}" }
                                }
                            }
                            th { class: "total", scope: "col", {tr.get("cross-table-total")} }
                        }
                    }
                    tbody {
                        for (i , row) in table.rows.into_iter().enumerate() {
                            tr { key: "{i}",
                                th { class: "name", scope: "row",
                                    span { class: "number", aria_hidden: "true", "{i + 1}" }
                                    " "
                                    "{names[i]}"
                                }
                                for (j , cell) in row.cells.into_iter().enumerate() {
                                    if i == j {
                                        td { key: "{j}", class: "self" }
                                    } else {
                                        td { key: "{j}",
                                            for played in cell {
                                                Cell { key: "{played.row.id:?}", played }
                                            }
                                        }
                                    }
                                }
                                td { class: "total", {tr.number(row.total.get())} }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A match in a cell: how it ended for the row's team, nothing while it has no result.
#[component]
fn Cell(played: CrossMatch) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    // tinted by how it ended, which the number says as well
    let (shown, said, class) = match played.outcome {
        Some(Outcome::Win) => ("1", "outcome-win", "win"),
        Some(Outcome::Draw) => ("½", "outcome-draw", "draw"),
        Some(Outcome::Loss) => ("0", "outcome-loss", "loss"),
        None => return rsx! {},
    };
    rsx! {
        span { class: "outcome {class}",
            span { aria_hidden: "true", "{shown}" }
            span { class: "visually-hidden", {tr.get(said)} }
        }
    }
}
