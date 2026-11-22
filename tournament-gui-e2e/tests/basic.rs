//! The basic view of the phases before the start, which a template opens: each phase with only
//! what anyone setting up a competition decides, and the settings of the whole competition, each
//! change made at once.

use tournament_gui_e2e::{Gui, run_in_new_competition, thirtyfour::error::WebDriverResult};

/// Enters six entrants and sets them up by the template `template`.
async fn set_up(gui: &Gui, template: &str) -> WebDriverResult<()> {
    gui.paste_entrant_names("Alice\nBob\nCarol\nDave\nEve\nFrank")
        .await?;
    gui.choose_how_played(template).await?;
    gui.click("Add phases").await?;
    gui.expect_announcement("2 phases are added.").await
}

/// The groups and how many go on are changed in place, each change one action.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_phases_of_a_template_are_refined_in_the_basic_view() {
    run_in_new_competition(async |gui| {
        set_up(&gui, "Groups and knockout").await?;
        gui.expect_basic_phases(&["Group phase", "Knockout"])
            .await?;
        gui.page.check_accessibility().await?;

        gui.enter_in_phase("Group phase", "divisions", "3").await?;
        gui.enter_last_place_in_phase("Knockout", "6").await?;
        gui.click("Undo").await?;
        gui.expect_announcement("Undone: editing Knockout.").await?;

        gui.choose_level("Advanced").await?;
        gui.expect_phase_list(&[
            ("Group phase", "Everyone registered"),
            ("Knockout", "Places 1–4 of Group phase"),
        ])
        .await?;
        gui.choose_level("Basic").await?;
        gui.enter_last_place_in_phase("Knockout", "6").await?;
        gui.choose_level("Advanced").await?;
        gui.expect_phase_list(&[
            ("Group phase", "Everyone registered"),
            ("Knockout", "Places 1–6 of Group phase"),
        ])
        .await
    })
    .await;
}

/// The match format is set on every phase at once, which one undo takes back.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_match_format_is_set_for_the_whole_competition() {
    run_in_new_competition(async |gui| {
        set_up(&gui, "Groups and knockout").await?;

        gui.choose_plugin("match_format", "Score").await?;
        gui.expect_field_shown("Winning score", true).await?;
        gui.page.check_accessibility().await?;

        gui.click("Undo").await?;
        gui.expect_announcement("Undone: editing 2 phases.").await
    })
    .await;
}

/// Drawn pairs are a checkbox, for the phases taking the registration.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_is_played_by_drawn_pairs_by_a_checkbox() {
    run_in_new_competition(async |gui| {
        set_up(&gui, "Groups and knockout").await?;

        gui.toggle("Draw your partner (DYP): the entrants play in pairs drawn by lot")
            .await?;
        gui.choose_level("Advanced").await?;

        gui.click_labelled("Edit phase: Group phase").await?;
        gui.expect_field_shown("Teams", true).await?;
        gui.expect_chosen("teams", "Drawn pairs").await
    })
    .await;
}
