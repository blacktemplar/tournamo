//! Phases of several divisions: groups shown each on its own, their matches listed together or
//! by group, and the best of the groups going on to a knockout.

use tournament_gui_e2e::{
    Gui, run_in_new_competition,
    thirtyfour::{By, error::WebDriverResult},
};

const ELEVEN: [&str; 11] = [
    "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi", "Ivan", "Judy", "Kim",
];

/// Enters eleven entrants in three groups, played as round robins compared across the groups by
/// points per match, then a knockout of the best eight of the groups.
async fn groups_and_knockout(gui: &Gui) -> WebDriverResult<()> {
    for name in ELEVEN {
        gui.add_entrant(name).await?;
    }
    gui.choose_format("Round robin").await?;
    gui.enter("divisions", "3").await?;
    gui.choose_plugin("across", "More points per match").await?;
    gui.click("Add phase").await?;
    gui.open_new_phase("Knockout").await?;
    gui.choose_format("Single elimination").await?;
    gui.choose_plugin("draw", "Groups kept apart").await?;
    gui.enter_place(0, "to", "8").await?;
    gui.click("Add phase").await?;
    gui.expect_announcement("The phase is added.").await
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn groups_then_a_knockout_of_their_best_are_played_through() {
    run_in_new_competition(async |gui| {
        groups_and_knockout(&gui).await?;
        gui.click("Start").await?;
        gui.expect_standings_headings(&["Group A", "Group B", "Group C"])
            .await?;
        // group A has three entrants, so one match of it can be played, and two of each other
        gui.expect_division_tags(&["A", "B", "B", "C", "C"]).await?;
        gui.click_labelled("Group B").await?;
        gui.expect_division_tags(&["B", "B"]).await?;
        gui.page.check_accessibility().await?;
        gui.click("All").await?;

        // every match drawn, from the list of every group, so that everyone shares the first
        // place of their group, and the cut of the knockout goes through them
        for number in 1..=15 {
            gui.enter_result(&format!("{number} x")).await?;
        }
        gui.expect_announcement("Match 15: Draw. Every match of the phase has a result.")
            .await?;
        gui.go_to_phase("1 · Round robin").await?;
        gui.expect_compared(&["Place 1 compared across the groups"], 11)
            .await?;
        gui.page.check_accessibility().await?;

        gui.click("Start Knockout").await?;
        gui.page
            .expect_texts(
                By::Css("[role=dialog] h2"),
                &["Start with a tie decided by lot?"],
            )
            .await?;
        gui.click("Start anyway").await?;
        gui.expect_bar_phase(Some("Knockout")).await?;
        gui.expect_standings_headings(&["Standings"]).await?;
        // with a match for third place
        for number in 16..=23 {
            gui.enter_result(&format!("{number} 1")).await?;
        }
        gui.page
            .expect_texts(By::Css(".winners-banner h2"), &["Finished"])
            .await?;
        gui.go_to_phase("Ranking").await?;
        gui.page
            .expect_texts(
                By::Css("#ranking-list tbody td:last-child"),
                &[
                    "Knockout",
                    "Knockout",
                    "Knockout",
                    "Knockout",
                    "Knockout",
                    "Knockout",
                    "Knockout",
                    "Knockout",
                    "1 · Round robin",
                    "1 · Round robin",
                    "1 · Round robin",
                ],
            )
            .await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn knockouts_in_blocks_play_for_places_of_their_own() {
    run_in_new_competition(async |gui| {
        for name in &ELEVEN[..8] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Single elimination").await?;
        gui.enter("divisions", "2").await?;
        gui.choose_plugin("draw", "In blocks").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        gui.expect_standings_headings(&["Single elimination A", "Single elimination B"])
            .await?;
        // each a knockout of four with a match for third place
        for number in 1..=8 {
            gui.enter_result(&format!("{number} 1")).await?;
        }
        gui.page
            .expect_texts(By::Css(".winners-banner h2"), &["Finished"])
            .await?;
        gui.click("Bracket").await?;
        gui.page
            .expect_texts(
                By::Css("h2[id^='bracket']"),
                &["Single elimination A", "Single elimination B"],
            )
            .await?;
        gui.page.check_accessibility().await
    })
    .await;
}
