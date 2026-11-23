//! Templates: the phases of a whole competition set up at once, while it has none, then refined
//! as any phase.

use tournament_gui_e2e::{Gui, run_in_new_competition, thirtyfour::error::WebDriverResult};

/// Enters six entrants, chooses groups and a knockout, and adds its phases.
async fn groups_and_knockout(gui: &Gui) -> WebDriverResult<()> {
    gui.paste_entrant_names("Alice\nBob\nCarol\nDave\nEve\nFrank")
        .await?;
    gui.choose_how_played("Groups and knockout").await?;
    gui.expect_preview(&[
        ("Group phase", "Everyone registered"),
        ("Knockout", "Places 1–4 of Group phase"),
    ])
    .await?;
    gui.page.check_accessibility().await?;
    gui.click("Add phases").await?;
    gui.expect_announcement("2 phases are added.").await?;
    // refined in the basic view
    gui.expect_basic_phases(&["Group phase", "Knockout"]).await
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_template_sets_up_phases_edited_and_played_as_any() {
    run_in_new_competition(async |gui| {
        groups_and_knockout(&gui).await?;
        gui.expect_how_played(false).await?;

        gui.enter_in_phase("Group phase", "divisions", "3").await?;
        gui.choose_level("Advanced").await?;
        gui.expect_phase_list(&[
            ("Group phase", "Everyone registered"),
            ("Knockout", "Places 1–4 of Group phase"),
        ])
        .await?;
        gui.click_labelled("Edit phase: Knockout").await?;
        gui.toggle("Match for third place").await?;
        gui.click("Save changes").await?;
        gui.expect_announcement("The phase is changed.").await?;

        gui.click("Start").await?;
        // three groups of two
        for number in 1..=3 {
            gui.enter_result(&format!("{number} 1")).await?;
        }
        gui.click("Start Knockout").await?;
        // the best 4 in total of three groups: the firsts and one of the three seconds, tied, by
        // lot
        gui.click("Start anyway").await?;
        gui.expect_bar_phase(Some("Knockout")).await?;
        // a bracket of four, without a match for third place
        for number in 4..=6 {
            gui.enter_result(&format!("{number} 1")).await?;
        }
        gui.go_to_phase("Ranking").await?;
        gui.expect_bar_phase(Some("Ranking")).await
    })
    .await;
}

/// Adding the phases of a template is one action: one undo takes all of them back and offers the
/// templates again, one redo adds them again.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_phases_of_a_template_are_undone_and_redone_as_a_whole() {
    run_in_new_competition(async |gui| {
        groups_and_knockout(&gui).await?;

        gui.click("Undo").await?;
        gui.expect_announcement("Undone: adding 2 phases.").await?;
        gui.expect_how_played(true).await?;
        gui.click("Redo").await?;
        gui.expect_announcement("Redone: adding 2 phases.").await?;
        gui.expect_basic_phases(&["Group phase", "Knockout"]).await
    })
    .await;
}

/// The setup says which template the phases came from; a setting the basic view hides that is
/// changed from the template, and a phase removed from it, are said.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn what_differs_from_the_template_is_said() {
    run_in_new_competition(async |gui| {
        groups_and_knockout(&gui).await?;
        gui.expect_based_on(&["Based on the template “Groups and knockout”."])
            .await?;
        gui.expect_basic_hidden_settings(&[]).await?;

        gui.choose_level("Expert").await?;
        gui.click_labelled("Edit phase: Knockout").await?;
        gui.choose_plugin("seeding", "At random").await?;
        gui.click("Save changes").await?;
        gui.expect_announcement("The phase is changed.").await?;
        gui.choose_level("Basic").await?;
        gui.expect_basic_hidden_settings(&[
            "One setting differs from the template, see the expert settings.",
        ])
        .await?;
        gui.page.check_accessibility().await?;

        gui.choose_level("Advanced").await?;
        gui.click_labelled("Remove phase: Knockout").await?;
        gui.expect_based_on(&[
            "Based on the template “Groups and knockout”.",
            "Removed from the template: Knockout.",
        ])
        .await
    })
    .await;
}
