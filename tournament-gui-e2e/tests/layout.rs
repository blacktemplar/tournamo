use tournament_gui_e2e::{
    Gui, run, run_in_new_competition,
    thirtyfour::{By, error::WebDriverResult},
};

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

/// Of twenty entrants, twelve move on by a bye: their pairings are left out, each tagged in the
/// next round instead, and the round of 32 takes the eight rows of the round of 16, so the bracket
/// fits the height of a laptop's window, the lines still meeting.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_bye_is_a_tag_rather_than_a_pairing() {
    run_in_new_competition(async |gui| {
        let names: Vec<String> = (1..=20).map(|number| format!("Player {number}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        show_bracket(&gui, "Single elimination", &names).await?;

        gui.expect_bracket_lines_meet().await?;
        gui.page
            .expect_texts(By::Css(".pairing .tag"), &["bye"; 12])
            .await?;
        let first_column = ".bracket-column:first-child > .bracket-slots > ol > li";
        assert_eq!(gui.page.rects_of(first_column).await?.len(), 8);
        let height = gui.page.rects_of(".bracket").await?[0].height();
        assert!(
            height < 900.0,
            "the bracket must be lower than the window, but is {height}px high"
        );
        gui.page.check_accessibility().await
    })
    .await;
}

/// On a phone, the bracket's edge fades out where more of it is out of sight, hinting at the
/// rounds to the right, and at the first round once scrolled to the last.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_edges_of_a_bracket_scrolling_fade_out() {
    run_in_new_competition(async |gui| {
        let names = [
            "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi",
        ];
        gui.page.resize(390, 844).await?;
        show_bracket(&gui, "Single elimination", &names).await?;
        gui.expect_bracket_fades(false, true).await?;

        gui.scroll_bracket_to_the_end().await?;

        gui.expect_bracket_fades(true, false).await
    })
    .await;
}

/// A knockout of 64 is higher than the screen, so it scrolls within a box as high as the screen
/// below the view switch, the names of its stages staying on top.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_stages_of_a_high_bracket_stay_on_top() {
    run_in_new_competition(async |gui| {
        let names: Vec<String> = (1..=64).map(|number| format!("Player {number}")).collect();
        gui.paste_entrant_names(&names.join("\n")).await?;
        show_bracket(&gui, "Single elimination", &[]).await?;

        gui.expect_bracket_stages_stay_on_top().await?;
        gui.expect_bracket_lines_meet().await?;
        gui.page.check_accessibility().await
    })
    .await;
}

/// The pairings of byes in the first rounds of a double elimination are left out, their places
/// kept, and the lines still meet.
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

/// On a phone the standings show what decides the rank right after the names, and what only
/// breaks ties after it, where the table scrolls; before an entrant plays, its metrics recede.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_standings_show_what_ranks_first_on_a_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(360, 740).await?;
        start_round_robin_of(&gui, &[LONG_NAME, "Bob", "Carol", "Dave"]).await?;
        gui.click_labelled("Bob wins").await?;

        gui.click("Standings").await?;

        gui.expect_standings_headers(&["Rank", "Name", "Points", "SB", "W-D-L", "Played"])
            .await?;
        gui.page
            .expect_in_view("#standing-list thead th:nth-child(3)")
            .await?;
        gui.expect_metrics_muted_until_played().await?;
        gui.page.expect_no_sideways_scroll().await
    })
    .await;
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

/// The bar shows the labels of its buttons down to a small tablet, and only their icons on a
/// phone, where the competition's name takes two lines rather than being cut off.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_bar_shows_its_labels_until_a_phone() {
    run(async |gui| {
        gui.new_competition("Friday Night Darts").await?;

        gui.page.resize(820, 900).await?;
        let labels = gui.page.rects_of(".bar-actions .bar-label").await?;
        assert!(labels.iter().all(|label| label.width() > 1.0), "{labels:?}");

        gui.page.resize(390, 844).await?;
        let labels = gui.page.rects_of(".bar-actions .bar-label").await?;
        assert!(
            labels.iter().all(|label| label.width() <= 1.0),
            "{labels:?}"
        );
        gui.page.expect_room_for_words(".bar h1").await?;
        let size = gui
            .driver
            .execute(
                "const title = document.querySelector('.bar h1');
                return [title.scrollHeight, title.clientHeight,
                        title.scrollWidth, title.clientWidth];",
                Vec::new(),
            )
            .await?
            .convert::<Vec<f64>>()?;
        // A fractional line height can round these integer heights a pixel apart.
        assert!(
            size[0] <= size[1] + 1.0 && size[2] <= size[3],
            "the name must show whole: {size:?}"
        );
        gui.page.expect_no_sideways_scroll().await
    })
    .await;
}

/// On a wide screen, the bar's content lines up with the page's below it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_bar_lines_up_with_the_page_on_a_wide_screen() {
    run_in_new_competition(async |gui| {
        gui.page.resize(1920, 1000).await?;

        let back = gui.page.rects_of(".bar .back").await?[0];
        let actions = gui.page.rects_of(".bar-actions button").await?;
        let cards = gui.page.rects_of("main .card").await?;

        let left = cards
            .iter()
            .map(|card| card.left)
            .fold(f64::INFINITY, f64::min);
        let right = cards.iter().map(|card| card.right).fold(0.0, f64::max);
        assert!((back.left - left).abs() <= 1.0, "{back:?}, {cards:?}");
        let last = actions.last().unwrap();
        assert!((last.right - right).abs() <= 1.0, "{last:?}, {cards:?}");
        Ok(())
    })
    .await;
}

/// The relative luminance of the CSS colour `color`, e.g. `"rgb(17, 20, 24)"`, as WCAG defines it.
fn luminance(color: &str) -> f64 {
    let channels: Vec<f64> = color
        .trim_start_matches("rgba(")
        .trim_start_matches("rgb(")
        .trim_end_matches(')')
        .split(',')
        .take(3)
        .map(|channel| channel.trim().parse::<f64>().unwrap() / 255.0)
        .map(|channel| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        })
        .collect();
    0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
}

/// In the dark theme, a card stands out from the page behind it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_cards_stand_out_from_the_page_in_the_dark_theme() {
    run_in_new_competition(async |gui| {
        gui.choose_theme("Dark").await?;

        let card = luminance(&gui.page.style_of(".card", "background-color").await?);
        let page = luminance(&gui.page.style_of("body", "background-color").await?);

        let contrast = (card + 0.05) / (page + 0.05);
        assert!(contrast >= 1.2, "the contrast is {contrast}");
        Ok(())
    })
    .await;
}

/// The names of the sides of a match in the bracket are buttons, in the text's colour, rather
/// than links.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_names_in_the_bracket_look_like_buttons() {
    run_in_new_competition(async |gui| {
        show_bracket(
            &gui,
            "Single elimination",
            &["Alice", "Bob", "Carol", "Dave"],
        )
        .await?;

        let name = ".pairing button.side";
        assert_eq!(
            gui.page.style_of(name, "text-decoration-line").await?,
            "none"
        );
        assert_eq!(
            gui.page.style_of(name, "color").await?,
            gui.page.style_of("body", "color").await?
        );
        Ok(())
    })
    .await;
}

/// Under a finger, a checkbox is big, and its row as high as every control.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_checkbox_is_big_enough_to_tap() {
    run_in_new_competition(async |gui| {
        gui.page.emulate_touch_screen().await?;

        let checkboxes = gui
            .page
            .rects_of("#field-metrics input[type=checkbox]")
            .await?;
        let rows = gui.page.rects_of("#field-metrics > li > label").await?;

        assert!(
            !checkboxes.is_empty() && checkboxes.iter().all(|checkbox| checkbox.width() >= 24.0),
            "{checkboxes:?}"
        );
        assert!(rows.iter().all(|row| row.height() >= 44.0), "{rows:?}");
        Ok(())
    })
    .await;
}

/// The views are tabs, underlined when chosen, unlike the switch in the card of the matches, whose
/// buttons are joined; the round is chosen from a list looking like the choices of the phase.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_views_look_unlike_the_switch_within_a_card() {
    run_in_new_competition(async |gui| {
        start_round_robin_of(&gui, &["Alice", "Bob", "Carol"]).await?;
        gui.click("By round").await?;

        let tab = ".view-switch [aria-pressed=true]";
        let switch = ".card .segmented [aria-pressed=true]";
        assert_eq!(gui.page.style_of(tab, "border-top-width").await?, "0px");
        assert_ne!(gui.page.style_of(tab, "border-bottom-width").await?, "0px");
        assert_ne!(gui.page.style_of(switch, "border-top-width").await?, "0px");
        let round = gui.page.rects_of("#round").await?[0];
        let round_radius = gui.page.style_of("#round", "border-radius").await?;
        gui.click("Phase").await?;
        gui.click("Edit phase").await?;
        gui.expect_focus_on_choice("format").await?;
        // the button opening the choice of the format
        let choice = gui.page.rects_of("#field-format").await?[0];
        assert_eq!(
            gui.page.style_of("#field-format", "border-radius").await?,
            round_radius
        );
        assert!(
            (choice.height() - round.height()).abs() <= 1.0,
            "{choice:?}, {round:?}"
        );
        Ok(())
    })
    .await;
}

/// On a phone the form of the phase keeps its lists flat within their card, rather than boxes in
/// it, and the buttons of a criterion beside its name.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_form_of_the_phase_is_flat_on_a_phone() {
    run_in_new_competition(async |gui| {
        gui.page.resize(390, 844).await?;

        assert_eq!(
            gui.page.style_of(".field.list", "border-top-width").await?,
            "0px"
        );
        let names = gui.page.rects_of("#field-ranking > li > .name").await?;
        let buttons = gui
            .page
            .rects_of("#field-ranking > li > .item-buttons")
            .await?;
        assert_eq!(names.len(), buttons.len());
        for (name, buttons) in names.iter().zip(&buttons) {
            assert!(
                buttons.top < name.bottom && name.top < buttons.bottom,
                "{name:?}, {buttons:?}"
            );
        }
        Ok(())
    })
    .await;
}

/// Removing an entrant is a quiet button, so that the list is no column of outlines, and deleting
/// a competition is quieter than its other actions.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn removing_and_deleting_are_quiet() {
    run(async |gui| {
        gui.new_competition("Cup").await?;
        gui.add_entrant("Alice").await?;

        assert_eq!(
            gui.page
                .style_of("#entrant-list .remove", "border-top-color")
                .await?,
            "rgba(0, 0, 0, 0)"
        );
        gui.follow("All competitions").await?;
        gui.expect_competitions(&["Cup"]).await?;
        // rename, export, delete
        let delete = gui
            .page
            .style_of(".listing-buttons button:last-child", "color")
            .await?;
        let rename = gui
            .page
            .style_of(".listing-buttons button:first-child", "color")
            .await?;
        assert_ne!(delete, rename);
        Ok(())
    })
    .await;
}

/// The field of a new entrant keeps room for a name beside or above the German button's long
/// label.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_field_of_a_new_entrant_keeps_room_in_german() {
    run_in_new_competition(async |gui| {
        gui.choose_language("Deutsch").await?;

        // where the button leaves the field the least room beside it
        for width in [360, 412, 480] {
            gui.page.resize(width, 900).await?;
            let field = gui.page.rects_of("#entrant-name").await?[0];
            assert!(field.width() >= 192.0, "{width}: {field:?}");
        }
        Ok(())
    })
    .await;
}
