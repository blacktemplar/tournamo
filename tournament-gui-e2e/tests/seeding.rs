//! Ratings, which the start orders the registration by: a knockout seeded by them, groups dealt
//! by them, and the best rated going straight to a main phase.

use tournament_gui_e2e::{Gui, run_in_new_competition, thirtyfour::error::WebDriverResult};

const EIGHT: [&str; 8] = [
    "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi",
];

/// Adds the entrants `EIGHT`, Alice rated best, then each one less, and Heidi worst.
async fn eight_rated(gui: &Gui) -> WebDriverResult<()> {
    for name in EIGHT {
        gui.add_entrant(name).await?;
    }
    gui.show_ratings().await?;
    for (name, rating) in EIGHT.iter().zip((1..=8).rev()) {
        gui.rate_entrant(name, &(rating * 100).to_string()).await?;
    }
    gui.expect_seeds(&["#1", "#2", "#3", "#4", "#5", "#6", "#7", "#8"])
        .await
}

/// The best rated meets the worst rated in the first round of a knockout, the second the third.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_knockout_pairs_the_best_rated_with_the_worst() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob", "Carol", "Dave"] {
            gui.add_entrant(name).await?;
        }
        gui.show_ratings().await?;
        gui.rate_entrant("Alice", "1500").await?;
        gui.rate_entrant("Bob", "1800").await?;
        gui.rate_entrant("Carol", "1600").await?;
        gui.rate_entrant("Dave", "2000").await?;
        gui.expect_seeds(&["#4", "#2", "#3", "#1"]).await?;
        // the rows stay in the order they were added
        gui.expect_entrants(&["Alice", "Bob", "Carol", "Dave"])
            .await?;
        gui.page.check_accessibility().await?;
        gui.choose_format("Single elimination").await?;
        gui.click("Add phase").await?;

        gui.click("Start").await?;

        gui.expect_pairings(&[("Dave", "Alice"), ("Bob", "Carol")])
            .await
    })
    .await;
}

/// Pasted lines give ratings after a `;`, which show by themselves and stay after a reload; a
/// rating that is no number stays shown with why.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn pasted_ratings_are_kept_and_a_wrong_one_is_pointed_out() {
    run_in_new_competition(async |gui| {
        gui.paste_entrant_names("Alice;1500\nBob;1700\nCarol\n")
            .await?;
        gui.expect_entrants(&["Alice", "Bob", "Carol"]).await?;
        gui.expect_ratings(&["1500", "1700", ""]).await?;
        gui.expect_seeds(&["#2", "#1", ""]).await?;

        gui.rate_entrant("Carol", "strong").await?;
        gui.expect_field_error(
            "entrant-rating-2",
            "“strong” is no rating: a rating is a number, such as 1500.",
        )
        .await?;
        gui.expect_ratings(&["1500", "1700", "strong"]).await?;
        gui.page.check_accessibility().await?;
        gui.rate_entrant("Carol", "1600").await?;
        gui.expect_seeds(&["#3", "#1", "#2"]).await?;

        gui.page.reload().await?;
        gui.expect_open("Cup").await?;
        gui.expect_ratings(&["1500", "1700", "1600"]).await?;
        gui.expect_seeds(&["#3", "#1", "#2"]).await
    })
    .await;
}

/// Two groups dealt back and forth take the registration by rating: the best and the fourth in
/// one, the second and the third in the other, and so on.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn groups_are_dealt_the_rated_registration_back_and_forth() {
    run_in_new_competition(async |gui| {
        eight_rated(&gui).await?;
        gui.choose_format("Round robin").await?;
        gui.enter("divisions", "2").await?;
        gui.choose_level("Expert").await?;
        gui.choose_plugin("draw", "Back and forth").await?;
        gui.click("Add phase").await?;

        gui.click("Start").await?;

        gui.expect_standings_entrants(Some('A'), &["Alice", "Dave", "Erin", "Heidi"])
            .await?;
        gui.expect_standings_entrants(Some('B'), &["Bob", "Carol", "Frank", "Grace"])
            .await
    })
    .await;
}

/// The best four rated go straight to the main phase, the others play a qualification for the
/// other four places of it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_best_rated_go_straight_to_the_main_phase() {
    run_in_new_competition(async |gui| {
        eight_rated(&gui).await?;
        gui.type_phase_name("Qualification").await?;
        gui.choose_format("Round robin").await?;
        gui.click("Only some places").await?;
        gui.enter_place(0, "from", "5").await?;
        gui.expect_source_warnings(&[]).await?;
        gui.click("Add phase").await?;
        gui.open_new_phase("Main").await?;
        gui.choose_format("Single elimination").await?;
        // the best four of the registration first, then the best four of the qualification
        gui.choose_source(0, "the registration").await?;
        gui.enter_place(0, "to", "4").await?;
        gui.click("Add source").await?;
        gui.enter_place(1, "to", "4").await?;
        gui.page.check_accessibility().await?;
        gui.click("Add phase").await?;

        gui.click("Start").await?;
        gui.expect_standings_entrants(None, &["Erin", "Frank", "Grace", "Heidi"])
            .await?;
        // the first-listed side wins every match
        for number in 1..=6 {
            gui.enter_result(&format!("{number} 1")).await?;
        }
        gui.click("Start Main").await?;
        gui.expect_bar_phase(Some("Main")).await?;
        // the best rated, seeded first, meet the qualifiers in the first round
        gui.expect_standings_entrants(None, &EIGHT).await?;
        gui.page
            .expect_script(
                "const best = ['Alice', 'Bob', 'Carol', 'Dave'];
                const matches = [...document.querySelectorAll('#match-list li.match')]
                    .map((match) => [...match.querySelectorAll('.side')]
                        .filter((side) => best.includes(
                            side.textContent.replace(/[\\u2068\\u2069]/g, '').trim())).length);
                return matches.length === 4 && matches.every((count) => count === 1)
                    ? null : `the best rated per match are ${matches}`;",
                Vec::new(),
                "every match of the first round must pair one of the best rated with a qualifier",
            )
            .await
    })
    .await;
}
