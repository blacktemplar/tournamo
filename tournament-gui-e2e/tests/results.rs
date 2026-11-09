//! Results entered by the match format of the phase, a score or the games of a best of: in the
//! row of the match, in a dialog, or typed in.

use tournament_gui_e2e::{Gui, Key, run_in_new_competition, thirtyfour::error::WebDriverResult};

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
        gui.click_labelled("Details of the result of match 1")
            .await?;
        gui.page.check_accessibility().await?;

        gui.expect_result_outcomes(&["Match: Not decided yet"])
            .await?;
        gui.enter_value("Score of Alice", "17").await?;
        gui.enter_value("Score of Bob", "21").await?;
        gui.expect_result_outcomes(&["Match: Bob wins"]).await?;
        gui.expect_result_choices(&[]).await?;
        gui.report_in_dialog().await?;

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
        gui.click_labelled("Details of the result of match 1")
            .await?;
        gui.enter_value("Score of Alice", "1").await?;
        gui.enter_value("Score of Bob", "0").await?;

        gui.click("Override result").await?;
        gui.expect_result_choices(&["Result: Match"]).await?;
        gui.page.check_accessibility().await?;
        gui.choose_result("Result: Match", "Bob wins").await?;

        gui.expect_result_warnings(&["This is not the result the score suggests."])
            .await?;
        gui.report_in_dialog().await?;
        gui.expect_standings(&["Bob", "Alice"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_result_overridden_follows_the_score_again_on_request() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;
        gui.click_labelled("Details of the result of match 1")
            .await?;
        gui.enter_value("Score of Alice", "1").await?;
        gui.enter_value("Score of Bob", "0").await?;
        gui.click("Override result").await?;
        gui.choose_result("Result: Match", "Bob wins").await?;

        gui.click("Use result as entered").await?;

        gui.expect_result_choices(&[]).await?;
        gui.expect_result_outcomes(&["Match: Alice wins"]).await?;
        gui.expect_result_warnings(&[]).await?;
        gui.report_in_dialog().await?;
        gui.expect_standings(&["Alice", "Bob"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_games_of_a_best_of_decide_the_match() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        gui.click_labelled("Details of the result of match 1")
            .await?;
        gui.page.check_accessibility().await?;

        for (game, alice, bob) in [(1, "21", "17"), (2, "19", "21"), (3, "21", "15")] {
            gui.enter_value(&format!("Score of Alice in Game {game}"), alice)
                .await?;
            gui.enter_value(&format!("Score of Bob in Game {game}"), bob)
                .await?;
        }
        gui.expect_result_outcomes_either_way(&[
            "Game 1: Alice wins",
            "Game 2: Bob wins",
            "Game 3: Alice wins",
            "Match: Alice wins (2–1)",
        ])
        .await?;
        gui.report_in_dialog().await?;

        gui.expect_standings(&["Alice", "Bob"]).await?;
        // across the page, where the list has room for the scores of the games
        gui.page.resize(900, 900).await?;
        gui.expect_scores_either_way("2–1 (21–17 19–21 21–15)")
            .await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_score_entered_in_the_row_of_the_match_reports_it() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;

        gui.expect_row_outcomes(&[]).await?;
        gui.enter_in_row("Score of Alice, match 1", "17").await?;
        gui.enter_in_row("Score of Bob, match 1", "21").await?;
        gui.expect_row_outcomes(&["Bob wins"]).await?;
        gui.page.check_accessibility().await?;
        gui.click_labelled("Report match 1").await?;

        gui.expect_scores_either_way("17–21").await?;
        gui.expect_standings(&["Bob", "Alice"]).await
    })
    .await;
}

/// Starts a round robin of Alice and Bob whose results are scores usually won with 21.
async fn start_to_21(gui: &Gui) -> WebDriverResult<()> {
    for name in ["Alice", "Bob"] {
        gui.add_entrant(name).await?;
    }
    gui.choose_format("Round robin").await?;
    gui.choose_plugin("match_format", "Score").await?;
    gui.toggle("Winning score").await?;
    gui.enter("match_format.winning_score", "21").await?;
    gui.click("Add phase").await?;
    gui.click("Start").await
}

/// A score two or more below the winning score, once left, gives the other side the winning score,
/// selected where Tab moves to, so that typing replaces it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_score_far_below_the_winning_score_completes_the_other_in_the_row() {
    run_in_new_competition(async |gui| {
        start_to_21(&gui).await?;

        gui.enter_first_in_row(1, "15").await?;
        gui.press(Key::Tab).await?;

        gui.expect_focus_value("21").await?;
        gui.press(Key::Enter).await?;
        gui.expect_scores_either_way("15–21").await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_winning_score_completed_is_replaced_by_what_is_typed() {
    run_in_new_competition(async |gui| {
        start_to_21(&gui).await?;
        gui.enter_first_in_row(1, "19").await?;
        gui.press(Key::Tab).await?;
        gui.expect_focus_value("21").await?;

        gui.type_text("17").await?;

        gui.expect_focus_value("17").await?;
        gui.press(Key::Enter).await?;
        gui.expect_scores_either_way("19–17").await
    })
    .await;
}

/// From one below the winning score on, the other side may win by more, or lose.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_score_close_to_the_winning_score_completes_nothing() {
    run_in_new_competition(async |gui| {
        start_to_21(&gui).await?;

        gui.enter_first_in_row(1, "20").await?;
        gui.press(Key::Tab).await?;

        gui.expect_focus_value("").await?;
        gui.expect_row_outcomes(&["Not decided yet"]).await
    })
    .await;
}

/// Tab out of a score completed waits for the game the match needs then, and moves into it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn tab_out_of_a_score_completed_moves_into_the_game_it_makes_needed() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Round robin").await?;
        gui.choose_plugin("match_format", "Best of games").await?;
        gui.toggle("Winning score of a game").await?;
        gui.enter("match_format.winning_score", "21").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        gui.enter_in_part_in_row("Game 1, match 1", 0, "17").await?;
        gui.enter_in_part_in_row("Game 1, match 1", 1, "21").await?;

        // the second side, after which Tab would leave the games shown so far
        gui.enter_in_part_in_row("Game 2, match 1", 1, "15").await?;
        gui.press(Key::Tab).await?;

        gui.expect_focus_in_part_in_row("Game 3, match 1").await?;
        gui.expect_row_outcomes(&["Not decided yet (1–1)"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_score_far_below_the_winning_score_completes_the_other_in_the_dialog() {
    run_in_new_competition(async |gui| {
        start_to_21(&gui).await?;
        gui.click_labelled("Details of the result of match 1")
            .await?;

        gui.enter_first_value("8").await?;
        gui.press(Key::Tab).await?;

        gui.expect_focus_value("21").await?;
        gui.report_in_dialog().await?;
        gui.expect_scores_either_way("8–21").await
    })
    .await;
}

/// A count too large to be one stays as typed, marked as invalid with why, and neither leaving it
/// nor leaving a score far below the winning score on the other side completes anything.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_count_too_large_stays_marked_and_is_not_completed_over_in_the_row() {
    run_in_new_competition(async |gui| {
        start_to_21(&gui).await?;

        gui.enter_first_in_row(1, "70000").await?;
        gui.press(Key::Tab).await?;
        gui.expect_focus_value("").await?;
        gui.type_text("15").await?;
        gui.press(Key::Tab).await?;

        gui.expect_marked_invalid("70000").await?;
        gui.expect_row_issues(&["This takes a whole number from 0 to 65535."])
            .await?;
        gui.expect_row_outcomes(&["Not decided yet"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_count_too_large_stays_marked_in_the_dialog() {
    run_in_new_competition(async |gui| {
        start_to_21(&gui).await?;
        gui.click_labelled("Details of the result of match 1")
            .await?;

        gui.enter_first_value("70000").await?;
        gui.press(Key::Tab).await?;

        gui.expect_focus_value("").await?;
        gui.expect_marked_invalid("70000").await?;
        gui.expect_result_issues(&["This takes a whole number from 0 to 65535."])
            .await
    })
    .await;
}

/// The games of a best of are entered side by side, and Enter in any of them reports the match.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_games_of_a_best_of_are_entered_in_the_row_of_the_match() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;

        for (game, alice, bob) in [(1, "21", "17"), (2, "19", "21"), (3, "21", "15")] {
            gui.enter_in_row(&format!("Score of Alice in Game {game}, match 1"), alice)
                .await?;
            gui.enter_in_row(&format!("Score of Bob in Game {game}, match 1"), bob)
                .await?;
        }
        gui.expect_row_outcomes_either_way(&["Alice wins (2–1)"])
            .await?;
        gui.press_enter_in_row("Score of Bob in Game 3, match 1")
            .await?;

        gui.expect_standings(&["Alice", "Bob"]).await?;
        // across the page, where the list has room for the scores of the games
        gui.page.resize(900, 900).await?;
        gui.expect_scores_either_way("2–1 (21–17 19–21 21–15)")
            .await
    })
    .await;
}

/// A game of a best of appears in the row once the match needs it, in time for Tab to move into it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_game_appears_in_the_row_once_the_match_needs_it() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        let third = "Score of Alice in Game 3, match 1";
        gui.expect_in_row(third, false).await?;

        for (game, alice, bob) in [(1, "21", "17"), (2, "17", "21")] {
            gui.enter_in_row(&format!("Score of Alice in Game {game}, match 1"), alice)
                .await?;
            gui.enter_in_row(&format!("Score of Bob in Game {game}, match 1"), bob)
                .await?;
        }
        gui.tab_out_of_part_in_row("Game 2, match 1").await?;

        gui.expect_focus_in_part_in_row("Game 3, match 1").await?;
        gui.expect_in_row(third, true).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_result_the_row_does_not_decide_is_not_reported() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        gui.enter_in_row("Score of Alice in Game 1, match 1", "21")
            .await?;
        gui.enter_in_row("Score of Bob in Game 1, match 1", "17")
            .await?;
        gui.expect_row_outcomes_either_way(&["Not decided yet (1–0)"])
            .await?;

        gui.click_labelled("Report match 1").await?;

        gui.expect_row_error("The result does not say who won.")
            .await?;
        gui.expect_finished(&[]).await
    })
    .await;
}

/// A phase edited after the start enters its matches as its new match format does.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_row_of_a_match_follows_the_phase_edited() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        gui.enter_in_row("Score of Alice in Game 1, match 1", "21")
            .await?;

        gui.click("Phase").await?;
        gui.click("Edit phase").await?;
        gui.enter("match_format.games", "5").await?;
        gui.click("Save changes").await?;
        gui.click("Matches & standings").await?;

        for game in 1..=2 {
            gui.enter_in_row(&format!("Score of Alice in Game {game}, match 1"), "21")
                .await?;
            gui.enter_in_row(&format!("Score of Bob in Game {game}, match 1"), "17")
                .await?;
        }
        // two games of five
        gui.expect_row_outcomes_either_way(&["Not decided yet (2–0)"])
            .await?;
        gui.enter_in_row("Score of Alice in Game 3, match 1", "21")
            .await
    })
    .await;
}

/// The dialog takes over what was entered in the row, for what only it enters, e.g. overriding
/// the result.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_details_of_a_result_start_from_the_row_of_the_match() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;
        gui.enter_in_row("Score of Alice, match 1", "1").await?;
        gui.enter_in_row("Score of Bob, match 1", "0").await?;

        gui.click_labelled("Details of the result of match 1")
            .await?;

        gui.expect_result_outcomes(&["Match: Alice wins"]).await?;
        gui.click("Override result").await?;
        gui.choose_result("Result: Match", "Bob wins").await?;
        gui.report_in_dialog().await?;
        gui.expect_standings(&["Bob", "Alice"]).await
    })
    .await;
}

/// A match reopened keeps its scores in its row, so that correcting a typo takes changing only
/// that value.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_match_reopened_keeps_its_scores_to_be_corrected() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        for (game, alice, bob) in [(1, "21", "17"), (2, "19", "21"), (3, "21", "15")] {
            gui.enter_in_row(&format!("Score of Alice in Game {game}, match 1"), alice)
                .await?;
            gui.enter_in_row(&format!("Score of Bob in Game {game}, match 1"), bob)
                .await?;
        }
        gui.click_labelled("Report match 1").await?;

        gui.click_labelled("Reopen match 1").await?;

        gui.expect_in_row_value("Score of Alice in Game 2, match 1", "19")
            .await?;
        gui.expect_row_outcomes_either_way(&["Alice wins (2–1)"])
            .await?;
        gui.enter_in_row("Score of Bob in Game 3, match 1", "19")
            .await?;
        gui.click_labelled("Report match 1").await?;
        // across the page, where the list has room for the scores of the games
        gui.page.resize(900, 900).await?;
        gui.expect_scores_either_way("2–1 (21–17 19–21 21–19)")
            .await
    })
    .await;
}

/// A result reported against the score is reopened overriding it, which only the dialog shows.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_result_overridden_is_reopened_overriding_in_the_dialog() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;
        gui.enter_in_row("Score of Alice, match 1", "1").await?;
        gui.enter_in_row("Score of Bob, match 1", "0").await?;
        gui.click_labelled("Details of the result of match 1")
            .await?;
        gui.click("Override result").await?;
        gui.choose_result("Result: Match", "Bob wins").await?;
        gui.report_in_dialog().await?;

        gui.click_labelled("Reopen match 1").await?;

        gui.click_labelled("Enter result of match 1").await?;
        gui.expect_result_choices(&["Result: Match"]).await?;
        gui.expect_result_warnings(&["This is not the result the score suggests."])
            .await?;
        gui.page.check_accessibility().await?;
        gui.report_in_dialog().await?;
        gui.expect_standings(&["Bob", "Alice"]).await
    })
    .await;
}

/// Scores typed in after part of the winner's name are the winner's first, and must say the
/// result the name does.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_score_is_typed_in_after_the_name_of_the_winner() {
    run_in_new_competition(async |gui| {
        start(&gui, "Score").await?;

        gui.type_result("ali 17-21").await?;
        gui.expect_result_preview(Some(
            "The result entered for match 1 is not the one its scores say.",
        ))
        .await?;
        gui.enter_result("bob 21-17").await?;

        gui.expect_scores_either_way("17–21").await?;
        gui.expect_standings(&["Bob", "Alice"]).await
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
        gui.enter_in_row("Score of Alice, match 1", "3").await?;
        gui.enter_in_row("Score of Bob, match 1", "1").await?;
        gui.click_labelled("Report match 1").await?;

        gui.click("Bracket").await?;

        gui.expect_bracket_scores_either_way("3–1").await?;
        gui.page.check_accessibility().await?;
        // its score, underlined, shows that the pairing opens what was reported for it
        assert_eq!(
            gui.page
                .style_of(".bracket .scores", "text-decoration-line")
                .await?,
            "underline"
        );
        gui.click_labelled("Details of the result of match 1")
            .await?;
        gui.expect_reported_result(&[&[]], &[], "Match: Alice wins")
            .await
    })
    .await;
}

/// A finished match shows what was reported for it in a dialog of its own, read-only.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_details_of_a_finished_match_show_its_games() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        gui.enter_result("ali 21-17 19-21 21-15").await?;

        // the match itself is the button
        gui.click_labelled("Details of the result of match 1")
            .await?;

        gui.expect_reported_result(
            &[
                &["21", "17", "19", "21", "21", "15"],
                &["17", "21", "21", "19", "15", "21"],
            ],
            &["21", "21", "21"],
            "Match: Alice wins (2–1)",
        )
        .await?;
        gui.page.check_accessibility().await?;
        gui.click("Close").await?;
        gui.expect_no_dialog().await
    })
    .await;
}

/// A finished match shows that it opens by its score, underlined as a link is, as a phone has no
/// hover to show it; a tap on the score opens what was reported for it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_tap_on_the_underlined_score_of_a_finished_match_opens_its_details() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        gui.enter_result("ali 21-17 19-21 21-15").await?;
        gui.page.resize(390, 844).await?;
        gui.expect_finished(&["1"]).await?;

        assert_eq!(
            gui.page
                .style_of("#match-list .result.scores", "text-decoration-line")
                .await?,
            "underline"
        );
        gui.tap_scores_of(1).await?;

        gui.expect_reported_result(
            &[
                &["21", "17", "19", "21", "21", "15"],
                &["17", "21", "21", "19", "15", "21"],
            ],
            &["21", "21", "21"],
            "Match: Alice wins (2–1)",
        )
        .await
    })
    .await;
}

/// The cross table shows the games each side won of a best of, from the row's side, a win in
/// bold; a screen reader says them after how the match ended. A click on a match, its score
/// underlined, opens its details.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_cross_table_shows_the_games_won_and_opens_the_details() {
    run_in_new_competition(async |gui| {
        start(&gui, "Best of games").await?;
        gui.enter_result("ali 21-17 19-21 21-15").await?;
        gui.click("Cross table").await?;

        gui.expect_cross_table_row(1, &["", "2–1\nwon, 2–1", "1"])
            .await?;
        gui.expect_cross_table_row(2, &["1–2\nlost, 1–2", "", "0"])
            .await?;
        gui.expect_cross_table_row_said(1, &["", "won, 2–1 Details of the result of match 1", "1"])
            .await?;
        gui.expect_cross_table_row_said(
            2,
            &["lost, 1–2 Details of the result of match 1", "", "0"],
        )
        .await?;
        let win = gui.page.style_of(".outcome.win", "font-weight").await?;
        let loss = gui.page.style_of(".outcome.loss", "font-weight").await?;
        assert!(win > loss, "{win} {loss}");
        assert_eq!(
            gui.page
                .style_of(".outcome.win", "text-decoration-line")
                .await?,
            "underline"
        );
        gui.page.check_accessibility().await?;

        gui.click_labelled("Details of the result of match 1")
            .await?;

        gui.expect_reported_result(
            &[
                &["21", "17", "19", "21", "21", "15"],
                &["17", "21", "21", "19", "15", "21"],
            ],
            &["21", "21", "21"],
            "Match: Alice wins (2–1)",
        )
        .await?;
        gui.page.check_accessibility().await?;
        gui.click("Close").await?;
        gui.expect_no_dialog().await
    })
    .await;
}
