use tournament_gui_e2e::{
    COMPETITION, Gui, Key, run, run_in_new_competition,
    thirtyfour::{By, error::WebDriverResult, stringmatch::StringMatch},
};

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_is_kept_in_the_browser() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;

        gui.page.reload().await?;

        gui.expect_open(COMPETITION).await?;
        gui.expect_entrants(&["Alice"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_competitions_stored_are_listed_to_open() {
    run(async |gui| {
        gui.expect_competitions(&[]).await?;
        gui.new_competition("Cup").await?;
        gui.add_entrant("Alice").await?;
        gui.follow("All competitions").await?;
        gui.new_competition("League").await?;
        gui.follow("All competitions").await?;

        // the latest first
        gui.expect_competitions(&["League", "Cup"]).await?;
        gui.expect_progress(&["Not started · 0 entrants", "Not started · one entrant"])
            .await?;
        gui.open_competition("Cup").await?;
        gui.expect_entrants(&["Alice"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_is_renamed_and_deleted_from_the_list() {
    run(async |gui| {
        gui.new_competition("Cup").await?;
        gui.add_entrant("Alice").await?;
        gui.follow("All competitions").await?;
        gui.new_competition("League").await?;
        gui.follow("All competitions").await?;

        // buttons of an icon only, which shows what they do under the pointer
        for action in ["Rename", "Export", "Delete"] {
            gui.expect_titled(&format!("{action} Cup")).await?;
        }
        gui.rename_competition("Cup", "Cup 2026").await?;

        gui.expect_no_dialog().await?;
        gui.expect_competitions(&["League", "Cup 2026"]).await?;
        gui.delete_competition("League").await?;
        gui.expect_competitions(&["Cup 2026"]).await?;
        gui.page.check_accessibility().await?;
        gui.open_competition("Cup 2026").await?;
        gui.expect_entrants(&["Alice"]).await
    })
    .await;
}

/// A name another competition has is pointed out while typed, when creating one and when renaming
/// one, though not the one's own name; it is taken all the same.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_name_another_competition_has_is_pointed_out() {
    run(async |gui| {
        gui.new_competition("Cup").await?;
        gui.follow("All competitions").await?;

        gui.type_competition_name("Cup").await?;

        gui.expect_name_taken(Some("Another competition is named “Cup” already."))
            .await?;
        gui.page.check_accessibility().await?;
        gui.click("Create").await?;
        gui.expect_open("Cup").await?;
        gui.follow("All competitions").await?;
        gui.expect_name_taken(None).await?;
        gui.click_labelled("Rename Cup").await?;
        gui.expect_name_taken(Some("Another competition is named “Cup” already."))
            .await?;
        gui.type_new_name("League").await?;
        gui.expect_name_taken(None).await
    })
    .await;
}

/// Deleting cannot be undone, so the dialog asking opens at Cancel, and a double Enter on the
/// button opening it deletes nothing.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_question_whether_to_delete_opens_at_cancel() {
    run(async |gui| {
        gui.new_competition("Cup").await?;
        gui.follow("All competitions").await?;

        gui.click_labelled("Delete Cup").await?;

        gui.expect_alert_dialog("Delete Cup?").await?;
        gui.expect_focus("Cancel").await?;
        gui.press(Key::Enter).await?;
        gui.expect_no_dialog().await?;
        gui.expect_competitions(&["Cup"]).await
    })
    .await;
}

/// The question whether to delete offers to export the competition first, as it advises.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_question_whether_to_delete_offers_an_export() {
    run(async |gui| {
        gui.new_competition("Cup").await?;
        gui.follow("All competitions").await?;
        gui.click_labelled("Delete Cup").await?;

        gui.click("Export").await?;

        gui.page.expect_download("Cup.json").await?;
        gui.expect_alert_dialog("Delete Cup?").await?;
        gui.page.check_accessibility().await?;
        gui.click("Delete").await?;
        gui.expect_competitions(&[]).await
    })
    .await;
}

/// A competition is created only with a name, which the field says.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_without_a_name_is_refused_at_its_field() {
    run(async |gui| {
        gui.click("Create").await?;

        gui.expect_field_error("competition-name", "A competition needs a name.")
            .await?;
        gui.page.check_accessibility().await
    })
    .await;
}

/// A tab showing a competition would go on saving it, so it is not deleted meanwhile.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_open_in_another_tab_is_not_deleted() {
    run_in_new_competition(async |gui| {
        let showing = gui.page.tab().await?;
        gui.page.open_tab().await?;
        gui.follow("All competitions").await?;

        gui.delete_competition(COMPETITION).await?;

        gui.expect_dialog_error("The competition is open in another tab; close it there first.")
            .await?;
        gui.click("Cancel").await?;
        gui.expect_competitions(&[COMPETITION]).await?;
        gui.page.close_tab(&showing).await?;
        gui.expect_open(COMPETITION).await
    })
    .await;
}

/// Two tabs changing a competition would fork its log, so only one of them does.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_open_in_another_tab_is_only_shown_and_followed() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        let first = gui.page.tab().await?;

        let second = gui.page.open_tab().await?;

        gui.expect_read_only(true).await?;
        gui.expect_entrants(&["Alice"]).await?;
        gui.expect_disabled("Add entrant").await?;
        gui.page.switch_to(&first).await?;
        gui.expect_read_only(false).await?;
        gui.add_entrant("Bob").await?;
        gui.page.switch_to(&second).await?;
        gui.expect_entrants(&["Alice", "Bob"]).await?;
        gui.page.check_accessibility().await?;

        gui.page.switch_to(&first).await?;
        gui.page.close_tab(&second).await?;

        gui.expect_read_only(false).await?;
        gui.add_entrant("Carol").await?;
        gui.expect_entrants(&["Alice", "Bob", "Carol"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn an_exported_competition_is_imported_as_another_one() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.click("Export").await?;
        let file = gui.page.expect_download("Cup.json").await?;
        gui.follow("All competitions").await?;

        gui.import(&file).await?;

        gui.expect_open("Cup (2)").await?;
        gui.expect_entrants(&["Alice"]).await?;
        gui.follow("All competitions").await?;
        // the copy told apart from it, and listed first, as the latest
        gui.expect_competitions(&["Cup (2)", COMPETITION]).await
    })
    .await;
}

/// A file that is no competition is refused in the operator's language, with where reading it
/// stopped folded away below.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_file_that_is_no_competition_is_refused_with_its_detail_apart() {
    run(async |gui| {
        let file =
            std::env::temp_dir().join(format!("not-a-competition-{}.json", std::process::id()));
        std::fs::write(&file, "<html>").unwrap();

        gui.import(&file).await?;

        gui.expect_error("The file is no competition of Tournamo.")
            .await?;
        gui.expect_error_detail("line 1 column 1").await?;
        gui.page.check_accessibility().await?;
        std::fs::remove_file(file).unwrap();
        Ok(())
    })
    .await;
}

/// A file of a newer version, and one whose log does not load, are told apart from a file that is
/// no competition.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_newer_or_damaged_competition_is_refused_for_that() {
    run(async |gui| {
        let newer = std::env::temp_dir().join(format!("newer-{}.json", std::process::id()));
        std::fs::write(&newer, r#"{"format": 2, "log": {}, "people": []}"#).unwrap();
        gui.import(&newer).await?;
        gui.expect_error(
            "The file is of a newer version of Tournamo, which this one cannot open; reload the \
             page for the newest.",
        )
        .await?;
        // another file, since choosing the same one again changes nothing
        let damaged = std::env::temp_dir().join(format!("damaged-{}.json", std::process::id()));
        std::fs::write(&damaged, r#"{"format": 1, "log": [{}], "people": []}"#).unwrap();

        gui.import(&damaged).await?;

        gui.expect_error("The file is a competition of Tournamo, but damaged.")
            .await?;
        std::fs::remove_file(newer).unwrap();
        std::fs::remove_file(damaged).unwrap();
        Ok(())
    })
    .await;
}

/// Why an action was refused shows until the next one is done, not past it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_refusal_is_gone_once_the_next_action_is_done() {
    run(async |gui| {
        gui.new_competition("Cup").await?;
        gui.follow("All competitions").await?;
        let file = std::env::temp_dir().join(format!("refused-{}.json", std::process::id()));
        std::fs::write(&file, "<html>").unwrap();
        gui.import(&file).await?;
        gui.expect_error("The file is no competition of Tournamo.")
            .await?;

        gui.rename_competition("Cup", "Cup 2026").await?;

        gui.expect_no_dialog().await?;
        gui.expect_no_error().await?;
        std::fs::remove_file(file).unwrap();
        Ok(())
    })
    .await;
}

/// Choosing another language does not say again why the last action was refused.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_refusal_is_gone_once_the_language_changes() {
    run(async |gui| {
        gui.click("Create").await?;
        gui.expect_field_error("competition-name", "A competition needs a name.")
            .await?;

        gui.choose_language("Deutsch").await?;

        gui.expect_no_error().await
    })
    .await;
}

/// After an update, say, a log may hold what the page no longer knows; the operator can still keep
/// a copy of it.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_competition_that_no_longer_loads_is_still_exported() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.follow("All competitions").await?;
        append_unknown_entry(&gui).await?;

        gui.follow(COMPETITION).await?;

        gui.page
            .expect(
                By::Css("[role=alert]"),
                |query| query.with_text(StringMatch::new("is stored damaged").partial()),
                "why the competition does not load",
            )
            .await?;
        gui.follow("All competitions").await?;
        gui.click_labelled("Export Cup").await?;
        let file = gui.page.expect_download("Cup.json").await?;
        let file = std::fs::read_to_string(file).unwrap();
        assert!(file.contains("Alice") && file.contains("unknown"));
        Ok(())
    })
    .await;
}

/// Appends an entry no page knows to the log of the only competition stored.
async fn append_unknown_entry(gui: &Gui) -> WebDriverResult<()> {
    gui.driver
        .execute_async(
            r#"
            const done = arguments[arguments.length - 1];
            const request = indexedDB.open("tournamo");
            request.onsuccess = () => {
                const database = request.result;
                const transaction = database.transaction("saves", "readwrite");
                const saves = transaction.objectStore("saves");
                saves.openCursor().onsuccess = (event) => {
                    const cursor = event.target.result;
                    const file = JSON.parse(cursor.value);
                    file.log.push({ unknown: true });
                    cursor.update(JSON.stringify(file));
                };
                transaction.oncomplete = () => {
                    database.close();
                    done();
                };
            };
            "#,
            Vec::new(),
        )
        .await?;
    Ok(())
}
