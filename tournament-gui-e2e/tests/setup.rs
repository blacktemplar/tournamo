use tournament_gui_e2e::{Key, run_in_new_competition};

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn entrants_are_listed_in_the_order_they_were_added() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.add_entrant("Bob").await?;

        gui.expect_entrants(&["Alice", "Bob"]).await
    })
    .await;
}

/// A name differing from another only in case is said to look like it while it is typed, and is
/// refused by the name entered.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_name_like_one_entered_is_pointed_out_and_refused() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;

        gui.type_entrant_name("ALICE").await?;

        gui.expect_entrant_like(Some("“Alice” is entered already."))
            .await?;
        gui.press(Key::Enter).await?;
        gui.expect_field_error("entrant-name", "There already is an entrant named “Alice”.")
            .await?;
        gui.expect_entrants(&["Alice"]).await
    })
    .await;
}

/// A new competition opens with the focus in the field of the first entrant's name.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_new_competition_opens_at_its_first_entrant() {
    run_in_new_competition(async |gui| gui.expect_focus_on_entrant_name().await).await;
}

/// Adding or removing an entrant is announced; after a removal, the focus moves on to the entrant
/// after it, or else to the field adding one.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn an_entrant_is_removed() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.expect_announcement("Added Alice.").await?;
        gui.add_entrant("Bob").await?;

        gui.click_labelled("Remove Alice").await?;

        gui.expect_entrants(&["Bob"]).await?;
        gui.expect_announcement("Removed Alice.").await?;
        gui.expect_focus_labelled("Remove Bob").await?;
        gui.click_labelled("Remove Bob").await?;
        gui.expect_focus_on_entrant_name().await
    })
    .await;
}

/// A list pasted adds an entrant for each of its lines, or nobody if a name is taken; a single
/// name pasted adds nobody by itself.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_list_pasted_adds_an_entrant_for_each_line() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;

        gui.paste_entrant_names("Bob\nCarol\n\nDave\n").await?;

        gui.expect_entrants(&["Alice", "Bob", "Carol", "Dave"])
            .await?;
        gui.expect_announcement("Added 3 entrants.").await?;
        gui.paste_entrant_names("Eve\nalice").await?;
        gui.expect_field_error("entrant-name", "There already is an entrant named “Alice”.")
            .await?;
        gui.expect_entrants(&["Alice", "Bob", "Carol", "Dave"])
            .await?;
        gui.paste_entrant_names("Eve").await?;
        gui.expect_entrants(&["Alice", "Bob", "Carol", "Dave"])
            .await
    })
    .await;
}

/// An entrant is renamed in place, keeping its place in the list, and the focus is back at its
/// button; a name entered already is refused at the field, and Escape leaves the name as it was.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn an_entrant_is_renamed_in_place() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob", "Carol"] {
            gui.add_entrant(name).await?;
        }

        gui.rename_entrant("Bob", "Robert", Key::Enter).await?;

        gui.expect_entrants(&["Alice", "Robert", "Carol"]).await?;
        gui.expect_announcement("Renamed Bob to Robert.").await?;
        gui.expect_focus_labelled("Rename Robert").await?;
        gui.rename_entrant("Alice", "carol", Key::Enter).await?;
        gui.expect_field_error(
            "entrant-new-name-0",
            "There already is an entrant named “Carol”.",
        )
        .await?;
        gui.page.check_accessibility().await?;
        gui.press(Key::Escape).await?;
        gui.expect_focus_labelled("Rename Alice").await?;
        gui.page.reload().await?;
        gui.expect_entrants(&["Alice", "Robert", "Carol"]).await
    })
    .await;
}

/// The start moves the focus to the matches, and says how many there are to play.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_start_moves_the_focus_to_the_matches() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob", "Carol"] {
            gui.add_entrant(name).await?;
        }
        gui.click("Add phase").await?;
        gui.expect_announcement("The phase is added.").await?;

        gui.click("Start").await?;

        gui.expect_focus("Matches").await?;
        gui.expect_announcement("Started: 3 matches to play.").await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn an_entrant_without_a_name_is_refused_with_why() {
    run_in_new_competition(async |gui| {
        gui.click("Add entrant").await?;

        gui.expect_field_error("entrant-name", "An entrant name must not be empty.")
            .await?;
        gui.page.check_accessibility().await?;
        gui.expect_entrants(&[]).await?;
        // gone once the name is changed
        gui.type_entrant_name("Alice").await?;
        gui.expect_no_error().await
    })
    .await;
}

/// The name of an entrant entered already is refused at the field, in place of saying that it is
/// entered, rather than in a second message.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn a_name_entered_already_is_refused_at_its_field() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.type_entrant_name("Alice").await?;
        gui.expect_entrant_like(Some("“Alice” is entered already."))
            .await?;

        gui.press(Key::Enter).await?;

        gui.expect_field_error("entrant-name", "There already is an entrant named “Alice”.")
            .await?;
        gui.expect_entrant_like(None).await?;
        gui.expect_entrants(&["Alice"]).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_start_says_why_it_is_not_possible_yet() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.expect_start_refusal(Some("The competition has no phase yet."))
            .await?;
        gui.click("Add phase").await?;

        gui.expect_start_refusal(Some("A competition needs at least two entrants."))
            .await?;
        gui.expect_disabled("Start").await?;
        gui.add_entrant("Bob").await?;
        gui.expect_start_refusal(None).await
    })
    .await;
}

/// A Monster DYP draws teams of two that play each other, so it starts with four entrants.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn the_start_says_how_many_entrants_the_format_needs() {
    run_in_new_competition(async |gui| {
        for name in ["Alice", "Bob", "Carol"] {
            gui.add_entrant(name).await?;
        }
        gui.choose_format("Monster DYP").await?;
        gui.click("Add phase").await?;

        gui.expect_start_refusal(Some("The format needs at least 4 entrants."))
            .await?;
        gui.expect_disabled("Start").await?;
        gui.add_entrant("Dave").await?;
        gui.expect_start_refusal(None).await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn undo_and_redo_walk_through_the_actions() {
    run_in_new_competition(async |gui| {
        gui.expect_disabled("Undo").await?;
        gui.add_entrant("Alice").await?;
        gui.add_entrant("Bob").await?;

        gui.click("Undo").await?;
        gui.expect_entrants(&["Alice"]).await?;
        gui.click("Redo").await?;
        gui.expect_entrants(&["Alice", "Bob"]).await?;
        gui.expect_disabled("Redo").await
    })
    .await;
}

/// Undo keeps the focus once there is nothing left to undo, and pressing it then does nothing.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn undo_keeps_the_focus_when_nothing_is_left_to_undo() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;

        gui.click("Undo").await?;

        gui.expect_entrants(&[]).await?;
        gui.expect_disabled("Undo").await?;
        gui.expect_focus("Undo").await?;
        gui.press(Key::Enter).await?;
        gui.expect_no_error().await
    })
    .await;
}

/// The phase added, the focus moves on to the start, rather than being lost with the form.
#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn adding_the_phase_moves_the_focus_to_the_start() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.add_entrant("Bob").await?;

        gui.click("Add phase").await?;

        gui.expect_focus("Start").await
    })
    .await;
}

#[tokio::test]
#[ignore = "needs Chrome and the page built by dx; run by the gui-e2e hook"]
async fn starting_needs_a_phase() {
    run_in_new_competition(async |gui| {
        gui.add_entrant("Alice").await?;
        gui.add_entrant("Bob").await?;
        gui.expect_disabled("Start").await?;

        gui.choose_format("Single elimination").await?;
        gui.click("Add phase").await?;

        gui.click("Start").await
    })
    .await;
}
