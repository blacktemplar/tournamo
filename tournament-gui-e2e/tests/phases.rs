//! Competitions of several phases: adding, starting and following them, and the ranking.

use tournament_gui_e2e::{
    Gui, run_in_new_competition,
    thirtyfour::{By, error::WebDriverResult},
};

/// Enters Alice, Bob, Carol and Dave, a round robin of them, and a final of its places 1–2.
async fn round_robin_and_final(gui: &Gui) -> WebDriverResult<()> {
    for name in ["Alice", "Bob", "Carol", "Dave"] {
        gui.add_entrant(name).await?;
    }
    gui.choose_format("Round robin").await?;
    gui.click("Add phase").await?;
    gui.open_new_phase("Final").await?;
    gui.choose_format("Single elimination").await?;
    gui.enter_place(0, "to", "2").await?;
    gui.click("Add phase").await?;
    gui.expect_announcement("The phase is added.").await
}

/// Draws every match of the round robin, so that everyone shares the first place.
async fn draw_round_robin(gui: &Gui) -> WebDriverResult<()> {
    for number in 1..=6 {
        gui.enter_result(&format!("{number} x")).await?;
    }
    gui.expect_announcement("Match 6: Draw. Every match of the phase has a result.")
        .await
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_of_two_phases_is_played_through() {
    run_in_new_competition(async |gui| {
        round_robin_and_final(&gui).await?;
        gui.click("Start").await?;
        gui.expect_phase_links(&["1 · Round robin", "Final", "Ranking"], "1 · Round robin")
            .await?;
        gui.expect_bar_phase(Some("1 · Round robin")).await?;
        // tied, yet the lot has set the first two apart
        gui.expect_cuts(&["Places 1–2 go on to Final"]).await?;

        draw_round_robin(&gui).await?;
        gui.click("Start Final").await?;
        gui.page
            .expect_texts(
                By::Css("[role=dialog] h2"),
                &["Start with a tie decided by lot?"],
            )
            .await?;
        gui.page.check_accessibility().await?;
        gui.click("Start anyway").await?;

        gui.expect_bar_phase(Some("Final")).await?;
        gui.expect_focus("Matches").await?;
        gui.enter_result("7 1").await?;
        gui.page
            .expect_texts(By::Css(".winners-banner h2"), &["Finished"])
            .await?;
        gui.go_to_phase("Ranking").await?;
        gui.expect_bar_phase(Some("Ranking")).await?;
        gui.page
            .expect_texts(
                By::Css("#ranking-list tbody td:last-child"),
                &["Final", "Final", "1 · Round robin", "1 · Round robin"],
            )
            .await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_result_is_not_reopened_once_the_next_phase_started() {
    run_in_new_competition(async |gui| {
        round_robin_and_final(&gui).await?;
        gui.click("Start").await?;
        draw_round_robin(&gui).await?;
        gui.click("Start Final").await?;
        gui.click("Start anyway").await?;
        gui.go_to_phase("1 · Round robin").await?;

        gui.click_labelled("Reopen match 6").await?;

        gui.expect_error(
            "Final has started with the entrants of this phase; undo its start first.",
        )
        .await?;
        // the start of a phase is undone without a question, staying where the page is
        gui.click("Undo").await?;
        gui.expect_announcement("Undone: the start of Final.")
            .await?;
        gui.expect_bar_phase(Some("1 · Round robin")).await?;
        gui.click_labelled("Reopen match 6").await?;
        gui.expect_announcement("Match 6 is open again.").await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_phase_is_removed_and_brought_back_by_undo() {
    run_in_new_competition(async |gui| {
        round_robin_and_final(&gui).await?;
        // the round robin is in use by the final
        gui.expect_disabled("Remove phase").await?;

        gui.click_labelled("Remove phase: Final").await?;

        gui.expect_announcement("The phase is removed.").await?;
        gui.click("Undo").await?;
        gui.expect_announcement("Undone: removing Final.").await?;
        gui.page
            .expect_texts(By::Css("#phases h3"), &["1 · Round robin", "Final"])
            .await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_reload_stays_on_the_phase_shown() {
    run_in_new_competition(async |gui| {
        round_robin_and_final(&gui).await?;
        gui.click("Start").await?;
        gui.go_to_phase("Final").await?;
        gui.expect_bar_phase(Some("Final")).await?;
        gui.page.check_accessibility().await?;

        gui.page.reload().await?;

        gui.expect_bar_phase(Some("Final")).await?;
        gui.expect_phase_links(&["1 · Round robin", "Final", "Ranking"], "Final")
            .await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_of_one_phase_shows_no_links_to_phases() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob"] {
            gui.add_entrant(name).await?;
        }
        gui.click("Add phase").await?;
        gui.click("Start").await?;

        gui.expect_focus("Matches").await?;
        gui.expect_phase_links(&[], "").await?;
        gui.expect_bar_phase(None).await
    })
    .await;
}

/// A phase with no tie at its cut starts at once from its button.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_phase_without_a_tie_at_its_cut_starts_at_once() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob", "Carol", "Dave"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Round robin").await?;
        gui.click("Add phase").await?;
        gui.open_new_phase("Final").await?;
        gui.choose_format("Single elimination").await?;
        gui.enter_place(0, "to", "4").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        draw_round_robin(&gui).await?;

        gui.click("Start Final").await?;

        gui.expect_bar_phase(Some("Final")).await?;
        gui.expect_focus("Matches").await
    })
    .await;
}
