//! Entering the result of a match whose match format enters more than who won, e.g. a score, and
//! showing it once reported.
//!
//! A match whose result takes no more than a row ([`ResultForm::quick_entry`]), e.g. a score or the
//! score of each game, is entered in the row below its pairing ([`quick`]), the dialog
//! ([`dialog`]) a button away for anything more, e.g. overriding the result; any other is entered
//! in the dialog.
//!
//! A value left completes the score of the other side where its winning score is the only one
//! possible ([`ResultForm::complete_score`]), selected if the focus moves there.
//!
//! What a field cannot take, e.g. a count above 65535, stays as typed, marked as invalid, with why
//! below the row or the field ([`ResultForm::enter`]).

mod dialog;
mod quick;
mod reported;
mod value;

use dioxus::prelude::*;
use tournament_app::{i18n::Translator, result_form::ResultForm, view::MatchRow};
use tournament_engine::Sides;

use self::quick::QuickResult;
pub use self::{dialog::EnterResult, reported::ResultDetails};
use crate::{competition::Ctx, i18n::t, play::Focus};

/// What enters the result of the match `row`, whose match format enters more than who won: the row
/// of its values if it takes no more, else the button opening the dialog; once reported, the focus
/// moves to `after_report`.
#[component]
pub fn ResultOf(row: MatchRow, focus: Signal<Option<Focus>>, after_report: Focus) -> Element {
    let ctx = use_context::<Ctx>();
    let id = row.id;
    let mut form = use_signal(|| ctx.session.peek().result_form(id).ok());
    // what the result looks like, changing only with the match format, e.g. when the phase is
    // edited; what was entered for another one is dropped then
    let template = use_memo(move || ctx.session.read().result_template(id).ok());
    use_effect(move || {
        if form.peek().as_ref().map(ResultForm::template) != template.read().as_ref() {
            form.set(ctx.session.peek().result_form(id).ok());
        }
    });
    let quick = form.read().as_ref().and_then(ResultForm::quick_entry);
    match quick {
        Some(quick) => rsx! {
            QuickResult {
                row,
                form,
                quick,
                focus,
                after_report,
            }
        },
        None => rsx! {
            EnterResult { row, focus, after_report }
        },
    }
}

/// `said`, what is said of a part's result, with the parts each side won, if any, in the order of
/// the sides, e.g. "Bob wins (1–3)" or "Not decided yet (1–1)".
fn with_parts_won(tr: &Translator, said: String, parts_won: Option<Sides<u32>>) -> String {
    match parts_won {
        Some(won) => t!(
            tr,
            "result-with-parts-won",
            result = said,
            won = tr.score(won)
        ),
        None => said,
    }
}
