use tournament_gui_e2e::{Gui, Key, run_in_new_competition, thirtyfour::error::WebDriverResult};

/// Enters Alice and Bob, adds a round robin with what it recommends, and starts.
async fn start_round_robin(gui: &Gui) -> WebDriverResult<()> {
    gui.add_entrant("Alice").await?;
    gui.add_entrant("Bob").await?;
    gui.click("Add phase").await?;
    gui.click("Start").await
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_format_brings_what_it_recommends() {
    run_in_new_competition(async |gui| {
        gui.expect_list("ranking", &["More points", "More Sonneborn-Berger"])
            .await?;

        gui.choose_format("Single elimination").await?;

        gui.expect_list("ranking", &["Later first loss", "Fewer losses"])
            .await?;
        gui.page.check_accessibility().await?;
        gui.click("Add phase").await?;
        gui.expect_phase(&[
            "Single elimination",
            "Later first loss, fewer losses",
            "Round of the first loss, win-draw-loss record",
        ])
        .await
    })
    .await;
}

/// Enter takes the number typed, as leaving the field does, rather than adding the phase.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn enter_in_a_number_takes_it_without_adding_the_phase() {
    run_in_new_competition(async |gui| {
        gui.choose_format("Monster DYP").await?;

        gui.type_into("format.rounds", "0").await?;
        gui.press(Key::Enter).await?;

        gui.expect_issues(&["This takes a whole number from 1 to 100."])
            .await?;
        gui.type_into("format.rounds", "7").await?;
        gui.press(Key::Enter).await?;
        gui.expect_issues(&[]).await?;
        gui.expect_phase(&[]).await?;
        gui.click("Add phase").await?;
        gui.expect_phase(&[
            "Monster DYP",
            "More points",
            "Win-draw-loss record",
            "Points",
        ])
        .await
    })
    .await;
}

/// Adding a phase that cannot be played shows what to change rather than refusing it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn adding_a_phase_with_issues_moves_the_focus_to_them() {
    run_in_new_competition(async |gui| {
        gui.toggle("Points").await?;

        gui.click("Add phase").await?;

        gui.expect_focus(
            "More points needs one of these: points.\nSonneborn-Berger needs one of these: points.",
        )
        .await?;
        gui.expect_no_error().await?;
        gui.expect_phase(&[]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_number_not_taken_is_shown_with_why() {
    run_in_new_competition(async |gui| {
        gui.choose_format("Monster DYP").await?;

        gui.enter("format.rounds", "0").await?;

        gui.expect_issues(&["This takes a whole number from 1 to 100."])
            .await?;
        gui.enter("format.rounds", "7").await?;
        gui.expect_issues(&[]).await
    })
    .await;
}

/// A number the field does not take keeps the phase from being added, rather than the answer
/// before it being kept unnoticed: why shows while it is typed, and the focus goes to the field.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_number_not_taken_keeps_the_phase_from_being_added() {
    run_in_new_competition(async |gui| {
        gui.type_into("metrics[1].points_for_win", "x").await?;
        gui.expect_issues(&["“x” is no number."]).await?;
        gui.type_into("metrics[1].points_for_win", "-1").await?;

        gui.expect_issues(&["This takes a number of at least 0."])
            .await?;
        gui.click("Add phase").await?;
        gui.expect_phase(&[]).await?;
        gui.expect_focus_on_invalid("metrics[1].points_for_win")
            .await?;
        gui.expect_entered("metrics[1].points_for_win", "-1")
            .await?;
        gui.type_into("metrics[1].points_for_win", "3").await?;
        gui.expect_issues(&[]).await?;
        gui.click("Add phase").await?;
        gui.expect_phase(&[
            "Round robin",
            "More points, more Sonneborn-Berger",
            "Win-draw-loss record",
            "Points",
            "Sonneborn-Berger",
        ])
        .await
    })
    .await;
}

/// Leaving a number with Tab focuses the button of what is said about the next field, whose text
/// shows below it rather than over why the number was not taken.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn what_is_said_about_the_next_field_leaves_the_issue_in_sight() {
    run_in_new_competition(async |gui| {
        gui.choose_format("Monster DYP").await?;

        gui.enter("format.rounds", "0").await?;

        gui.expect_issues(&["This takes a whole number from 1 to 100."])
            .await?;
        gui.expect_toggletip_clear_of_issues().await
    })
    .await;
}

/// The points can be scored in many ways, so they are not added without being chosen, and the
/// ranking by them stays rather than being dropped unnoticed.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_metric_needed_and_not_chosen_is_an_issue_at_the_metrics() {
    run_in_new_competition(async |gui| {
        gui.toggle("Points").await?;

        gui.expect_issues(&[
            "More points needs one of these: points.",
            "Sonneborn-Berger needs one of these: points.",
        ])
        .await?;
        gui.expect_list("ranking", &["More points", "More Sonneborn-Berger"])
            .await?;
        gui.toggle("Points").await?;
        gui.expect_issues(&[]).await
    })
    .await;
}

/// A metric stays where it is when it is unchecked or checked again, keeping the focus, so that a
/// second click at the same place is at the same metric; the others show as checked still.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_metrics_stay_in_place_when_toggled() {
    const ALL: [&str; 3] = ["Points", "Sonneborn-Berger", "Win-draw-loss record"];
    run_in_new_competition(async |gui| {
        gui.expect_checked("metrics", &ALL).await?;

        gui.toggle("Points").await?;

        gui.expect_checked("metrics", &["Sonneborn-Berger", "Win-draw-loss record"])
            .await?;
        gui.expect_focus_on_checkbox("Points").await?;
        gui.toggle("Sonneborn-Berger").await?;
        gui.expect_checked("metrics", &["Win-draw-loss record"])
            .await?;
        gui.toggle("Points").await?;
        gui.toggle("Sonneborn-Berger").await?;
        gui.expect_checked("metrics", &ALL).await?;
        gui.expect_focus_on_checkbox("Sonneborn-Berger").await
    })
    .await;
}

/// A metric calculated since a criterion ranks by it names that criterion.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_metric_implied_names_what_needs_it() {
    run_in_new_competition(async |gui| {
        gui.choose_plugin("ranking", "More Buchholz").await?;

        gui.expect_checked(
            "metrics",
            &[
                "Buchholz, needed by more Buchholz",
                "Points",
                "Sonneborn-Berger",
                "Win-draw-loss record",
            ],
        )
        .await
    })
    .await;
}

/// The focus stays with what the operator works on: the list a plugin was added from, the
/// plugin moved, even to the top, and the plugin after the one removed.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_criteria_of_the_ranking_are_added_and_moved() {
    run_in_new_competition(async |gui| {
        gui.choose_plugin("ranking", "More wins").await?;
        gui.expect_list(
            "ranking",
            &["More points", "More Sonneborn-Berger", "More wins"],
        )
        .await?;
        gui.expect_focus_on_choice("ranking").await?;

        gui.click_labelled("Move more wins up").await?;
        gui.click_labelled("Move more wins up").await?;

        gui.expect_list(
            "ranking",
            &["More wins", "More points", "More Sonneborn-Berger"],
        )
        .await?;
        gui.expect_focus_in_list("ranking", "More wins").await?;
        gui.click_labelled("Remove more points").await?;
        gui.expect_list("ranking", &["More wins", "More Sonneborn-Berger"])
            .await?;
        gui.expect_focus_in_list("ranking", "More Sonneborn-Berger")
            .await?;
        gui.click_labelled("Remove more Sonneborn-Berger").await?;
        gui.expect_focus_in_list("ranking", "More wins").await
    })
    .await;
}

/// The format chosen, the focus is back at its choice rather than on the list that closed.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_focus_is_back_at_the_format_once_it_is_chosen() {
    run_in_new_competition(async |gui| {
        gui.choose_format("Single elimination").await?;

        gui.expect_focus_on_choice("format").await
    })
    .await;
}

/// The click after choosing a format reaches what it is on, rather than the list just closed.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_click_after_choosing_a_format_is_not_lost() {
    run_in_new_competition(async |gui| {
        gui.choose_format("Single elimination").await?;
        gui.expect_ticked("Match for third place", true).await?;

        gui.toggle("Match for third place").await?;

        gui.expect_ticked("Match for third place", false).await?;
        gui.expect_list("ranking", &["Later first loss"]).await
    })
    .await;
}

/// Escape closes the list of a choice, and the focus is back at the choice.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn escape_closes_the_list_of_a_choice() {
    run_in_new_competition(async |gui| {
        gui.open_choice("format").await?;

        gui.press(Key::Escape).await?;

        gui.expect_choice_open("format", false).await?;
        gui.expect_focus_on_choice("format").await
    })
    .await;
}

/// Escape in the list of a choice in the dialog closes only the list, so the edits stay.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn escape_in_a_choice_of_the_dialog_leaves_the_dialog_open() {
    run_in_new_competition(async |gui| {
        start_round_robin(&gui).await?;
        gui.click("Phase").await?;
        gui.click("Edit phase").await?;
        gui.click_labelled("Move more Sonneborn-Berger up").await?;
        gui.open_choice("format").await?;

        gui.press(Key::Escape).await?;

        gui.expect_choice_open("format", false).await?;
        gui.expect_focus_on_choice("format").await?;
        gui.expect_list("ranking", &["More Sonneborn-Berger", "More points"])
            .await?;
        gui.press(Key::Escape).await?;
        gui.expect_no_dialog().await
    })
    .await;
}

/// The option under the pointer, or reached by the keyboard, stands out from the list.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_option_highlighted_stands_out() {
    run_in_new_competition(async |gui| {
        gui.open_choice("ranking").await?;

        gui.hover_option("More wins").await?;
        gui.expect_highlighted_option_stands_out().await?;
        gui.press(Key::Down).await?;
        gui.expect_highlighted_option_stands_out().await
    })
    .await;
}

/// A phase ranked by nothing is added, but the form says what that leads to first.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_ranking_of_nothing_is_warned_about() {
    run_in_new_competition(async |gui| {
        gui.click_labelled("Remove more points").await?;
        gui.click_labelled("Remove more Sonneborn-Berger").await?;

        gui.expect_warnings(&["Ranked by nothing, every entrant is tied in the standings."])
            .await?;
        gui.page.check_accessibility().await?;
        gui.click("Add phase").await?;
        gui.expect_phase(&[
            "Round robin",
            "(none)",
            "Win-draw-loss record",
            "Points",
            "Sonneborn-Berger",
        ])
        .await
    })
    .await;
}

/// Undoing the phase brings back the form as it was when the phase was added, so that adding it
/// again adds the same phase.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn undoing_the_phase_brings_back_its_form() {
    run_in_new_competition(async |gui| {
        gui.choose_format("Monster DYP").await?;
        gui.enter("format.rounds", "7").await?;
        gui.click("Add phase").await?;

        gui.click("Undo").await?;

        gui.expect_entered("format.rounds", "7").await?;
        gui.expect_list("ranking", &["More points"]).await
    })
    .await;
}

/// Numbers are read by the form: with a comma before the decimals as well, and what is not read
/// as a number is named.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_number_is_read_with_a_comma_and_what_is_none_is_named() {
    run_in_new_competition(async |gui| {
        gui.enter("metrics[1].points_for_win", "2,5").await?;
        gui.expect_entered("metrics[1].points_for_win", "2.5")
            .await?;
        gui.expect_issues(&[]).await?;

        gui.enter("metrics[1].points_for_win", "abc").await?;

        gui.expect_issues(&["“abc” is no number."]).await?;
        gui.expect_entered("metrics[1].points_for_win", "abc").await
    })
    .await;
}

/// In German, a number is written with a comma before its decimals.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_number_is_shown_with_a_comma_in_german() {
    run_in_new_competition(async |gui| {
        gui.choose_language("Deutsch").await?;

        gui.enter("metrics[1].points_for_win", "2.5").await?;

        gui.expect_entered("metrics[1].points_for_win", "2,5").await
    })
    .await;
}

/// What is said about a list or a plugin in it shows while the pointer is on its button, and from a
/// click on it until the next; Escape hides it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn what_is_said_about_a_plugin_shows_on_hover_and_after_a_click() {
    const POINTS: &str = "Scored for every win, every draw and every bye.";
    run_in_new_competition(async |gui| {
        gui.expect_toggletip(None).await?;

        gui.hover_labelled("About order of the standings").await?;

        gui.expect_toggletip(Some(
            "Best first, every criterion only breaks the ties of the ones before it.",
        ))
        .await?;
        gui.press(Key::Escape).await?;
        gui.expect_toggletip(None).await?;
        // a criterion says when to rank by it, rather than what its metric is
        gui.hover_labelled("About more points").await?;
        gui.expect_toggletip(Some(
            "Usually the first criterion; the points set what each result is worth.",
        ))
        .await?;
        gui.hover_labelled("About format").await?;
        gui.expect_toggletip(Some("How the matches are scheduled."))
            .await?;
        gui.click_labelled("About points").await?;
        gui.expect_toggletip(Some(POINTS)).await?;
        gui.page.check_accessibility().await?;
        gui.click_labelled("About points").await?;
        gui.expect_toggletip(None).await?;
        gui.click_labelled("About points").await?;
        gui.expect_toggletip(Some(POINTS)).await?;
        gui.press(Key::Escape).await?;
        gui.expect_toggletip(None).await
    })
    .await;
}

/// The options of a plugin show their labels only, and the description of the one under the
/// pointer below them.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_description_of_an_option_shows_while_the_pointer_is_on_it() {
    run_in_new_competition(async |gui| {
        gui.open_choice("ranking").await?;

        gui.hover_option("Fewer matches played").await?;

        gui.expect_option_description("The same result out of fewer matches ranks higher.")
            .await?;
        gui.page.check_accessibility().await?;
        gui.hover_option("Later first loss").await?;
        gui.expect_option_description("By the round of the first loss; never having lost is best.")
            .await?;
        gui.hover_option("More Buchholz").await?;
        gui.expect_option_description("A tie break favouring those who met stronger opponents.")
            .await
    })
    .await;
}

/// A touch screen shows the description of an option by a tap on its ⓘ, which does not choose it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_tap_on_the_info_of_an_option_shows_its_description_without_choosing_it() {
    run_in_new_competition(async |gui| {
        gui.open_choice("ranking").await?;

        gui.tap_option_info("Fewer matches played").await?;

        gui.expect_option_description_tapped_open(
            "The same result out of fewer matches ranks higher.",
        )
        .await?;
        gui.page.check_accessibility().await?;
        gui.expect_list("ranking", &["More points", "More Sonneborn-Berger"])
            .await?;
        gui.choose_option("Fewer matches played").await?;
        gui.expect_list(
            "ranking",
            &[
                "More points",
                "More Sonneborn-Berger",
                "Fewer matches played",
            ],
        )
        .await
    })
    .await;
}

/// The dialog cuts off what reaches out of it, so what is said about a field at its right and
/// bottom edges shows to the left of its button and above it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn what_is_said_about_a_field_at_the_right_of_the_dialog_shows_whole() {
    run_in_new_competition(async |gui| {
        start_round_robin(&gui).await?;
        gui.click("Phase").await?;
        gui.click("Edit phase").await?;

        gui.hover_labelled("About weight of an opponent played to a draw")
            .await?;

        gui.expect_toggletip(Some("The points of that opponent are multiplied by this."))
            .await
    })
    .await;
}

/// After the start, the phase is a view of its own, where it is edited in a dialog.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_phase_is_edited_after_the_start_in_a_dialog() {
    run_in_new_competition(async |gui| {
        start_round_robin(&gui).await?;
        gui.click_labelled("Alice wins").await?;
        gui.click("Phase").await?;
        gui.expect_view("Phase").await?;

        gui.click("Edit phase").await?;
        // at the first field, rather than on what is said about it, which would cover the format's
        gui.expect_focus_on_choice("format").await?;
        gui.click_labelled("Move more Sonneborn-Berger up").await?;
        gui.page.check_accessibility().await?;
        gui.click("Save changes").await?;

        gui.expect_no_dialog().await?;
        gui.expect_announcement("The phase is changed. The result reported stays.")
            .await?;
        gui.expect_phase(&[
            "Round robin",
            "More Sonneborn-Berger, more points",
            "Win-draw-loss record",
            "Points",
            "Sonneborn-Berger",
        ])
        .await?;
        gui.click("Undo").await?;
        gui.expect_phase(&[
            "Round robin",
            "More points, more Sonneborn-Berger",
            "Win-draw-loss record",
            "Points",
            "Sonneborn-Berger",
        ])
        .await?;
        gui.click("Matches & standings").await?;
        gui.expect_results(&["Alice won"]).await
    })
    .await;
}

/// The teams of a Monster DYP are pairs, so it never schedules the match of single entrants whose
/// result was reported.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn an_edit_a_result_does_not_fit_is_refused_in_the_dialog() {
    run_in_new_competition(async |gui| {
        start_round_robin(&gui).await?;
        gui.click_labelled("Alice wins").await?;
        gui.click("Phase").await?;
        gui.click("Edit phase").await?;

        gui.choose_format("Monster DYP").await?;
        gui.click("Save changes").await?;

        gui.expect_dialog_error(
            "The phase changed this way does not play match 1, which has a result; reopen that \
             match first to change the phase this way.",
        )
        .await?;
        gui.click("Cancel").await?;
        gui.expect_no_dialog().await?;
        gui.expect_phase(&[
            "Round robin",
            "More points, more Sonneborn-Berger",
            "Win-draw-loss record",
            "Points",
            "Sonneborn-Berger",
        ])
        .await
    })
    .await;
}

/// A round robin pairs its first round as a knockout does, so the results of a knockout's first
/// round stay when it is changed to a round robin.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_knockout_changed_to_a_round_robin_keeps_its_first_results() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob", "Carol", "Dave"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Single elimination").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        gui.click("Bracket").await?;
        gui.win_in_bracket("1").await?;
        gui.win_in_bracket("2").await?;
        gui.expect_bracket_waiting(&[]).await?;
        gui.click("Phase").await?;
        gui.click("Edit phase").await?;

        gui.choose_format("Round robin").await?;
        gui.click("Save changes").await?;

        gui.expect_no_dialog().await?;
        gui.expect_announcement("The phase is changed. The 2 results reported stay.")
            .await?;
        gui.expect_phase(&[
            "Round robin",
            "More points, more Sonneborn-Berger",
            "Win-draw-loss record",
            "Points",
            "Sonneborn-Berger",
        ])
        .await?;
        gui.click("Matches & standings").await?;
        gui.choose_round("Round 1").await?;
        gui.expect_finished(&["1", "2"]).await
    })
    .await;
}

/// The round the operator chose stays shown while they edit the phase in its view, rather than
/// the first open round.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_round_chosen_stays_shown_after_the_phase_is_edited() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob", "Carol", "Dave"] {
            gui.add_entrant(name).await?;
        }
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        gui.choose_round("Round 2 (2 open)").await?;
        gui.expect_matches(&["3", "4"]).await?;
        gui.click("Phase").await?;
        gui.click("Edit phase").await?;

        gui.click_labelled("Move more Sonneborn-Berger up").await?;
        gui.click("Save changes").await?;

        gui.expect_no_dialog().await?;
        gui.click("Matches & standings").await?;
        gui.expect_matches(&["3", "4"]).await
    })
    .await;
}
