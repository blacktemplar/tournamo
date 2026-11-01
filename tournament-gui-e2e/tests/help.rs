//! The help, which the footer of every screen links to beside the source code.

use tournament_gui_e2e::{
    run, run_in_new_competition,
    thirtyfour::{By, extensions::query::ElementQueryable as _},
};

const REPOSITORY: &str = "https://github.com/blacktemplar/tournamo";

/// The help says that the competitions stay in the browser, leads to the source code and names the
/// media owner (the one the gui-e2e hook builds the page with); it is a page of its own, kept by a
/// reload, with the way back to the competitions.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_help_says_where_the_competitions_are_kept() {
    run_in_new_competition(async |gui| {
        gui.follow("Help").await?;

        gui.page.expect_title("Help – Tournamo").await?;
        gui.page
            .expect_texts(
                By::Css("main h2"),
                &[
                    "Running a competition",
                    "Formats and standings",
                    "Your data stays on your device",
                    "Offline use and installation",
                    "Open source",
                ],
            )
            .await?;
        let source = gui.driver.query(By::Css("main a")).first().await?;
        assert_eq!(source.attr("href").await?.as_deref(), Some(REPOSITORY));
        gui.page
            .expect_texts(
                By::Css("main section:last-child p:last-child"),
                &["Media owner (disclosure under § 25 MedienG): Jane Doe, Vienna, Austria."],
            )
            .await?;
        gui.page.check_accessibility().await?;

        gui.page.reload().await?;
        gui.page.expect_title("Help – Tournamo").await?;

        gui.follow("All competitions").await?;
        gui.expect_competitions(&["Cup"]).await
    })
    .await;
}

/// The list of competitions says what the page is for and links to how a competition is run; the
/// help explains every format, criterion and metric by its label and description, and a link to
/// a section of it opens it there.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_help_explains_how_a_competition_is_run() {
    run(async |gui| {
        gui.follow("How it works").await?;

        gui.page.expect_title("Help – Tournamo").await?;
        gui.page.expect_in_view("#help-running").await?;
        gui.expect_explained("Round robin", "Everyone plays everyone.")
            .await?;
        gui.expect_explained(
            "More Sonneborn-Berger",
            "A tie break favouring those who beat or drew stronger opponents.",
        )
        .await?;
        gui.expect_explained(
            "Win-draw-loss record",
            "How many matches were won, drawn and lost.",
        )
        .await?;
        gui.page.check_accessibility().await?;
        gui.page.open_at("help-formats").await?;
        gui.page.expect_in_view("#help-formats").await
    })
    .await;
}

/// Every screen's footer links to the source code.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_footer_links_to_the_source_code() {
    run(async |gui| {
        let source = gui
            .driver
            .query(By::LinkText("Source code"))
            .first()
            .await?;
        assert_eq!(source.attr("href").await?.as_deref(), Some(REPOSITORY));
        Ok(())
    })
    .await;
}
