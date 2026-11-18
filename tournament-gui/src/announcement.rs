//! What the operator is told was done: an entrant added, removed or renamed, the phase added or
//! changed, the start, a result reported or reopened, and what undo and redo walked through. It is
//! shown in a live region, so that a screen reader says it too.

use tournament_app::{
    i18n::Translator,
    view::{Action, EntrantRow, PhaseName, Reopened, Reported},
};

use crate::{capitalized, i18n::t, result_text, result_with_scores, team_name, winners_text};

/// Something done that the operator is told about, kept as data so that it is said again in
/// another language when that is chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum Announcement {
    AddedEntrant(EntrantRow),
    /// Entrants added at once, e.g. from a list pasted, by their number.
    AddedEntrants(usize),
    RemovedEntrant(EntrantRow),
    /// An entrant renamed, as it was and as it is.
    RenamedEntrant(EntrantRow, EntrantRow),
    AddedPhase,
    /// The phases of a preset added, as many as there are.
    AddedPhases(usize),
    /// The phase changed, with the number of matches that have a result after the change.
    EditedPhase(usize),
    /// The start, with the number of matches it scheduled.
    Started(usize),
    /// A phase removed.
    RemovedPhase,
    /// A phase started on its own, as it is called, with the number of matches it scheduled.
    StartedPhase(Option<PhaseName>, usize),
    Reported(Reported),
    Reopened(Reopened),
    Undone(Action),
    Redone(Action),
    /// A table copied as text.
    Copied,
    /// A competition imported, under the name it was given.
    Imported(String),
}

impl Announcement {
    /// What it says in the language of `tr`.
    pub fn text(&self, tr: &Translator) -> String {
        let entrant = |entrant: &EntrantRow| team_name(tr, std::slice::from_ref(entrant));
        match self {
            Self::AddedEntrant(added) => {
                t!(tr, "announce-added-entrant", name = entrant(added))
            }
            Self::AddedEntrants(count) => t!(tr, "announce-added-entrants", count = *count),
            Self::RemovedEntrant(removed) => {
                t!(tr, "announce-removed-entrant", name = entrant(removed))
            }
            Self::RenamedEntrant(was, is) => t!(
                tr,
                "announce-renamed-entrant",
                name = entrant(was),
                new_name = entrant(is),
            ),
            Self::AddedPhase => tr.get("announce-added-phase"),
            Self::AddedPhases(count) => t!(tr, "announce-added-phases", count = *count),
            Self::EditedPhase(results) => {
                let mut sentences = vec![tr.get("announce-edited-phase")];
                if *results > 0 {
                    sentences.push(t!(tr, "announce-results-stay", count = *results));
                }
                sentences.join(" ")
            }
            Self::Started(matches) => t!(tr, "announce-started", count = *matches),
            Self::RemovedPhase => tr.get("announce-removed-phase"),
            Self::StartedPhase(phase, matches) => t!(
                tr,
                "announce-started-phase",
                phase = phase
                    .as_ref()
                    .map(|phase| tr.phase_name(phase))
                    .unwrap_or_default(),
                count = *matches,
            ),
            Self::Reported(reported) => {
                let row = &reported.row;
                let mut sentences = vec![t!(
                    tr,
                    "announce-reported",
                    number = row.number,
                    result = result_with_scores(tr, row).unwrap_or_default(),
                )];
                if reported.new_matches > 0 {
                    sentences.push(t!(tr, "announce-new-matches", count = reported.new_matches));
                }
                if reported.finished_phase && reported.winners.is_none() {
                    sentences.push(tr.get("announce-phase-finished"));
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
            Self::Copied => tr.get("announce-copied"),
            Self::Imported(name) => t!(tr, "announce-imported", name = name.as_str()),
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
        Action::AddEntrants(entrants) => t!(tr, "action-add-entrants", count = entrants.len()),
        Action::RemoveEntrant(entrant) => t!(
            tr,
            "action-remove-entrant",
            name = team_name(tr, std::slice::from_ref(entrant)),
        ),
        Action::AddPhase(phase) => t!(tr, "action-add-phase", phase = tr.phase_name(phase)),
        Action::AddPhases(phases) => t!(tr, "action-add-phases", count = phases.len()),
        Action::EditPhase(phase) => t!(tr, "action-edit-phase", phase = tr.phase_name(phase)),
        Action::RemovePhase(phase) => {
            t!(tr, "action-remove-phase", phase = tr.phase_name(phase))
        }
        Action::Start => tr.get("action-start"),
        Action::StartPhase(phase) => t!(tr, "action-start-phase", phase = tr.phase_name(phase)),
        Action::Report(row) => t!(
            tr,
            "action-report",
            number = row.number,
            result = result_text(tr, row).unwrap_or_default(),
        ),
        Action::Reopen(row) => t!(tr, "action-reopen", number = row.number),
    }
}
