//! Teams: entrants drawn into pairs, named by both members, found by either, split up again in a
//! later phase, and a field that cannot be paired up.

use tournament_gui_e2e::{Gui, run_in_new_competition, thirtyfour::error::WebDriverResult};

const SIX: [&str; 6] = ["Alice", "Bob", "Carol", "Dave", "Erin", "Frank"];

/// Adds `names`, then a round robin of them drawn into pairs.
async fn drawn_pairs(gui: &Gui, names: &[&str], format: &str) -> WebDriverResult<()> {
    for name in names {
        gui.add_entrant(name).await?;
    }
    gui.choose_format(format).await?;
    gui.choose_plugin("teams", "Drawn pairs").await
}

/// Six entrants drawn into pairs play as three teams of two; a second phase splitting them up
/// lists everyone on their own.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn pairs_play_together_and_are_split_up_in_a_later_phase() {
    run_in_new_competition(async |gui| {
        drawn_pairs(&gui, &SIX, "Round robin").await?;
        gui.page.check_accessibility().await?;
        gui.click("Add phase").await?;
        gui.open_new_phase("Singles").await?;
        gui.choose_plugin("teams", "Everyone on their own").await?;
        gui.click("Add phase").await?;

        gui.click("Start").await?;
        gui.expect_standings_team_sizes(&[2, 2, 2]).await?;
        gui.page.check_accessibility().await?;
        for number in 1..=3 {
            gui.enter_result(&format!("{number} 1")).await?;
        }
        gui.click("Start Singles").await?;
        gui.expect_bar_phase(Some("Singles")).await?;
        gui.expect_standings_entrants(None, &SIX).await
    })
    .await;
}

/// A result is typed with the name of either member of a team.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_result_is_typed_with_the_name_of_either_member() {
    run_in_new_competition(async |gui| {
        let eight = [
            "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi",
        ];
        drawn_pairs(&gui, &eight, "Single elimination").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        let (number, member) = gui.last_member_of_first_side().await?;

        gui.enter_result(&member).await?;

        gui.expect_no_error().await?;
        gui.expect_finished(&[&number]).await
    })
    .await;
}

/// Five entrants cannot be drawn into pairs: the start says so and is not offered.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn an_odd_number_of_entrants_is_not_drawn_into_pairs() {
    run_in_new_competition(async |gui| {
        drawn_pairs(&gui, &SIX[..5], "Round robin").await?;
        gui.click("Add phase").await?;

        gui.expect_start_refusal(Some(
            "1 · Round robin cannot form its teams (drawn pairs) of 5 entrants, e.g. an odd \
             number into pairs.",
        ))
        .await?;
        gui.expect_disabled("Start").await?;
        gui.add_entrant("Frank").await?;
        gui.expect_start_refusal(None).await
    })
    .await;
}
