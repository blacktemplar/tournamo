//! Results entered in a dialog, by the match format of the phase: a score, the games of a best of.

use tournament_gui_e2e::{Gui, run_in_new_competition, thirtyfour::error::WebDriverResult};

/// Enters Alice and Bob and starts a round robin of them whose results are entered by the match
/// format labelled `match_format`.
async fn start(gui: &Gui, match_format: &str) -> WebDriverResult<()> {
    for name in ["Alice", "Bob"] {
        gui.add_entrant(name).await?;
    }
    gui.choose_format("Round robin").await?;
    gui.choose_plugin("match_format", match_format).await?;
    gui.click("Add phase").await?;
    gui.click("Start").await
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_score_entered_decides_the_match_and_shows_in_it() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;
        gui.click_labelled("Enter result of match 1").await?;
        gui.page.check_accessibility().await?;

        gui.expect_result_outcomes(&["Match: Not decided yet"])
            .await?;
        gui.enter_value("Score of Alice", "17").await?;
        gui.enter_value("Score of Bob", "21").await?;
        gui.expect_result_outcomes(&["Match: Bob wins"]).await?;
        gui.expect_result_choices(&[]).await?;
        gui.click("Report").await?;

        gui.expect_no_dialog().await?;
        gui.expect_scores_either_way("17–21").await?;
        gui.expect_standings(&["Bob", "Alice"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_result_overridden_against_the_score_is_pointed_out_and_taken() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;
        gui.click_labelled("Enter result of match 1").await?;
        gui.enter_value("Score of Alice", "1").await?;
        gui.enter_value("Score of Bob", "0").await?;

        gui.click("Override result").await?;
        gui.expect_result_choices(&["Result: Match"]).await?;
        gui.page.check_accessibility().await?;
        gui.choose_result("Result: Match", "Bob wins").await?;

        gui.expect_result_warnings(&["This is not the result the score suggests."])
            .await?;
        gui.click("Report").await?;
        gui.expect_standings(&["Bob", "Alice"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_result_overridden_follows_the_score_again_on_request() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;
        gui.click_labelled("Enter result of match 1").await?;
        gui.enter_value("Score of Alice", "1").await?;
        gui.enter_value("Score of Bob", "0").await?;
        gui.click("Override result").await?;
        gui.choose_result("Result: Match", "Bob wins").await?;

        gui.click("Use result as entered").await?;

        gui.expect_result_choices(&[]).await?;
        gui.expect_result_outcomes(&["Match: Alice wins"]).await?;
        gui.expect_result_warnings(&[]).await?;
        gui.click("Report").await?;
        gui.expect_standings(&["Alice", "Bob"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_games_of_a_best_of_decide_the_match() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        gui.click_labelled("Enter result of match 1").await?;
        gui.page.check_accessibility().await?;

        for (game, alice, bob) in [(1, "21", "17"), (2, "19", "21"), (3, "21", "15")] {
            gui.enter_value(&format!("Score of Alice in Game {game}"), alice)
                .await?;
            gui.enter_value(&format!("Score of Bob in Game {game}"), bob)
                .await?;
        }
        gui.expect_result_outcomes(&[
            "Game 1: Alice wins",
            "Game 2: Bob wins",
            "Game 3: Alice wins",
            "Match: Alice wins",
        ])
        .await?;
        gui.click("Report").await?;

        gui.expect_scores_either_way("21–17 19–21 21–15").await?;
        gui.expect_standings(&["Alice", "Bob"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_knockout_shows_the_score_in_its_bracket() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Single elimination").await?;
        gui.choose_plugin("match_format", "Score").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        gui.click_labelled("Enter result of match 1").await?;
        gui.enter_value("Score of Alice", "3").await?;
        gui.enter_value("Score of Bob", "1").await?;
        gui.click("Report").await?;

        gui.click("Bracket").await?;

        gui.expect_bracket_scores_either_way("3–1").await?;
        gui.page.check_accessibility().await
    })
    .await;
}
