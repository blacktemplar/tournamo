//! The page as an app: offline, installed, and what it says while the browser may not keep what is
//! stored, which headless Chrome refuses to.

use tournament_gui_e2e::{
    COMPETITION, run_in_new_competition,
    thirtyfour::{By, extensions::query::ElementQueryable as _},
};

/// The warning, in English, while the browser offers to install the page, as headless Chrome does.
const NOT_PERSISTED: [&str; 3] = [
    "This browser may not keep the competitions stored here.",
    "For example in a private window, or when it runs short of space. Export them to keep a copy.",
    "Installed as an app, the page is usually allowed to keep them.",
];

/// At a venue without a network, a page opened before still opens, with what is stored.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_page_opened_before_opens_offline() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.page.expect_kept_offline().await?;

        gui.page.go_offline().await;
        gui.page.reload().await?;

        gui.expect_open(COMPETITION).await?;
        gui.expect_entrants(&["Alice"]).await?;
        gui.add_entrant("Bob").await?;
        gui.expect_entrants(&["Alice", "Bob"]).await
    })
    .await;
}

/// The tab names the competition shown, in plain text without the marks isolating it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_tab_is_titled_by_the_competition() {
    run_in_new_competition(async |gui| {
        gui.page.expect_title("Cup – Tournamo").await?;

        gui.follow("All competitions").await?;

        gui.page.expect_title("Tournamo").await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_page_can_be_installed() {
    run_in_new_competition(async |gui| gui.page.expect_installable().await).await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_warning_that_the_browser_may_not_keep_competitions_offers_to_export() {
    run_in_new_competition(async |gui| {
        gui.unfold_storage_warning().await?;
        gui.expect_storage_warning(&NOT_PERSISTED).await?;

        gui.click("Export this competition").await?;

        gui.page.expect_download("Cup.json").await?;
        Ok(())
    })
    .await;
}

/// The warning takes one line, which unfolds it, so that it leaves a phone's screen to the
/// competition; unfolded, it stays so on the next visit, until folded again.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_warning_that_the_browser_may_not_keep_competitions_takes_one_line() {
    run_in_new_competition(async |gui| {
        let first_line = || async {
            gui.page
                .driver
                .query(By::Css(".warning summary"))
                .first()
                .await?
                .click()
                .await
        };
        gui.expect_storage_warning(&NOT_PERSISTED[..1]).await?;

        first_line().await?;

        gui.expect_storage_warning(&NOT_PERSISTED).await?;
        gui.page.reload().await?;
        gui.expect_open(COMPETITION).await?;
        gui.expect_storage_warning(&NOT_PERSISTED).await?;
        first_line().await?;
        gui.expect_storage_warning(&NOT_PERSISTED[..1]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_warning_that_the_browser_may_not_keep_competitions_can_be_dismissed_for_good() {
    run_in_new_competition(async |gui| {
        gui.unfold_storage_warning().await?;

        gui.click("Don’t show again").await?;

        gui.expect_storage_warning(&[]).await?;
        gui.page.reload().await?;
        gui.expect_open(COMPETITION).await?;
        gui.expect_storage_warning(&[]).await
    })
    .await;
}
