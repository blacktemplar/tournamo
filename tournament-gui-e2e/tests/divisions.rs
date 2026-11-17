//! Phases of several divisions: groups shown each on its own, their matches listed together or
//! by group, and the best of the groups going on to a knockout.

use tournament_gui_e2e::{
    Gui, run_in_new_competition,
    thirtyfour::{By, error::WebDriverResult},
};

const ELEVEN: [&str; 11] = [
    "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi", "Ivan", "Judy", "Kim",
];

/// Enters eleven entrants in three groups, played as round robins compared across the groups as
/// suggested, by points per match, then by points, then a knockout of the best eight of the
/// groups.
async fn groups_and_knockout(gui: &Gui) -> WebDriverResult<()> {
    for name in ELEVEN {
        gui.add_entrant(name).await?;
    }
    gui.choose_format("Round robin").await?;
    gui.enter("divisions", "3").await?;
    gui.click("Add phase").await?;
    gui.open_new_phase("Knockout").await?;
    gui.choose_format("Single elimination").await?;
    gui.choose_plugin("seeding", "Groups kept apart").await?;
    gui.enter_place(0, "to", "8").await?;
    gui.expect_source_in_groups(
        0,
        "With 3 groups side by side, places 1–8 are places 1–2 of each group and the best 2 of \
         those in place 3 of their group, those of one place compared across the groups.",
    )
    .await?;
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
        // place of their group: the cut of the knockout takes the firsts and seconds of the
        // groups by the lot of each group, and goes through the thirds
        for number in 1..=15 {
            gui.enter_result(&format!("{number} x")).await?;
        }
        gui.expect_announcement("Match 15: Draw. Every match of the phase has a result.")
            .await?;
        gui.go_to_phase("1 · Round robin").await?;
        gui.expect_compared(&["Place 3 compared across the groups"], 3)
            .await?;
        // two of the thirds by the comparison, in the groups and in the comparison
        let third = "Place 3 goes on to Knockout as things stand, compared across the groups";
        gui.expect_cuts(&[
            "Places 1–2 go on to Knockout",
            third,
            "Places 1–2 go on to Knockout",
            third,
            "Places 1–2 go on to Knockout",
            "Go on to Knockout as things stand",
        ])
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

/// The group tables show what the groups are ranked by and what that is calculated from, the
/// card comparing a place what the groups are compared by; the operator hides and adds a metric
/// for either while the phase is played.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn each_table_shows_what_it_is_ranked_by_and_what_the_operator_ticks() {
    const WDL_IN_GROUPS: &str = "Show win-draw-loss record in the group tables";
    const BUCHHOLZ_IN_GROUPS: &str = "Show Buchholz in the group tables";
    run_in_new_competition(async |gui| {
        groups_and_knockout(&gui).await?;
        gui.click("Start").await?;
        for number in 1..=15 {
            gui.enter_result(&format!("{number} x")).await?;
        }
        gui.go_to_phase("1 · Round robin").await?;

        // not the points per match, which only the comparison is ranked by
        gui.expect_division_standings_headers(
            'a',
            &["Rank", "Name", "Points", "SB", "W-D-L", "Played"],
        )
        .await?;
        gui.expect_compared_headers(&["Name", "Group", "P/M", "Points", "W-D-L", "Played"])
            .await?;

        gui.click("Phase").await?;
        gui.click("Edit phase").await?;
        gui.page.check_accessibility().await?;
        gui.toggle_named(WDL_IN_GROUPS).await?;
        gui.expect_named_checkbox(WDL_IN_GROUPS, false, false)
            .await?;
        gui.click("Save changes").await?;
        gui.click("Matches & standings").await?;
        gui.expect_division_standings_headers('a', &["Rank", "Name", "Points", "SB", "Played"])
            .await?;
        gui.expect_compared_headers(&["Name", "Group", "P/M", "Points", "W-D-L", "Played"])
            .await?;
        gui.click("Undo").await?;
        gui.expect_division_standings_headers(
            'a',
            &["Rank", "Name", "Points", "SB", "W-D-L", "Played"],
        )
        .await?;

        gui.click("Phase").await?;
        gui.click("Edit phase").await?;
        gui.toggle_named(BUCHHOLZ_IN_GROUPS).await?;
        gui.expect_named_checkbox(BUCHHOLZ_IN_GROUPS, true, false)
            .await?;
        gui.click("Save changes").await?;
        gui.click("Matches & standings").await?;
        gui.expect_division_standings_headers(
            'a',
            &["Rank", "Name", "Points", "SB", "W-D-L", "Bh", "Played"],
        )
        .await?;
        gui.click("Phase").await?;
        gui.click("Edit phase").await?;
        gui.toggle_named(BUCHHOLZ_IN_GROUPS).await?;
        gui.click("Save changes").await?;
        // no longer calculated
        gui.expect_phase(&[
            "Round robin",
            "3",
            "Back and forth",
            "Winner only",
            "More points, more Sonneborn-Berger",
            "Win-draw-loss record",
            "Points",
            "Sonneborn-Berger",
            "Points per match",
            "More points per match, more points",
        ])
        .await
    })
    .await;
}
