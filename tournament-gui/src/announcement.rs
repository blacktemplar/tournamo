//! What the operator is told was done: an entrant added or removed, the phase added or changed,
//! the start, a result reported or reopened, and what undo and redo walked through. It is shown in
//! a live region, so that a screen reader says it too.

use tournament_app::{
    i18n::Translator,
    view::{Action, EntrantRow, Reopened, Reported},
};

use crate::{capitalized, i18n::t, result_text, team_name, winners_text};

/// Something done that the operator is told about, kept as data so that it is said again in
/// another language when that is chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Announcement {
    AddedEntrant(EntrantRow),
    RemovedEntrant(EntrantRow),
    AddedPhase,
    EditedPhase,
    /// The start, with the number of matches it scheduled.
    Started(usize),
    Reported(Reported),
    Reopened(Reopened),
    Undone(Action),
    Redone(Action),
}

impl Announcement {
    /// What it says in the language of `tr`.
    pub fn text(&self, tr: &Translator) -> String {
        let entrant = |entrant: &EntrantRow| team_name(tr, std::slice::from_ref(entrant));
        match self {
            Self::AddedEntrant(added) => {
                t!(tr, "announce-added-entrant", name = entrant(added))
            }
            Self::RemovedEntrant(removed) => {
                t!(tr, "announce-removed-entrant", name = entrant(removed))
            }
            Self::AddedPhase => tr.get("announce-added-phase"),
            Self::EditedPhase => tr.get("announce-edited-phase"),
            Self::Started(matches) => t!(tr, "announce-started", count = *matches),
            Self::Reported(reported) => {
                let row = &reported.row;
                let mut sentences = vec![t!(
                    tr,
                    "announce-reported",
                    number = row.number,
                    result = result_text(tr, row).unwrap_or_default(),
                )];
                if reported.new_matches > 0 {
                    sentences.push(t!(tr, "announce-new-matches", count = reported.new_matches));
                }
                if let Some(winners) = &reported.winners {
                    sentences.push(tr.get("announce-all-finished"));
                    let names: Vec<Vec<Option<String>>> = winners
                        .iter()
                        .map(|team| team.iter().map(|entrant| entrant.name.clone()).collect())
                        .collect();
                    sentences.push(t!(
                        tr,
                        "announce-winners",
                        winners = capitalized(&winners_text(tr, &names)),
                    ));
                }
                sentences.join(" ")
            }
            Self::Reopened(reopened) => {
                let mut sentences = vec![t!(tr, "announce-reopened", number = reopened.row.number)];
                if reopened.unscheduled > 0 {
                    sentences.push(t!(tr, "announce-unscheduled", count = reopened.unscheduled));
                }
                sentences.join(" ")
            }
            Self::Undone(action) => t!(tr, "announce-undone", action = action_text(tr, action)),
            Self::Redone(action) => t!(tr, "announce-redone", action = action_text(tr, action)),
        }
    }
}

/// What `action` was, as the object of a sentence, e.g. "adding Alice".
fn action_text(tr: &Translator, action: &Action) -> String {
    match action {
        Action::AddEntrant(entrant) => t!(
            tr,
            "action-add-entrant",
            name = team_name(tr, std::slice::from_ref(entrant)),
        ),
        Action::RemoveEntrant(entrant) => t!(
            tr,
            "action-remove-entrant",
            name = team_name(tr, std::slice::from_ref(entrant)),
        ),
        Action::AddPhase => tr.get("action-add-phase"),
        Action::EditPhase => tr.get("action-edit-phase"),
        Action::Start => tr.get("action-start"),
        Action::Report(row) => t!(
            tr,
            "action-report",
            number = row.number,
            result = result_text(tr, row).unwrap_or_default(),
        ),
        Action::Reopen(row) => t!(tr, "action-reopen", number = row.number),
    }
}
