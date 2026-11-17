//! Presets: the phases of a whole competition set up at once, while it has none.

use tournament_gui_e2e::{Gui, run_in_new_competition, thirtyfour::error::WebDriverResult};

/// Enters six entrants, chooses groups then a knockout in three groups, and adds its phases.
async fn three_groups_then_knockout(gui: &Gui) -> WebDriverResult<()> {
    gui.paste_entrant_names("Alice\nBob\nCarol\nDave\nEve\nFrank")
        .await?;
    gui.choose_how_played("Groups then knockout").await?;
    gui.enter("preset.groups", "3").await?;
    gui.expect_preview(&[
        ("Group phase", "Everyone registered"),
        ("Knockout", "Places 1–6 of Group phase"),
    ])
    .await?;
    gui.page.check_accessibility().await?;
    gui.click("Add phases").await?;
    gui.expect_announcement("2 phases are added.").await?;
    gui.expect_phase_list(&[
        ("Group phase", "Everyone registered"),
        ("Knockout", "Places 1–6 of Group phase"),
    ])
    .await
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_preset_sets_up_phases_edited_and_played_as_any() {
    run_in_new_competition(async |gui| {
        three_groups_then_knockout(&gui).await?;
        gui.expect_how_played(false).await?;

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
        gui.expect_bar_phase(Some("Knockout")).await?;
        // six of a bracket of eight, without a match for third place
        for number in 4..=8 {
            gui.enter_result(&format!("{number} 1")).await?;
        }
        gui.go_to_phase("Ranking").await?;
        gui.expect_bar_phase(Some("Ranking")).await
    })
    .await;
}

/// Adding the phases of a preset is one action: one undo takes all of them back and offers the
/// presets again, one redo adds them again.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_phases_of_a_preset_are_undone_and_redone_as_a_whole() {
    run_in_new_competition(async |gui| {
        three_groups_then_knockout(&gui).await?;

        gui.click("Undo").await?;
        gui.expect_announcement("Undone: adding 2 phases.").await?;
        gui.expect_how_played(true).await?;
        gui.click("Redo").await?;
        gui.expect_announcement("Redone: adding 2 phases.").await?;
        gui.expect_phase_list(&[
            ("Group phase", "Everyone registered"),
            ("Knockout", "Places 1–6 of Group phase"),
        ])
        .await
    })
    .await;
}
