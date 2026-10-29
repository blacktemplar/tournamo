use tournament_gui_e2e::{Gui, run, run_in_new_competition, thirtyfour::error::WebDriverResult};

const LONG_NAME: &str = "Maximilian-Alexander Müller-Lüdenscheidt";

/// Enters `names`, starts a knockout of them in `format`, and shows its bracket.
async fn show_bracket(gui: &Gui, format: &str, names: &[&str]) -> WebDriverResult<()> {
    for name in names {
        gui.add_entrant(name).await?;
    }
    gui.choose_format(format).await?;
    gui.click("Add phase").await?;
    gui.click("Start").await?;
    gui.click("Bracket").await
}

/// Long names wrap within their pairing, which keeps its column's width, so the lines meet.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_lines_of_the_bracket_meet_whatever_the_names() {
    run_in_new_competition(async |gui| {
        let names = [
            LONG_NAME, "Bo", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi",
        ];
        show_bracket(&gui, "Single elimination", &names).await?;
        gui.win_in_bracket("1").await?;

        gui.expect_bracket_lines_meet().await?;
        gui.page.resize(390, 844).await?;
        gui.expect_bracket_lines_meet().await?;
        gui.page.expect_room_for_words(".pairing .side").await?;
        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}

/// A knockout's view switch has four buttons on a phone, which fit with the German words.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_views_of_a_knockout_fit_a_phone_in_german() {
    run_in_new_competition(async |gui| {
        gui.page.resize(360, 740).await?;
        gui.choose_language("Deutsch").await?;
        for name in ["Alice", "Bob"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("K.-o.-System").await?;
        gui.click("Phase hinzufügen").await?;
        gui.click("Starten").await?;

        gui.click("Phase").await?;

        // those shown on a phone
        gui.page
            .expect_room_for_words(".view-switch button:not(.wide-only)")
            .await?;
        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}

/// The view switch stays below the bar on a phone, hiding what scrolls under it, and what gets
/// the focus is scrolled clear of both.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_view_switch_hides_what_scrolls_under_it() {
    run_in_new_competition(async |gui| {
        gui.page.resize(360, 740).await?;
        for number in 1..=8 {
            gui.add_entrant(&format!("Player {number}")).await?;
        }
        gui.click("Add phase").await?;
        gui.click("Start").await?;

        gui.page.scroll_by(300).await?;

        gui.expect_nothing_shows_around_the_view_switch().await?;
        // the focus moves on to the next match, up to the first if the match was the last
        gui.page.scroll_by(10_000).await?;
        gui.click_labelled("Player 8 wins").await?;
        gui.page.expect_in_view(":focus").await
    })
    .await;
}

/// At 320 px, the narrowest phone, a knockout's views fit, each on one line, in both languages.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_views_of_a_knockout_fit_the_narrowest_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(320, 740).await?;
        for name in ["Alice", "Bob"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Single elimination").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;

        for language in ["English", "Deutsch"] {
            gui.choose_language(language).await?;
            gui.page
                .expect_single_lines(".view-switch button:not(.wide-only)")
                .await?;
            gui.page.expect_no_sideways_scroll().await?;
        }
        Ok(())
    })
    .await;
}

/// The page keeps its width whether it scrolls or not, so the bar does not shift between views.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_page_keeps_its_width_between_views() {
    run_in_new_competition(async |gui| {
        gui.page.resize(1280, 1200).await?;
        for number in 1..=16 {
            gui.add_entrant(&format!("Player {number}")).await?;
        }
        gui.click("Add phase").await?;
        gui.click("Start").await?;
        let (scrolling, matches) = gui.page.scrolling_and_width().await?;
        assert!(scrolling, "the matches of 16 scroll");

        gui.click("Phase").await?;

        let (scrolling, phase) = gui.page.scrolling_and_width().await?;
        assert!(!scrolling, "the phase does not scroll");
        assert_eq!(matches, phase, "the page must keep its width");
        Ok(())
    })
    .await;
}

/// A cross table of 8 is wider than a phone, so it scrolls inside its card, not the page.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_cross_table_scrolls_inside_its_card_on_a_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(360, 740).await?;
        for name in [
            LONG_NAME, "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi",
        ] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Round robin").await?;
        gui.click("Add phase").await?;
        gui.click("Start").await?;

        gui.click("Cross table").await?;

        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}

/// A double elimination of 6 has byes, and a loser bracket whose rounds alternate between
/// dropping losers in and halving.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_lines_of_a_double_elimination_meet() {
    run_in_new_competition(async |gui| {
        let names = ["Alice", "Bob", "Carol", "Dave", "Erin", LONG_NAME];
        show_bracket(&gui, "Double elimination", &names).await?;

        gui.expect_bracket_lines_meet().await
    })
    .await;
}

/// A knockout of 16 has four rounds, which fit next to each other on a tablet held sideways.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_bracket_of_sixteen_fits_on_a_tablet() {
    run_in_new_competition(async |gui| {
        let names: Vec<String> = (1..=16).map(|number| format!("Player {number}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        gui.page.resize(820, 1180).await?;
        show_bracket(&gui, "Single elimination", &names).await?;

        gui.expect_bracket_lines_meet().await?;
        gui.expect_bracket_fits().await?;
        // every match is numbered from the start, so every side not decided names one
        let waiting: Vec<String> = (1..=14)
            .map(|number| format!("Winner of {number}"))
            .chain(["Loser of 14".to_owned(), "Loser of 13".to_owned()])
            .collect();
        let waiting: Vec<&str> = waiting.iter().map(String::as_str).collect();
        gui.expect_bracket_waiting(&waiting).await
    })
    .await;
}

/// Two byes meeting in the loser bracket leave their place empty, and the lines still meet.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_pairing_never_played_keeps_its_place() {
    run_in_new_competition(async |gui| {
        let names = ["Alice", "Bob", "Carol", "Dave", "Erin"];
        show_bracket(&gui, "Double elimination", &names).await?;

        gui.expect_bracket_lines_meet().await?;
        gui.page.check_accessibility().await
    })
    .await;
}

/// The list of entrants grows with the page, the field adding one below it staying in sight, and
/// the bar with undo stays on top.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_field_of_a_new_entrant_and_the_bar_stay_in_sight_on_a_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(390, 844).await?;

        for number in 1..=20 {
            gui.add_entrant(&format!("Player {number}")).await?;
        }

        gui.page.expect_in_view("#entrant-name").await?;
        gui.page.expect_in_view(".bar-actions").await
    })
    .await;
}

/// A match with a result takes one line where there is room, and on a phone keeps the line for its
/// pairing.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_match_with_a_result_breaks_no_name_within_a_word_on_a_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(390, 844).await?;
        for name in ["Player 1", "Player 2", "Player 3", "Player 4"] {
            gui.add_entrant(name).await?;
        }
        gui.click("Add phase").await?;
        gui.click("Start").await?;

        gui.click("Draw").await?;

        gui.page.expect_room_for_words(".match .side").await?;
        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}

/// On the narrowest phone, a match to play stacks its sides, each name on a line of its own
/// beside its button, so that no name breaks within a word, and every button keeps its word, in
/// German too.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_match_to_play_breaks_no_name_within_a_word_on_the_narrowest_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(320, 740).await?;
        for name in ["Zimmermann", "Hohenzollern-Sigmaringen", "Dieter", "Bo"] {
            gui.add_entrant(name).await?;
        }
        gui.click("Add phase").await?;
        gui.click("Start").await?;

        gui.page.expect_room_for_words(".match .side").await?;
        gui.page.expect_no_sideways_scroll().await?;
        gui.choose_language("Deutsch").await?;
        gui.page.resize(400, 740).await?;
        gui.page.expect_room_for_words(".match button").await
    })
    .await;
}

/// A match played recedes behind those to play, but stays a card of its own, with a solid border,
/// and its loser recedes behind its winner.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_winner_of_a_match_stands_out_from_its_loser() {
    run_in_new_competition(async |gui| {
        start_round_robin_of(&gui, &["Alice", "Bob"]).await?;

        gui.click_labelled("Alice wins").await?;

        gui.expect_finished_card_looks_played().await
    })
    .await;
}

/// Enters `names` and starts a round robin of them.
async fn start_round_robin_of(gui: &Gui, names: &[&str]) -> WebDriverResult<()> {
    for name in names {
        gui.add_entrant(name).await?;
    }
    gui.click("Add phase").await?;
    gui.click("Start").await
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_standings_scroll_inside_their_card_on_a_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(360, 740).await?;
        gui.choose_language("Deutsch").await?;
        for name in [LONG_NAME, "Bob", "Carol"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Jeder gegen jeden").await?;
        gui.click("Phase hinzufügen").await?;
        gui.click("Starten").await?;

        gui.click("Tabelle").await?;

        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn entrants_break_no_word_before_the_start_on_a_tablet() {
    run_in_new_competition(async |gui| {
        gui.page.resize(768, 1024).await?;

        gui.add_entrant(LONG_NAME).await?;
        gui.add_entrant("Zoë Ångström").await?;

        gui.page
            .expect_room_for_words("#entrant-list .name")
            .await?;
        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_bar_keeps_room_for_the_competition_in_german() {
    run(async |gui| {
        gui.new_competition("Stadtmeisterschaft 2026").await?;
        gui.choose_language("Deutsch").await?;

        // a phone's bar leaves too little room for a long word, which ends in "…" there
        for width in [700, 940, 980] {
            gui.page.resize(width, 900).await?;
            gui.page.expect_room_for_words(".bar h1").await?;
            gui.page.expect_no_sideways_scroll().await?;
        }
        gui.page.resize(360, 900).await?;
        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}
