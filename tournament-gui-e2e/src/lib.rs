//! End-to-end tests of `tournament-gui`: [`run`] opens the page in headless Chrome, and the test
//! uses it through a [`Gui`], the way an operator does, by typing and clicking; a test about a
//! single competition starts in a new one with [`run_in_new_competition`]. A [`Gui`] compares
//! texts without the [`ISOLATION_MARKS`] the translation puts around names and numbers.
//!
//! The page is served from where dx builds it with the profile `wasm-e2e` into `target/e2e` (as
//! the hook `gui-e2e` does): the page as it is hosted, but without the optimizations that only
//! make it smaller. Everything else, from the chromedriver to what a failing test leaves and the
//! environment variables that show a test in a window, is that of [`e2e_harness`].

use std::path::Path;

use e2e_harness::{
    ISOLATION_MARKS, Page, Site,
    thirtyfour::{
        By, WebDriver, WebElement, components::SelectElement, error::WebDriverResult,
        extensions::query::ElementQueryable, stringmatch::StringMatch,
    },
};
pub use e2e_harness::{failures, thirtyfour, thirtyfour::Key};

/// The name of the competition [`run_in_new_competition`] creates.
pub const COMPETITION: &str = "Cup";

/// Runs `test` on a new competition named [`COMPETITION`], open on the page freshly opened, see
/// [`run`].
///
/// # Panics
/// Panics as [`run`] does.
pub async fn run_in_new_competition(test: impl AsyncFnOnce(Gui) -> WebDriverResult<()>) {
    run(async move |gui| {
        gui.new_competition(COMPETITION).await?;
        test(gui).await
    })
    .await;
}

/// Runs `test` on the page freshly opened, which lists the competitions stored, none yet, see
/// [`e2e_harness::run`].
///
/// # Panics
/// Panics if `test` fails, if the page logs an error, or if it is not accessible at the end.
pub async fn run(test: impl AsyncFnOnce(Gui) -> WebDriverResult<()>) {
    let site = Site {
        // where dx builds the page with the profile `wasm-e2e`
        dist: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target/e2e/dx/tournament-gui/debug/web/public"),
        build: "CARGO_TARGET_DIR=\"$PWD/target/e2e\" dx build -p tournament-gui --profile wasm-e2e \
                --debug-symbols false",
        // rendered by the WebAssembly
        ready: By::Css("h1"),
    };
    e2e_harness::run(site, async |page| {
        test(Gui {
            driver: page.driver.clone(),
            page,
        })
        .await
    })
    .await;
}

/// The screen of the page.
///
/// After each of its actions (typing, clicking, choosing), a test pauses as long as `E2E_PAUSE`
/// says.
#[derive(Clone)]
pub struct Gui {
    pub driver: WebDriver,
    pub page: Page,
}

impl Gui {
    /// Creates the competition `name` from the list of competitions, and waits until it is open.
    pub async fn new_competition(&self, name: &str) -> WebDriverResult<()> {
        let input = self.driver.query(By::Css("form input")).first().await?;
        input.send_keys(name).await?;
        Page::pause().await;
        self.click("Create").await?;
        self.expect_open(name).await
    }

    /// Opens the competition `name` from the list of competitions, and waits until it is open.
    pub async fn open_competition(&self, name: &str) -> WebDriverResult<()> {
        self.follow(name).await?;
        self.expect_open(name).await
    }

    /// Imports the competition of the file at `path` from the list of competitions, which opens it.
    pub async fn import(&self, path: &Path) -> WebDriverResult<()> {
        let input = self.driver.query(By::Id("import")).first().await?;
        input
            .send_keys(path.to_str().expect("the path is a text"))
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Follows the link whose text is `text`, e.g. `"All competitions"`.
    pub async fn follow(&self, text: &str) -> WebDriverResult<()> {
        self.driver
            .query(By::LinkText(text))
            .first()
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the competitions listed are `names`, in this order.
    ///
    /// # Panics
    /// Panics if they are not in time, with those listed instead.
    pub async fn expect_competitions(&self, names: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#competition-list a"), names)
            .await
    }

    /// Waits until the list of competitions says how far each is as `progress`, in the order of
    /// the list, e.g. `"Round 2 · one match to play"`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_progress(&self, progress: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#competition-list .progress"), progress)
            .await
    }

    /// Renames the competition `name` of the list of competitions `new_name`, in the dialog its
    /// button opens.
    pub async fn rename_competition(&self, name: &str, new_name: &str) -> WebDriverResult<()> {
        self.click_labelled(&format!("Rename {name}")).await?;
        self.type_new_name(new_name).await?;
        self.click("Rename").await
    }

    /// Types `name` over the name in the dialog renaming a competition, without renaming it.
    pub async fn type_new_name(&self, name: &str) -> WebDriverResult<()> {
        let input = self.driver.query(By::Id("new-name")).first().await?;
        input.clear().await?;
        input.send_keys(name).await?;
        Page::pause().await;
        Ok(())
    }

    /// Types `name` into the field of a new competition's name, without creating it.
    pub async fn type_competition_name(&self, name: &str) -> WebDriverResult<()> {
        let input = self
            .driver
            .query(By::Id("competition-name"))
            .first()
            .await?;
        input.send_keys(name).await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the field of a competition's name shown, that of a new one or of renaming one,
    /// says that another competition is named as typed, `Some` of what it says, or says nothing
    /// for `None`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_name_taken(&self, text: Option<&str>) -> WebDriverResult<()> {
        let texts: Vec<&str> = text.into_iter().collect();
        self.page
            .expect_texts(By::Css(".name-taken:not(:empty)"), &texts)
            .await
    }

    /// Asks to delete the competition `name` of the list of competitions, and confirms it in the
    /// dialog asking.
    pub async fn delete_competition(&self, name: &str) -> WebDriverResult<()> {
        self.click_labelled(&format!("Delete {name}")).await?;
        self.click("Delete").await
    }

    /// Types `name` into the field of a new entrant, without adding it.
    pub async fn type_entrant_name(&self, name: &str) -> WebDriverResult<()> {
        let input = self.driver.query(By::Id("entrant-name")).first().await?;
        input.send_keys(name).await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the field of a new entrant says that what is typed looks like the name of an
    /// entrant, `Some` of what it says, or says nothing for `None`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_entrant_like(&self, text: Option<&str>) -> WebDriverResult<()> {
        let texts: Vec<&str> = text.into_iter().collect();
        self.page
            .expect_texts(By::Css("#entrant-like:not(:empty)"), &texts)
            .await
    }

    /// Waits until the competition `name` is open.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_open(&self, name: &str) -> WebDriverResult<()> {
        self.page.expect_texts(By::Css("h1"), &[name]).await?;
        // the screen of the competition, before or after the start, rather than that of opening it
        self.driver
            .query(By::Css("#entrants, #matches"))
            .first()
            .await?;
        Ok(())
    }

    /// Waits until the competition open is shown only, since another tab changes it, if
    /// `read_only`, or else until it can be changed here.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_read_only(&self, read_only: bool) -> WebDriverResult<()> {
        let notices: &[&str] = if read_only {
            &[
                "This competition is open in another tab, so it is only shown here. It follows \
               what is done there, and can be changed here once that tab leaves it.",
            ]
        } else {
            &[]
        };
        self.page.expect_texts(By::Css(".notice"), notices).await
    }

    /// Pastes `text` into the field of a new entrant's name, as from the clipboard.
    pub async fn paste_entrant_names(&self, text: &str) -> WebDriverResult<()> {
        let input = self.driver.query(By::Id("entrant-name")).first().await?;
        // headless Chrome has no clipboard to paste from
        self.driver
            .execute(
                "const data = new DataTransfer();
                data.setData('text/plain', arguments[1]);
                arguments[0].focus();
                arguments[0].dispatchEvent(new ClipboardEvent('paste', {
                    clipboardData: data, bubbles: true, cancelable: true,
                }));",
                vec![input.to_json()?, serde_json::Value::from(text)],
            )
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Renames the entrant `name` in place: opens its field by its button, and types `new_name`
    /// over the name there, then presses `key`, Enter to rename it or Escape to leave it.
    pub async fn rename_entrant(
        &self,
        name: &str,
        new_name: &str,
        key: Key,
    ) -> WebDriverResult<()> {
        self.click_labelled(&format!("Rename {name}")).await?;
        let input = self
            .driver
            .query(By::Css("#entrant-list input"))
            .first()
            .await?;
        input.send_keys(Key::Control + "a").await?;
        input.send_keys(new_name).await?;
        Page::pause().await;
        input.send_keys(key).await?;
        Page::pause().await;
        Ok(())
    }

    /// Adds the entrant `name`, and waits until it is listed.
    pub async fn add_entrant(&self, name: &str) -> WebDriverResult<()> {
        let input = self.driver.query(By::Id("entrant-name")).first().await?;
        input.send_keys(name).await?;
        Page::pause().await;
        input.send_keys(Key::Enter).await?;
        self.page
            .expect_text(By::Css("#entrant-list .name"), name.to_owned())
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Chooses the format labelled `label` in the form of the phase shown, which may be that of a
    /// dialog.
    pub async fn choose_format(&self, label: &str) -> WebDriverResult<()> {
        self.choose_plugin("format", label).await
    }

    /// Chooses the plugin labelled `label` for the field at `path` of the form of the phase, e.g.
    /// `"format"`, or adds it to the list at `path` if it is one whose order matters, e.g.
    /// `"ranking"`.
    pub async fn choose_plugin(&self, path: &str, label: &str) -> WebDriverResult<()> {
        self.open_choice(path).await?;
        self.choose_option(label).await
    }

    /// Chooses the option labelled `label` of the list of plugins open, by a click on its label
    /// (one on its ⓘ shows its description instead).
    pub async fn choose_option(&self, label: &str) -> WebDriverResult<()> {
        self.option(label)
            .first()
            .await?
            .find(By::Css(".option-label"))
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Opens the list of the plugins the field at `path` of the form of the phase can take, or
    /// those that can be added to the list at `path`, as [`Self::choose_plugin`] does.
    pub async fn open_choice(&self, path: &str) -> WebDriverResult<()> {
        let id = field_id(path);
        let trigger = match self.driver.find(By::Id(format!("{id}-add"))).await {
            Ok(add) => add,
            Err(_) => self.driver.query(By::Id(id)).first().await?,
        };
        // in sight, as below a long list of entrants it may be under the bar
        self.click_in_sight(&trigger).await
    }

    /// Moves the pointer onto the option labelled `label` of the list of plugins open.
    pub async fn hover_option(&self, label: &str) -> WebDriverResult<()> {
        let option = self.option(label).first().await?;
        self.driver
            .action_chain()
            .move_to_element_center(&option)
            .perform()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Taps the ⓘ of the option labelled `label` of the list of plugins open, as on a touch screen,
    /// which shows its description in it, or hides it again, rather than choosing it.
    pub async fn tap_option_info(&self, label: &str) -> WebDriverResult<()> {
        self.option(label)
            .first()
            .await?
            .find(By::Css(".option-info"))
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until an option of the list of plugins open shows its description `text` in it,
    /// tapped open by [`Self::tap_option_info`].
    ///
    /// # Panics
    /// Panics if it does not in time.
    pub async fn expect_option_description_tapped_open(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css(".option-description[data-tapped]"),
                |query| query.and_displayed().with_text(text.to_owned()),
                &format!("an option's description tapped open saying {text:?}"),
            )
            .await
    }

    /// Waits until the description shown of an option of the list of plugins open is `text`.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_option_description(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css(".option-description"),
                |query| query.and_displayed().with_text(text.to_owned()),
                &format!("an option's description shown saying {text:?}"),
            )
            .await
    }

    /// Enters `text` into the number field at `path` of the form of the phase, e.g.
    /// `"format.rounds"`, and leaves the field, which takes it.
    pub async fn enter(&self, path: &str, text: &str) -> WebDriverResult<()> {
        let input = self.driver.query(By::Id(field_id(path))).first().await?;
        input.clear().await?;
        input.send_keys(text).await?;
        Page::pause().await;
        input.send_keys(Key::Tab).await?;
        Page::pause().await;
        Ok(())
    }

    /// Types `text` into the field at `path` of the form of the phase in place of what it shows,
    /// and stays there, e.g. to press Enter next.
    pub async fn type_into(&self, path: &str, text: &str) -> WebDriverResult<()> {
        let input = self.driver.query(By::Id(field_id(path))).first().await?;
        input.clear().await?;
        input.send_keys(text).await?;
        Page::pause().await;
        Ok(())
    }

    /// Checks or unchecks the checkbox labelled `label`, e.g. a plugin of the metrics.
    pub async fn toggle(&self, label: &str) -> WebDriverResult<()> {
        self.driver
            .query(By::XPath(format!(
                "//label[{}='{label}']/input[@type='checkbox']",
                without_isolation_marks("normalize-space()")
            )))
            .and_enabled()
            .first()
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the checkbox labelled `label` is checked, if `ticked`, or else unchecked, as the
    /// browser shows it.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_ticked(&self, label: &str, ticked: bool) -> WebDriverResult<()> {
        let state = if ticked { ":checked" } else { ":not(:checked)" };
        self.page
            .expect(
                By::Css(format!("label:has(> input[type=checkbox]{state})")),
                |query| query.with_text(label.to_owned()),
                &format!("the checkbox {label:?} ticked: {ticked}"),
            )
            .await
    }

    /// Waits until the list of the choice of the field at `path` of the form of the phase is
    /// open, if `open`, or else closed, as its button says.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_choice_open(&self, path: &str, open: bool) -> WebDriverResult<()> {
        let id = field_id(path);
        self.page
            .expect(
                By::Css(format!("#{id}[aria-expanded={open}]")),
                |query| query,
                &format!("the list of the choice {path} open: {open}"),
            )
            .await
    }

    /// Checks that the option of the list of plugins open with the focus, or else the one under the
    /// pointer, stands out from the list 3:1 (WCAG 1.4.11), by its outline, its shadow or its
    /// background.
    ///
    /// # Panics
    /// Panics if no option is highlighted, or it does not stand out enough, with the contrast.
    pub async fn expect_highlighted_option_stands_out(&self) -> WebDriverResult<()> {
        let contrast = self
            .driver
            .execute(
                r#"
                const option = document.querySelector("[role=option]:focus")
                    ?? document.querySelector("[role=option]:hover");
                if (!option) return null;
                const list = option.closest("[role=listbox]");
                // the channels of an opaque colour, nothing for a transparent one
                const rgb = (colour) => {
                    const [r, g, b, a = 1] = (colour?.match(/[\d.]+/g) ?? []).map(Number);
                    return a > 0 && b !== undefined ? [r, g, b] : null;
                };
                const luminance = (colour) => colour
                    .map((channel) => channel / 255)
                    .map((c) => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)
                    .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0);
                const ratio = (a, b) => {
                    const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
                    return (light + 0.05) / (dark + 0.05);
                };
                const style = getComputedStyle(option);
                const background = rgb(getComputedStyle(list).backgroundColor);
                const outline = style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0
                    ? style.outlineColor : null;
                const shadow = style.boxShadow.match(/rgba?\([^)]*\)/)?.[0];
                return Math.max(...[outline, shadow, style.backgroundColor]
                    .map(rgb).filter(Boolean).map((colour) => ratio(colour, background)), 1);
                "#,
                Vec::new(),
            )
            .await?
            .convert::<Option<f64>>()?;
        let contrast = contrast.expect("an option must be highlighted");
        assert!(
            contrast >= 3.0,
            "the option highlighted must stand out 3:1 from the list, but does {contrast:.2}:1"
        );
        Ok(())
    }

    /// Waits until what is wrong with the answers of the form of the phase is `issues`, those of
    /// every field in the order of the fields.
    ///
    /// # Panics
    /// Panics if it is not in time, with what is shown instead.
    pub async fn expect_issues(&self, issues: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".phase-form .issues li"), issues)
            .await
    }

    /// Waits until what the answers of the form of the phase lead to that the operator may not
    /// want is `warnings`, in the order of the fields.
    ///
    /// # Panics
    /// Panics if it is not in time, with what is shown instead.
    pub async fn expect_warnings(&self, warnings: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".phase-form .warnings li"), warnings)
            .await
    }

    /// Waits until the plugins of the list at `path` of the form of the phase whose order matters
    /// are `labels`, in this order, e.g. `"ranking"`.
    ///
    /// # Panics
    /// Panics if they are not in time, with those shown instead.
    pub async fn expect_list(&self, path: &str, labels: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(format!("#{} > li > .name", field_id(path))), labels)
            .await
    }

    /// Waits until the plugins checked in the set of plugins at `path` of the form of the phase,
    /// e.g. `"metrics"`, are `labels`, in the order shown, those it implies included. What is
    /// checked is what the browser shows, not what the page last rendered.
    ///
    /// # Panics
    /// Panics if they are not in time, with those checked instead.
    pub async fn expect_checked(&self, path: &str, labels: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css(format!(
                    "#{} > li > label:has(> input:checked)",
                    field_id(path)
                )),
                labels,
            )
            .await
    }

    /// Waits until the field at `path` of the form of the phase, which takes a number, shows
    /// `text`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_entered(&self, path: &str, text: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Id(field_id(path)),
                |query| query.with_value(text.to_owned()),
                &format!("the value {text:?}"),
            )
            .await
    }

    /// Waits until the focus is on the checkbox labelled `label`, e.g. a plugin of the metrics.
    ///
    /// # Panics
    /// Panics if it is not in time, with the label of the checkbox it is on instead, if any.
    pub async fn expect_focus_on_checkbox(&self, label: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("label:has(> input[type=checkbox]:focus)"), &[label])
            .await
    }

    /// Waits until the focus is on the field at `path` of the form of the phase, marked as invalid
    /// (`aria-invalid`).
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_focus_on_invalid(&self, path: &str) -> WebDriverResult<()> {
        let id = field_id(path);
        self.page
            .expect(
                By::Css(format!("#{id}:focus[aria-invalid=true]")),
                |query| query,
                &format!("the focus on the field {path} marked as invalid"),
            )
            .await
    }

    /// Waits until the focus is in the item labelled `label` of the list at `path` of the form of
    /// the phase whose order matters, e.g. on a button moving it.
    ///
    /// # Panics
    /// Panics if it is not in time, with the item it is in instead, if any.
    pub async fn expect_focus_in_list(&self, path: &str, label: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css(format!("#{} > li:has(:focus) > .name", field_id(path))),
                &[label],
            )
            .await
    }

    /// Waits until the focus is on the button choosing the plugin of the field at `path` of the
    /// form of the phase, or adding one to the list at `path`.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_focus_on_choice(&self, path: &str) -> WebDriverResult<()> {
        let id = field_id(path);
        self.driver
            .query(By::Css(format!("#{id}:focus, #{id}-add:focus")))
            .first()
            .await?;
        Ok(())
    }

    /// Waits until how the phase is played reads `answers`, the answers of its fields in order,
    /// e.g. `"round robin"`, with the plugins of a list joined by commas, or each an answer of its
    /// own if any of them has fields.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it reads instead.
    pub async fn expect_phase(&self, answers: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css(
                    "[aria-labelledby=phase] > dl > dd > .answer, \
                     [aria-labelledby=phase] > dl > dd > ul > li > .answer",
                ),
                answers,
            )
            .await
    }

    /// Waits until the dialog open shows why what was done in it was refused, `text`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_dialog_error(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css("[role=dialog] [role=alert], [role=alertdialog] [role=alert]"),
                &[text],
            )
            .await
    }

    /// Waits until no dialog is open, nor one asking to confirm (`alertdialog`).
    ///
    /// # Panics
    /// Panics if one still is in time.
    pub async fn expect_no_dialog(&self) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=dialog], [role=alertdialog]"), &[])
            .await
    }

    /// Waits until a dialog asking to confirm what may not be undone (`alertdialog`) is open,
    /// titled `title`.
    ///
    /// # Panics
    /// Panics if none is in time, with the titles of those open instead.
    pub async fn expect_alert_dialog(&self, title: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=alertdialog] h2"), &[title])
            .await
    }

    /// Chooses the language named `name` in itself, e.g. `"Deutsch"`.
    pub async fn choose_language(&self, name: &str) -> WebDriverResult<()> {
        self.choose("language", name).await
    }

    /// Chooses the colours named `name`, e.g. `"Dark"`.
    pub async fn choose_theme(&self, name: &str) -> WebDriverResult<()> {
        self.choose("theme", name).await
    }

    /// Waits until the colours chosen, `<html data-theme>`, are `tag` (`"light"` or `"dark"`), or
    /// the system's for `None`.
    ///
    /// # Panics
    /// Panics if they are not in time.
    pub async fn expect_theme(&self, tag: Option<&str>) -> WebDriverResult<()> {
        let selector = match tag {
            Some(tag) => format!("html[data-theme='{tag}']"),
            None => "html:not([data-theme])".to_owned(),
        };
        self.page
            .expect(
                By::Css(selector),
                |query| query,
                &format!("the theme {tag:?}"),
            )
            .await
    }

    /// Waits until the language of the page, its `<html lang>`, is `tag`.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_page_language(&self, tag: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css("html"),
                |query| query.with_attribute("lang", tag.to_owned()),
                &format!("the language {tag:?}"),
            )
            .await
    }

    /// Clicks the button whose text is `text`, once it is enabled.
    pub async fn click(&self, text: &str) -> WebDriverResult<()> {
        let button = self.button(text).and_enabled().first().await?;
        self.click_in_sight(&button).await
    }

    /// Clicks the button whose accessible name, its `aria-label`, is `label`, once it is enabled,
    /// e.g. `"Alice wins"`.
    pub async fn click_labelled(&self, label: &str) -> WebDriverResult<()> {
        // e.g. a result button of the matches and one of the bracket, only one of them shown
        let button = self
            .labelled(label)
            .and_displayed()
            .and_enabled()
            .first()
            .await?;
        self.click_in_sight(&button).await
    }

    /// Clicks `element` once it is scrolled into the middle of the window, as an operator looks
    /// at what they click: WebDriver would click it where it is if it is in the window at all, even
    /// under the bar staying on top.
    async fn click_in_sight(&self, element: &WebElement) -> WebDriverResult<()> {
        self.driver
            .execute(
                "arguments[0].scrollIntoView({ block: 'center', inline: 'nearest' });",
                vec![element.to_json()?],
            )
            .await?;
        element.click().await?;
        Page::pause().await;
        Ok(())
    }

    /// Double-clicks the button whose text is `text`, once it is enabled, both clicks before the
    /// page shows what the first did.
    pub async fn double_click(&self, text: &str) -> WebDriverResult<()> {
        let button = self
            .button(text)
            .and_displayed()
            .and_enabled()
            .first()
            .await?;
        self.driver
            .action_chain()
            .move_to_element_center(&button)
            .double_click()
            .perform()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Taps twice, 250 ms apart as in a double tap, at the middle of the button whose text is
    /// `text`, once it is enabled: the second tap lands on whatever is there after the first.
    pub async fn double_tap(&self, text: &str) -> WebDriverResult<()> {
        let button = self
            .button(text)
            .and_displayed()
            .and_enabled()
            .first()
            .await?;
        self.driver
            .action_chain()
            .move_to_element_center(&button)
            .click()
            .perform()
            .await?;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        // where the pointer is still
        self.driver.action_chain().click().perform().await?;
        Page::pause().await;
        Ok(())
    }

    /// Moves the pointer onto the button whose accessible name, its `aria-label`, is `label`, e.g.
    /// `"About points"`.
    pub async fn hover_labelled(&self, label: &str) -> WebDriverResult<()> {
        let button = self.labelled(label).first().await?;
        self.driver
            .action_chain()
            .move_to_element_center(&button)
            .perform()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the toggletip shown says `text`, ignoring [`ISOLATION_MARKS`], or until none is
    /// shown for `None`.
    ///
    /// # Panics
    /// Panics if it does not in time, or if what it says is cut off or out of the window.
    pub async fn expect_toggletip(&self, text: Option<&str>) -> WebDriverResult<()> {
        let shown = By::Css(".toggletip-text");
        match text {
            Some(text) => {
                let saying = || {
                    let text = text.to_owned();
                    move |found: &str| found.replace(ISOLATION_MARKS, "") == text
                };
                self.page
                    .expect(
                        shown.clone(),
                        |query| query.and_displayed().with_text(saying()),
                        &format!("a toggletip shown saying {text:?}"),
                    )
                    .await?;
                let element = self
                    .driver
                    .query(shown)
                    .and_displayed()
                    .with_text(saying())
                    .first()
                    .await?;
                // the middle of each edge (its corners are round) shows the text, not something
                // in front of it or nothing, as where an element scrolling cuts it off or out of
                // the window
                let whole = self
                    .driver
                    .execute(
                        "const text = arguments[0];
                        const box = text.getBoundingClientRect();
                        const [x, y] = [(box.left + box.right) / 2, (box.top + box.bottom) / 2];
                        return [[box.left + 1, y], [box.right - 1, y],
                                [x, box.top + 1], [x, box.bottom - 1]]
                            .every(([x, y]) => text.contains(document.elementFromPoint(x, y)));",
                        vec![element.to_json()?],
                    )
                    .await?
                    .convert::<bool>()?;
                assert!(whole, "the toggletip saying {text:?} must show whole");
                Ok(())
            }
            None => {
                let gone = self.driver.query(shown).and_displayed();
                assert!(
                    gone.wait_until_gone().await.is_ok(),
                    "a toggletip must not be shown"
                );
                Ok(())
            }
        }
    }

    /// Waits until a toggletip shows, and checks that it covers none of the issues of the form of
    /// the phase.
    ///
    /// # Panics
    /// Panics if none shows in time, or it covers an issue.
    pub async fn expect_toggletip_clear_of_issues(&self) -> WebDriverResult<()> {
        let shown = self
            .driver
            .query(By::Css(".toggletip-text"))
            .and_displayed()
            .first()
            .await?;
        let covered = self
            .driver
            .execute(
                "const text = arguments[0].getBoundingClientRect();
                return [...document.querySelectorAll('.phase-form .issues li')]
                    .map(issue => [issue, issue.getBoundingClientRect()])
                    .filter(([, box]) => box.left < text.right && text.left < box.right
                        && box.top < text.bottom && text.top < box.bottom)
                    .map(([issue]) => issue.textContent);",
                vec![shown.to_json()?],
            )
            .await?
            .convert::<Vec<String>>()?;
        assert!(
            covered.is_empty(),
            "the toggletip shown must cover no issue, but covers {covered:?}"
        );
        Ok(())
    }

    /// Waits until the button labelled `label` shows and takes a click.
    ///
    /// # Panics
    /// Panics if it does not in time.
    pub async fn expect_enabled(&self, label: &str) -> WebDriverResult<()> {
        self.labelled(label)
            .and_displayed()
            .and_enabled()
            .first()
            .await?;
        Ok(())
    }

    /// Waits until the button labelled `label` shows, and expects it to take a click at once.
    ///
    /// # Panics
    /// Panics if it does not show in time, or takes no click when it shows.
    pub async fn expect_enabled_at_once(&self, label: &str) -> WebDriverResult<()> {
        let button = self.labelled(label).and_displayed().first().await?;
        assert!(
            button.is_enabled().await?,
            "{label} takes no click as it shows"
        );
        Ok(())
    }

    /// Waits until the button whose text is `text` is disabled, or says it is unavailable while
    /// staying focusable (`aria-disabled`).
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_disabled(&self, text: &str) -> WebDriverResult<()> {
        self.driver
            .query(By::XPath(format!(
                "//button[{}='{text}'][@disabled or @aria-disabled='true']",
                without_isolation_marks("normalize-space()")
            )))
            .first()
            .await?;
        Ok(())
    }

    /// Waits until the screen shows why the last action was refused, `text`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_error(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=alert]:not(:empty)"), &[text])
            .await
    }

    /// Waits until the field whose id is `field` shows why the last action was refused, `text`, as
    /// the only refusal on the screen, and is marked invalid and described by it.
    ///
    /// # Panics
    /// Panics if it does not in time, with what the screen shows instead.
    pub async fn expect_field_error(&self, field: &str, text: &str) -> WebDriverResult<()> {
        self.expect_error(text).await?;
        self.page
            .expect_texts(
                By::Css(format!(
                    "#{field}[aria-invalid=true] ~ [role=alert]#{field}-error"
                )),
                &[text],
            )
            .await?;
        self.driver
            .query(By::Css(format!(
                "#{field}[aria-describedby~={field}-error]"
            )))
            .first()
            .await?;
        Ok(())
    }

    /// Unfolds the detail of the refusal shown, and waits until it contains `part`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_error_detail(&self, part: &str) -> WebDriverResult<()> {
        self.driver
            .query(By::Css(".error-detail summary"))
            .first()
            .await?
            .click()
            .await?;
        self.page
            .expect(
                By::Css(".error-detail p"),
                |query| query.with_text(StringMatch::new(part).partial()),
                &format!("the detail of the refusal containing {part:?}"),
            )
            .await
    }

    /// Waits until the screen shows no refusal.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_no_error(&self) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=alert]:not(:empty)"), &[])
            .await
    }

    /// Waits until the entry of results shows why what was typed in was not reported, `text`,
    /// below its field.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_result_entry_error(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".result-entry [role=alert]"), &[text])
            .await
    }

    /// Waits until the start says why the competition cannot start, `text`, or says nothing for
    /// `None`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_start_refusal(&self, text: Option<&str>) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Id("start-refusal"), text.as_slice())
            .await
    }

    /// Waits until the entrants listed are `names`, in this order.
    ///
    /// # Panics
    /// Panics if they are not in time, with those listed instead.
    pub async fn expect_entrants(&self, names: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#entrant-list .name"), names)
            .await
    }

    /// Waits until the matches of the round shown are those numbered `numbers`, in this order.
    ///
    /// # Panics
    /// Panics if they are not in time, with the numbers shown instead.
    pub async fn expect_matches(&self, numbers: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#match-list .number"), numbers)
            .await
    }

    /// Shows the matches of the round whose option reads `text`, e.g. `"Round 2 (2 open)"`, while
    /// the matches are shown by round.
    pub async fn choose_round(&self, text: &str) -> WebDriverResult<()> {
        self.driver
            .query(By::XPath(format!(
                "//select[@id='round']/option[{}='{text}']",
                without_isolation_marks("normalize-space()")
            )))
            .first()
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the rounds to choose from, while the matches are shown by round, read `texts`,
    /// e.g. `"Round 2 (2 open)"`.
    ///
    /// # Panics
    /// Panics if they do not in time, with what they read instead.
    pub async fn expect_round_options(&self, texts: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("select#round option"), texts)
            .await
    }

    /// Waits until the cells of the column of the standings whose header starts with `header` are
    /// `cells`, best first.
    ///
    /// # Panics
    /// Panics if there is no such column, or its cells are not `cells` in time, with what they are
    /// instead.
    pub async fn expect_standings_cells(
        &self,
        header: &str,
        cells: &[&str],
    ) -> WebDriverResult<()> {
        let headers = self
            .driver
            .query(By::Css("#standing-list thead th"))
            .all_from_selector_required()
            .await?;
        let mut column = None;
        for (index, th) in headers.iter().enumerate() {
            if th.text().await?.starts_with(header) {
                column = Some(index + 1);
                break;
            }
        }
        let column = column.unwrap_or_else(|| panic!("the standings have no column {header:?}"));
        self.page
            .expect_texts(
                By::Css(format!("#standing-list tbody tr > :nth-child({column})")),
                cells,
            )
            .await
    }

    /// Waits until the results of the matches of the round shown that have one are `results`, in
    /// the order of the matches, e.g. `"Alice won"`.
    ///
    /// # Panics
    /// Panics if they are not in time, with the results shown instead.
    pub async fn expect_results(&self, results: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#match-list .result"), results)
            .await
    }

    /// Waits until the match of the round shown that has a result has the scores `scores`, read
    /// from either side, e.g. `"21–17 19–21"` or `"17–21 21–19"`, or `"2–1 (21–17 19–21 21–15)"`
    /// with the games won: a test must not depend on the sides.
    ///
    /// # Panics
    /// Panics if it does not in time, with the results shown instead.
    pub async fn expect_scores_either_way(&self, scores: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts_of(
                By::Css("#match-list .result"),
                &[&[scores], &[&mirrored(scores)]],
            )
            .await
    }

    /// Waits until the bracket shows the scores `scores` of its one finished match, read from
    /// either side, as [`Self::expect_scores_either_way`] does in the list of matches.
    ///
    /// # Panics
    /// Panics if it does not in time, with the scores shown instead.
    pub async fn expect_bracket_scores_either_way(&self, scores: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts_of(
                By::Css(".bracket .scores"),
                &[&[scores], &[&mirrored(scores)]],
            )
            .await
    }

    /// Waits until the matches of the list shown that have a result are those numbered `numbers`,
    /// in this order, whoever won them.
    ///
    /// # Panics
    /// Panics if they are not in time, with the numbers shown instead.
    pub async fn expect_finished(&self, numbers: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#match-list > li:has(.result) .number"), numbers)
            .await
    }

    /// Checks that the first match shown with a winner looks played rather than like a
    /// placeholder, with a solid border, and that its loser's name recedes behind the winner's:
    /// another colour, and no bolder.
    ///
    /// # Panics
    /// Panics if there is no such match, or it does not look so, with how it looks.
    pub async fn expect_finished_card_looks_played(&self) -> WebDriverResult<()> {
        let looks = self
            .driver
            .execute(
                r##"
                const card = document.querySelector("#match-list > li:has(.winner)");
                if (!card) return null;
                const style = (selector) => getComputedStyle(card.querySelector(selector));
                const [winner, loser] = [style(".side.winner"), style(".side:not(.winner)")];
                return {
                    border: getComputedStyle(card).borderTopStyle,
                    winner: [winner.color, winner.fontWeight],
                    loser: [loser.color, loser.fontWeight],
                };
                "##,
                Vec::new(),
            )
            .await?
            .json()
            .clone();
        let played = looks["border"] == "solid"
            && looks["winner"][0] != looks["loser"][0]
            && looks["loser"][1]
                .as_str()
                .and_then(|weight| weight.parse::<u32>().ok())
                <= looks["winner"][1]
                    .as_str()
                    .and_then(|weight| weight.parse().ok());
        assert!(
            played,
            "a match played must have a solid border and a loser receding, but has {looks:#}"
        );
        Ok(())
    }

    /// Waits until the stages of the matches of the round shown are `stages`, in the order of the
    /// matches, e.g. `"Semifinals"`.
    ///
    /// # Panics
    /// Panics if they are not in time, with the stages shown instead.
    pub async fn expect_stages(&self, stages: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#match-list .stage"), stages)
            .await
    }

    /// Waits until the stages the bracket shows are `stages`, in order, e.g. `"Final"`.
    ///
    /// # Panics
    /// Panics if they are not in time, with the stages shown instead.
    pub async fn expect_bracket_stages(&self, stages: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".bracket-stage"), stages)
            .await
    }

    /// Checks that the lines of the bracket shown meet its pairings: each pairing is as wide as its
    /// slot, whose edges the lines start and end at; the slots of a column are equally high; and a
    /// line goes straight on to the middle of the pairing it leads to, or meets the line of its
    /// neighbour at that middle.
    ///
    /// # Panics
    /// Panics with what is off, by section, column and slot, each counted from 0.
    pub async fn expect_bracket_lines_meet(&self) -> WebDriverResult<()> {
        let problems = self
            .driver
            .execute(
                r#"
                const problems = [];
                const middle = (slot) => {
                    const box = slot.querySelector(".pairing").getBoundingClientRect();
                    return (box.top + box.bottom) / 2;
                };
                const off = (a, b) => Math.abs(a - b) > 1;
                document.querySelectorAll(".bracket").forEach((bracket, section) => {
                    const columns = [...bracket.querySelectorAll(":scope > .bracket-column")]
                        .map((column) =>
                            [...column.querySelectorAll(":scope > .bracket-slots > ol > li")]);
                    columns.forEach((rows, c) => rows.forEach((row, r) => {
                        const at = `section ${section}, column ${c}, row ${r}`;
                        if (off(row.getBoundingClientRect().height,
                            rows[0].getBoundingClientRect().height)) {
                            problems.push(`${at}: the row is not as high as the column's first`);
                        }
                        const next = columns[c + 1];
                        // a column folded into the rows of the next one, or one of as many
                        // pairings, leads to the pairing of the same row, else to that of the
                        // two rows meeting
                        const to = () =>
                            next[rows.length === next.length ? r : Math.floor(r / 2)];
                        // two pairings in the halves of a row of a folded column
                        const pair = row.classList.contains("slots");
                        const slots = pair ? [...row.querySelectorAll(":scope > ol > li")] : [row];
                        slots.forEach((slot, s) => {
                            if (!slot.querySelector(".pairing")) {
                                return;
                            }
                            const box = slot.getBoundingClientRect();
                            const pairing = slot.querySelector(".pairing").getBoundingClientRect();
                            if (off(pairing.left, box.left) || off(pairing.right, box.right)) {
                                problems.push(`${at}: the pairing is narrower than its slot`);
                            }
                            if (slot.classList.contains("line-straight")
                                && off(middle(slot), middle(to()))) {
                                problems.push(`${at}: the line misses the pairing it leads to`);
                            }
                            const below = pair ? slots[s + 1] : rows[r + 1];
                            if (slot.classList.contains("line-down")
                                && off((middle(slot) + middle(below)) / 2, middle(to()))) {
                                problems.push(`${at}: the lines miss the pairing they lead to`);
                            }
                        });
                    }));
                });
                return problems;
                "#,
                Vec::new(),
            )
            .await?
            .json()
            .clone();
        assert!(
            problems.as_array().is_some_and(Vec::is_empty),
            "the lines of the bracket must meet its pairings, but {problems:#}"
        );
        Ok(())
    }

    /// Waits until the view chosen after the start is `label`, e.g. "Bracket", as the buttons
    /// choosing it say: the one pressed of those shown.
    ///
    /// # Panics
    /// Panics if it is not in time, with the view chosen instead.
    pub async fn expect_view(&self, label: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css(".view-switch button[aria-pressed=true]"),
                |query| query.and_displayed().with_text(label.to_owned()),
                &format!("the view chosen {label:?}"),
            )
            .await
    }

    /// Checks that nothing of the page shows between the bar on top and the bottom of the view
    /// switch below it, which stays on top as well: every point there is the bar's, the switch's or
    /// the bare background's beside it, and the switch is opaque.
    ///
    /// # Panics
    /// Panics if something else shows there, with where and what.
    pub async fn expect_nothing_shows_around_the_view_switch(&self) -> WebDriverResult<()> {
        let through = self
            .driver
            .execute(
                r#"
                const bar = document.querySelector(".bar").getBoundingClientRect();
                const view = document.querySelector(".view-switch");
                const box = view.getBoundingClientRect();
                const found = [];
                const background = getComputedStyle(view).backgroundColor;
                if (!/^rgb\(/.test(background)) {
                    found.push(`the switch is not opaque: ${background}`);
                }
                const width = document.documentElement.clientWidth;
                for (let y = Math.ceil(bar.bottom); y < box.bottom; y += 2) {
                    for (const x of [2, width / 2, width - 2]) {
                        const element = document.elementFromPoint(x, y);
                        const bare = ["HTML", "BODY", "MAIN"].includes(element?.tagName)
                            || element?.id === "main";
                        if (!bare && !element?.closest(".bar, .view-switch")) {
                            found.push(`at ${x}, ${y}: ${element?.outerHTML.slice(0, 80)}`);
                        }
                    }
                }
                return found.slice(0, 5);
                "#,
                Vec::new(),
            )
            .await?
            .convert::<Vec<String>>()?;
        assert!(
            through.is_empty(),
            "nothing must show between the bar and the view switch, but {through:#?}"
        );
        Ok(())
    }

    /// Checks that the bracket fits into its card, so that it does not scroll sideways.
    ///
    /// # Panics
    /// Panics if it does not, with how much wider it is.
    /// Scrolls the bracket sideways to its last column.
    pub async fn scroll_bracket_to_the_end(&self) -> WebDriverResult<()> {
        self.driver
            .execute(
                "const box = document.querySelector('[aria-labelledby=bracket] .scroll');
                box.scrollLeft = box.scrollWidth;",
                Vec::new(),
            )
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the bracket's left edge fades out if `left`, as where more of it is out of
    /// sight there, and its right edge if `right`, and the others do not.
    ///
    /// # Panics
    /// Panics if they do not in time.
    pub async fn expect_bracket_fades(&self, left: bool, right: bool) -> WebDriverResult<()> {
        let edge = |fades, side| {
            let attribute = format!("[data-more-{side}]");
            if fades {
                attribute
            } else {
                format!(":not({attribute})")
            }
        };
        let by = format!(
            ".bracket-frame{}{}",
            edge(left, "left"),
            edge(right, "right")
        );
        self.page
            .expect(
                By::Css(by),
                |query| query,
                &format!("the left edge fading out: {left}, the right one: {right}"),
            )
            .await
    }

    /// Checks that the bracket, higher than the screen below the bar and the view switch, scrolls
    /// within a box that fits there, and that the stages of its first section stay on top of it
    /// when it is scrolled down, which this does.
    ///
    /// # Panics
    /// Panics if the bracket is not higher than its box, the box, scrolled into view, reaches
    /// below the screen, or a stage scrolls away.
    pub async fn expect_bracket_stages_stay_on_top(&self) -> WebDriverResult<()> {
        let problems = self
            .driver
            .execute(
                r#"
                const box = document.querySelector("[aria-labelledby=bracket] .scroll");
                const problems = [];
                if (box.scrollHeight <= box.clientHeight) {
                    problems.push("the bracket is not higher than its box");
                }
                // at the top of the screen below the bar and the view switch, which the page's
                // scroll padding leaves free
                box.scrollIntoView({ block: "start" });
                const bottom = box.getBoundingClientRect().bottom;
                if (bottom > window.innerHeight + 1) {
                    problems.push(`its box ends at ${bottom}px, below the screen`);
                }
                box.scrollTop = box.scrollHeight / 2;
                const top = box.getBoundingClientRect().top;
                box.querySelectorAll(".bracket-section:first-child .bracket-stage")
                    .forEach((stage) => {
                        if (stage.closest(".below")) {
                            return;
                        }
                        const at = stage.getBoundingClientRect().top;
                        if (Math.abs(at - top) > 1) {
                            problems.push(`${stage.textContent} is at ${at}, not at ${top}`);
                        }
                    });
                return problems;
                "#,
                Vec::new(),
            )
            .await?
            .json()
            .clone();
        assert!(
            problems.as_array().is_some_and(Vec::is_empty),
            "the stages of the bracket must stay on top, but {problems:#}"
        );
        Ok(())
    }

    pub async fn expect_bracket_fits(&self) -> WebDriverResult<()> {
        let [content, shown] = self
            .driver
            .execute(
                r#"
                const scroll = document.querySelector("[aria-labelledby=bracket] .scroll");
                return [scroll.scrollWidth, scroll.clientWidth];
                "#,
                Vec::new(),
            )
            .await?
            .convert::<[u32; 2]>()?;
        assert!(
            content <= shown,
            "the bracket must fit into its card, but is {content}px wide in {shown}px"
        );
        Ok(())
    }

    /// Waits until the sides of the bracket not decided yet are `sides`, in the order of the
    /// bracket, e.g. `"Winner of 1"`.
    ///
    /// # Panics
    /// Panics if they are not in time, with the sides shown instead.
    pub async fn expect_bracket_waiting(&self, sides: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".bracket .side.pending"), sides)
            .await
    }

    /// Waits until the cells of the row of the cross table numbered `row`, from 1, are `cells`,
    /// each as shown followed by what only a screen reader gets on a line of its own, e.g.
    /// `"1\nwon"`, `""` for a match without a result and against itself.
    ///
    /// # Panics
    /// Panics if they are not in time, with the cells shown instead.
    pub async fn expect_cross_table_row(&self, row: usize, cells: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css(format!("#cross-table-grid tbody tr:nth-child({row}) td")),
                cells,
            )
            .await
    }

    /// Waits until the numbers of the bracket's pairings are `numbers`, in the order of the
    /// bracket, scheduled or not, e.g. `"3"`.
    ///
    /// # Panics
    /// Panics if they are not in time, with the numbers shown instead.
    pub async fn expect_bracket_numbers(&self, numbers: &[&str]) -> WebDriverResult<()> {
        // each after the hidden word "Match", which is a block of its own
        let texts: Vec<_> = numbers
            .iter()
            .map(|number| format!("Match\n{number}"))
            .collect();
        let texts: Vec<_> = texts.iter().map(String::as_str).collect();
        self.page
            .expect_texts(By::Css(".bracket .pairing > .number:not(:empty)"), &texts)
            .await
    }

    /// Reports that the side drawn on top of the match numbered `number` in the bracket won, by a
    /// click on its name.
    pub async fn win_in_bracket(&self, number: &str) -> WebDriverResult<()> {
        // the number's own text, after the hidden word "Match"
        let number = format!("span[@class = 'number'][normalize-space(text()) = '{number}']");
        let pairing = format!("//div[contains(@class, 'pairing')][{number}]");
        self.driver
            .query(By::XPath(format!("({pairing}//button)[1]")))
            .and_displayed()
            .first()
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the focus is on a side of the match numbered `number` in the bracket.
    ///
    /// # Panics
    /// Panics if it is not in time, with the number of the match it is on instead, if any.
    pub async fn expect_focus_in_bracket(&self, number: &str) -> WebDriverResult<()> {
        // the number follows the hidden word "Match", which is a block of its own
        self.page
            .expect_texts(
                By::Css(".pairing:has(:focus) > .number"),
                &[&format!("Match\n{number}")],
            )
            .await
    }

    /// Reports the result `entry` says by typing it, e.g. `"3 1"` or `"ali"`, in place of what was
    /// typed before, and pressing Enter; opens the entry of results first if it is closed.
    pub async fn enter_result(&self, entry: &str) -> WebDriverResult<()> {
        self.type_result(entry).await?;
        self.driver
            .query(By::Id("result-entry"))
            .first()
            .await?
            .send_keys(Key::Enter)
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Types `entry` into the entry of results, in place of what was typed before, without
    /// reporting it; opens the entry first if it is closed.
    pub async fn type_result(&self, entry: &str) -> WebDriverResult<()> {
        let open = By::Css("#result-entry-toggle[aria-expanded=true]");
        if self.driver.find(open).await.is_err() {
            self.toggle_result_entry().await?;
        }
        let input = self.driver.query(By::Id("result-entry")).first().await?;
        // selected, so that typing replaces it, as an operator would
        input.send_keys(Key::Control + "a").await?;
        input.send_keys(entry).await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the entry of results says what it would report, or why nothing, `text`, e.g.
    /// "Reports match 1: Alice won.", or says nothing for `None`, as while nothing is typed.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_result_preview(&self, text: Option<&str>) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css("#result-entry-preview:not(:empty)"),
                &Vec::from_iter(text),
            )
            .await
    }

    /// Opens the entry of results typed in if it is closed, or else closes it.
    pub async fn toggle_result_entry(&self) -> WebDriverResult<()> {
        self.driver
            .query(By::Id("result-entry-toggle"))
            .first()
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the entry of results typed in is open, if `open`, or else closed, and the button
    /// showing it says so.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_result_entry_open(&self, open: bool) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css(format!("#result-entry-toggle[aria-expanded={open}]")),
                |query| query,
                &format!("the button showing the entry of results expanded: {open}"),
            )
            .await?;
        let input = By::Id("result-entry");
        if open {
            return self
                .page
                .expect(
                    input,
                    |query| query.and_displayed(),
                    "the entry of results open",
                )
                .await;
        }
        let shown = self.driver.query(input).and_displayed();
        assert!(
            shown.wait_until_gone().await.is_ok(),
            "the entry of results must be closed"
        );
        Ok(())
    }

    /// Waits until the hint of the open entry of results is folded behind a toggletip, if
    /// `folded`, or else shows below the entry.
    ///
    /// # Panics
    /// Panics if it does not in time.
    pub async fn expect_result_entry_hint_folded(&self, folded: bool) -> WebDriverResult<()> {
        let (by, expected) = if folded {
            (
                ".result-entry .toggletip-text#result-entry-hint",
                "the hint folded behind a toggletip",
            )
        } else {
            ("p#result-entry-hint", "the hint shown")
        };
        self.page.expect(By::Css(by), |query| query, expected).await
    }

    /// Waits until neither the entry of results nor the button showing it is on the page, as
    /// once every match has a result.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_no_result_entry(&self) -> WebDriverResult<()> {
        let entry = self
            .driver
            .query(By::Css("#result-entry, #result-entry-toggle"));
        assert!(
            entry.wait_until_gone().await.is_ok(),
            "the entry of results must be gone"
        );
        Ok(())
    }

    /// Waits until the form of the phase shows the field labelled `label`, if `shown`, or else
    /// until it does not.
    ///
    /// # Panics
    /// Panics if it does not in time.
    pub async fn expect_field_shown(&self, label: &str, shown: bool) -> WebDriverResult<()> {
        let field = By::XPath(format!(
            "//form//label[{}='{label}']",
            without_isolation_marks("normalize-space()")
        ));
        if shown {
            return self
                .page
                .expect(field, |query| query.and_displayed(), "the field shown")
                .await;
        }
        let gone = self.driver.query(field).and_displayed();
        assert!(
            gone.wait_until_gone().await.is_ok(),
            "the field {label:?} must not be shown"
        );
        Ok(())
    }

    /// Waits until the phase says how many rounds and matches it takes, `text`, ignoring
    /// [`ISOLATION_MARKS`], or says nothing for `None`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_phase_scale(&self, text: Option<&str>) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css("#phase-scale[role=status]:not(:empty)"),
                &Vec::from_iter(text),
            )
            .await
    }

    /// Waits until the help explains the plugin labelled `label` as `description`.
    ///
    /// # Panics
    /// Panics if it does not in time.
    pub async fn expect_explained(&self, label: &str, description: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::XPath(format!(
                    "//dl[@class='glossary']/dt[.='{label}']\
                     /following-sibling::dd[1][.='{description}']"
                )),
                |query| query,
                &format!("{label:?} explained as {description:?}"),
            )
            .await
    }

    /// Presses `key` where the focus is, e.g. [`Key::Enter`] on a button.
    pub async fn press(&self, key: Key) -> WebDriverResult<()> {
        self.driver.active_element().await?.send_keys(key).await?;
        Page::pause().await;
        Ok(())
    }

    /// Types `text` where the focus is, e.g. into a value whose text is selected, replacing it.
    pub async fn type_text(&self, text: &str) -> WebDriverResult<()> {
        self.driver.active_element().await?.send_keys(text).await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the field with the focus has the value `text`, e.g. the value of a result.
    ///
    /// # Panics
    /// Panics if it does not in time.
    pub async fn expect_focus_value(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css(":focus"),
                |query| query.with_value(text.to_owned()),
                &format!("the focus on the value {text:?}"),
            )
            .await
    }

    /// Waits until the element with the focus shows `text`, e.g. a button.
    ///
    /// # Panics
    /// Panics if it does not in time, with what the element with the focus shows instead.
    pub async fn expect_focus(&self, text: &str) -> WebDriverResult<()> {
        self.page.expect_texts(By::Css(":focus"), &[text]).await
    }

    /// Waits until the focus is on the element whose accessible name, its `aria-label`, is
    /// `label`, e.g. the button `"Remove Carol"`.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_focus_labelled(&self, label: &str) -> WebDriverResult<()> {
        let expected = label.to_owned();
        self.page
            .expect(
                By::Css(":focus"),
                |query| {
                    query.with_attribute("aria-label", move |found: &str| {
                        found.replace(ISOLATION_MARKS, "") == expected
                    })
                },
                &format!("the label {label:?}"),
            )
            .await
    }

    /// Waits until the focus is in the field of a new entrant's name.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_focus_on_entrant_name(&self) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css("#entrant-name:focus"),
                |query| query,
                "the focus in the field of a new entrant's name",
            )
            .await
    }

    /// Waits until the focus is in the line of the match numbered `number` of the list shown, e.g.
    /// on a button reporting its result.
    ///
    /// # Panics
    /// Panics if it is not in time, with the number of the match it is in instead, if any.
    pub async fn expect_focus_in_match(&self, number: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#match-list > li:has(:focus) .number"), &[number])
            .await
    }

    /// Waits until the page shows what the last action did, `text`, in a toast at the bottom of
    /// the window, which takes no room of the page.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_toast(&self, text: &str) -> WebDriverResult<()> {
        self.page.expect_texts(By::Css(".toast"), &[text]).await
    }

    /// Unfolds the warning that the browser may not keep the competitions, which is folded to its
    /// first line until the operator unfolds it.
    pub async fn unfold_storage_warning(&self) -> WebDriverResult<()> {
        self.driver
            .query(By::Css(".warning details:not([open]) summary"))
            .first()
            .await?
            .click()
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the page announces that the last action did `text`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it announces instead.
    pub async fn expect_announcement(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".announcement"), &[text])
            .await
    }

    /// Waits until the standings name `names`, best first.
    ///
    /// # Panics
    /// Panics if they do not in time, with those they name instead.
    pub async fn expect_standings(&self, names: &[&str]) -> WebDriverResult<()> {
        // the headers of the rows
        self.page
            .expect_texts(By::Css("#standing-list tbody th[scope=row]"), names)
            .await
    }

    /// Waits until the headers of the columns of the standings are `headers`, in order, e.g.
    /// `"Rank"`.
    ///
    /// # Panics
    /// Panics if they are not in time, with the headers shown instead.
    pub async fn expect_standings_headers(&self, headers: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#standing-list thead th"), headers)
            .await
    }

    /// Checks that the metrics of each row of the standings without a match played recede behind
    /// its name, in another colour, and that those of a row with one do not.
    ///
    /// # Panics
    /// Panics if a row is not so, with its name.
    pub async fn expect_metrics_muted_until_played(&self) -> WebDriverResult<()> {
        let wrong = self
            .driver
            .execute(
                r##"
                return [...document.querySelectorAll("#standing-list tbody tr")].flatMap((row) => {
                    const name = row.querySelector("th");
                    const played = row.querySelector("td.played").textContent.trim() !== "0";
                    const colour = getComputedStyle(name).color;
                    const muted = [...row.querySelectorAll("td.metric")]
                        .some((cell) => getComputedStyle(cell).color !== colour);
                    return muted === played ? [name.textContent] : [];
                });
                "##,
                Vec::new(),
            )
            .await?
            .convert::<Vec<String>>()?;
        assert!(
            wrong.is_empty(),
            "the metrics must be muted until a match is played, but are not so for {wrong:?}"
        );
        Ok(())
    }

    /// Waits until the table of the standings is captioned `text` for screen readers.
    ///
    /// # Panics
    /// Panics if it is not in time, with its caption instead.
    pub async fn expect_standings_caption(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("#standing-list > caption"), &[text])
            .await
    }

    /// Waits until a group of controls is named `label` (`role=group`, `aria-label`), e.g. a
    /// switch of what is shown.
    ///
    /// # Panics
    /// Panics if none is in time.
    pub async fn expect_group(&self, label: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css(format!("[role=group][aria-label='{label}']")),
                |query| query,
                &format!("a group named {label:?}"),
            )
            .await
    }

    /// Waits until the button reporting a draw of the match of `a` and `b` is named by them, e.g.
    /// "Alice – Bob: draw", in the order the match has them, whichever it is.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_draw_named(&self, a: &str, b: &str) -> WebDriverResult<()> {
        let names = [format!("{a} – {b}: draw"), format!("{b} – {a}: draw")];
        self.page
            .expect(
                By::Css("button.draw"),
                |query| {
                    query.with_attribute("aria-label", move |found: &str| {
                        names.contains(&found.replace(ISOLATION_MARKS, ""))
                    })
                },
                "a draw named by its match",
            )
            .await
    }

    /// Waits until a button shows `label` under the pointer (`title`), as one showing only its icon
    /// does, e.g. `"Rename Cup"`.
    ///
    /// # Panics
    /// Panics if none is in time.
    pub async fn expect_titled(&self, label: &str) -> WebDriverResult<()> {
        self.driver
            .query(By::XPath(format!(
                "//button[{}='{label}']",
                without_isolation_marks("@title")
            )))
            .first()
            .await?;
        Ok(())
    }

    /// Waits until what the entry of results says it would report is a live region
    /// (`role=status`), there before anything is typed, so that a screen reader says it as it
    /// changes.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_result_preview_live(&self) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css("#result-entry-preview[role=status]"),
                |query| query,
                "the preview of the entry of results as a live region",
            )
            .await
    }

    /// Waits until the hint of the bracket on reporting a result says `text`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_bracket_hint(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[aria-labelledby=bracket] > .hint"), &[text])
            .await
    }

    /// Waits until the places of the standings that stand out once every match is played, the
    /// first three, name `names`, best first; none while a match has no result.
    ///
    /// # Panics
    /// Panics if they do not in time, with those they name instead.
    pub async fn expect_podium(&self, names: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css("#standing-list tbody tr.podium th[scope=row]"),
                names,
            )
            .await
    }

    /// Waits until the medals of the standings are `medals`, best first, as a screen reader says
    /// them, e.g. "Gold medal,"; none while a match has no result.
    ///
    /// # Panics
    /// Panics if they are not in time, with the medals shown instead.
    pub async fn expect_medals(&self, medals: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css("#standing-list td.rank .medal .visually-hidden"),
                medals,
            )
            .await
    }

    /// Waits until the screen says who won, `text`, e.g. "Winner: Alice", or says nothing of it for
    /// `None`, while a match has no result.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_winners(&self, text: Option<&str>) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".winners-banner p"), &Vec::from_iter(text))
            .await
    }

    /// Waits until the warning that the browser may not keep the competitions says `texts`: its
    /// first line, and one per paragraph unfolded, or until it is not shown for none.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_storage_warning(&self, texts: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".warning summary, .warning details[open] p"), texts)
            .await
    }

    /// Chooses the option with the text `text` of the select whose id is `id`.
    async fn choose(&self, id: &str, text: &str) -> WebDriverResult<()> {
        let select = self.driver.query(By::Id(id)).first().await?;
        SelectElement::new(&select)
            .await?
            .select_by_visible_text(text)
            .await?;
        Page::pause().await;
        Ok(())
    }

    fn button(&self, text: &str) -> e2e_harness::thirtyfour::extensions::query::ElementQuery {
        self.driver.query(By::XPath(format!(
            "//button[{}='{text}']",
            without_isolation_marks("normalize-space()")
        )))
    }

    /// The option labelled `label` of the list of plugins open.
    fn option(&self, label: &str) -> e2e_harness::thirtyfour::extensions::query::ElementQuery {
        self.driver
            .query(By::XPath(format!(
                "//*[@role='option'][.//*[@class='option-label'][normalize-space()='{label}']]"
            )))
            .and_displayed()
    }

    /// The button whose accessible name, its `aria-label`, is `label`.
    /// Types `text` into the value of a result in the dialog open labelled `label`, e.g.
    /// `"Score of Alice"`, in place of what it shows.
    pub async fn enter_value(&self, label: &str, text: &str) -> WebDriverResult<()> {
        let input = self
            .driver
            .query(By::XPath(format!(
                "//*[@role='dialog']//input[{}='{label}']",
                without_isolation_marks("@aria-label")
            )))
            .first()
            .await?;
        input.clear().await?;
        input.send_keys(text).await?;
        Page::pause().await;
        Ok(())
    }

    /// Reports the result entered in the dialog open, by its button, rather than by that of the
    /// row of the match below it.
    pub async fn report_in_dialog(&self) -> WebDriverResult<()> {
        let button = self
            .driver
            .query(By::XPath(format!(
                "//*[@role='dialog']//button[{}='Report']",
                without_isolation_marks("normalize-space()")
            )))
            .and_enabled()
            .first()
            .await?;
        self.click_in_sight(&button).await
    }

    /// Types `text` into the value of a result in the row of its match labelled `label`, e.g.
    /// `"Score of Alice, match 1"`, in place of what it shows.
    pub async fn enter_in_row(&self, label: &str, text: &str) -> WebDriverResult<()> {
        let input = self.value_in_row(label).first().await?;
        input.clear().await?;
        input.send_keys(text).await?;
        Page::pause().await;
        Ok(())
    }

    /// Types `text` into the first value of the result of the match `number` in its row, whichever
    /// side it is of, in place of what it shows, and stays there, e.g. to press Tab next.
    pub async fn enter_first_in_row(&self, number: usize, text: &str) -> WebDriverResult<()> {
        let input = self
            .values_in_row_ending(&format!(", match {number}"))
            .first()
            .await?;
        input.clear().await?;
        input.send_keys(text).await?;
        Page::pause().await;
        Ok(())
    }

    /// Types `text` into the value of the side `side` (0 the first shown, 1 the second) of `part`
    /// in the row of its match, e.g. `"Game 2, match 1"`, in place of what it shows, and stays
    /// there, e.g. to press Tab next.
    pub async fn enter_in_part_in_row(
        &self,
        part: &str,
        side: usize,
        text: &str,
    ) -> WebDriverResult<()> {
        let values = self.values_of_part_in_row(part).all_from_selector().await?;
        let input = values.get(side).expect("a value of each side of the part");
        input.clear().await?;
        input.send_keys(text).await?;
        Page::pause().await;
        Ok(())
    }

    /// Types `text` into the first value of a result in the dialog open, whichever side it is
    /// of, in place of what it shows, and stays there, e.g. to press Tab next.
    pub async fn enter_first_value(&self, text: &str) -> WebDriverResult<()> {
        let input = self
            .driver
            .query(By::Css("[role=dialog] input[type=number]"))
            .first()
            .await?;
        input.clear().await?;
        input.send_keys(text).await?;
        Page::pause().await;
        Ok(())
    }

    /// Presses Enter in the value of a result in the row of its match labelled `label`, which
    /// reports what the row shows.
    pub async fn press_enter_in_row(&self, label: &str) -> WebDriverResult<()> {
        self.value_in_row(label)
            .first()
            .await?
            .send_keys(Key::Enter)
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the rows of results entered say the results their values suggest,
    /// `outcomes`, e.g. `"Alice wins"`; none where nothing is entered.
    ///
    /// # Panics
    /// Panics if they do not in time, with what they say instead.
    pub async fn expect_row_outcomes(&self, outcomes: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".quick-result output:not(:empty)"), outcomes)
            .await
    }

    /// Waits until the rows of results entered say `outcomes`, as [`Self::expect_row_outcomes`]
    /// does, with each score in them read from either side, e.g. `"Alice wins (2–1)"` or
    /// `"Alice wins (1–2)"`: a test must not depend on the sides.
    ///
    /// # Panics
    /// Panics if they do not in time, with what they say instead.
    pub async fn expect_row_outcomes_either_way(&self, outcomes: &[&str]) -> WebDriverResult<()> {
        let mirrored: Vec<String> = outcomes.iter().map(|outcome| mirrored(outcome)).collect();
        let mirrored: Vec<&str> = mirrored.iter().map(String::as_str).collect();
        self.page
            .expect_texts_of(
                By::Css(".quick-result output:not(:empty)"),
                &[outcomes, &mirrored],
            )
            .await
    }

    /// Waits until the value of a result labelled `label` is in the row of its match, e.g. a game
    /// of a best of once the match needs it, or is not there, as `shown` says.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_in_row(&self, label: &str, shown: bool) -> WebDriverResult<()> {
        if shown {
            return self
                .page
                .expect(
                    By::XPath(format!(
                        "//*[contains(@class, 'quick-result')]//input[{}='{label}']",
                        without_isolation_marks("@aria-label")
                    )),
                    |query| query.and_displayed(),
                    &format!("the value {label:?} in its row"),
                )
                .await;
        }
        assert!(
            self.value_in_row(label).wait_until_gone().await.is_ok(),
            "the value {label:?} must not be in its row"
        );
        Ok(())
    }

    /// Presses Tab in the last value of `part` in the row of its match, e.g. `"Game 2, match 1"`,
    /// whichever side it is of.
    pub async fn tab_out_of_part_in_row(&self, part: &str) -> WebDriverResult<()> {
        let values = self.values_of_part_in_row(part).all_from_selector().await?;
        values
            .last()
            .expect("a value of the part")
            .send_keys(Key::Tab)
            .await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the focus is on a value of `part` in the row of its match, e.g.
    /// `"Game 3, match 1"`.
    ///
    /// # Panics
    /// Panics if it is not in time.
    pub async fn expect_focus_in_part_in_row(&self, part: &str) -> WebDriverResult<()> {
        let suffix = format!(" in {part}");
        self.page
            .expect(
                By::Css(":focus"),
                |query| {
                    query.with_attribute("aria-label", move |found: &str| {
                        found.replace(ISOLATION_MARKS, "").ends_with(&suffix)
                    })
                },
                &format!("a value of {part:?}"),
            )
            .await
    }

    /// Waits until the rows of results entered point out `warnings`.
    ///
    /// # Panics
    /// Panics if they do not in time, with what they point out instead.
    pub async fn expect_row_warnings(&self, warnings: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".quick-result .warnings"), warnings)
            .await
    }

    /// Waits until the rows of results entered say why values typed into them are not taken,
    /// `issues`.
    ///
    /// # Panics
    /// Panics if they do not in time, with what they say instead.
    pub async fn expect_row_issues(&self, issues: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css(".quick-result .issues p"), issues)
            .await
    }

    /// Waits until a value of a result marked as invalid (`aria-invalid`), in the row of its match
    /// or in the dialog open, shows `text`, e.g. a count too large typed there.
    ///
    /// # Panics
    /// Panics if none does in time.
    pub async fn expect_marked_invalid(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect(
                By::Css(
                    ".quick-value input[aria-invalid=true], \
                     [role=dialog] input[aria-invalid=true]",
                ),
                |query| query.with_value(text.to_owned()),
                &format!("a value {text:?} marked as invalid"),
            )
            .await
    }

    /// Waits until the row of a result says why it was not reported, `text`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_row_error(&self, text: &str) -> WebDriverResult<()> {
        self.page
            .expect_texts(
                By::Css(".quick-result .error[role=alert]:not(:empty)"),
                &[text],
            )
            .await
    }

    /// The value of a result in the row of its match labelled `label`.
    fn value_in_row(
        &self,
        label: &str,
    ) -> e2e_harness::thirtyfour::extensions::query::ElementQuery {
        self.driver.query(By::XPath(format!(
            "//*[contains(@class, 'quick-result')]//input[{}='{label}']",
            without_isolation_marks("@aria-label")
        )))
    }

    /// The values of `part` in the row of its match, e.g. `"Game 2, match 1"`, in the order shown.
    fn values_of_part_in_row(
        &self,
        part: &str,
    ) -> e2e_harness::thirtyfour::extensions::query::ElementQuery {
        self.values_in_row_ending(&format!(" in {part}"))
    }

    /// The values of results in the rows of their matches whose labels end with `suffix`, in the
    /// order shown.
    fn values_in_row_ending(
        &self,
        suffix: &str,
    ) -> e2e_harness::thirtyfour::extensions::query::ElementQuery {
        // XPath 1.0 has no ends-with
        let label = without_isolation_marks("@aria-label");
        let length = suffix.chars().count();
        self.driver.query(By::XPath(format!(
            "//*[contains(@class, 'quick-result')]//input[substring({label}, \
             string-length({label}) - {length} + 1) = '{suffix}']"
        )))
    }

    /// Chooses `option` in the choice labelled `label` of the result in the dialog open, e.g.
    /// `"Alice wins"` for `"Result: Match"`.
    pub async fn choose_result(&self, label: &str, option: &str) -> WebDriverResult<()> {
        let select = self
            .driver
            .query(By::XPath(format!(
                "//*[@role='dialog']//div[@class='result-choice'][label[{}='{label}']]/select",
                without_isolation_marks("normalize-space()")
            )))
            .first()
            .await?;
        let option = select
            .find(By::XPath(format!(
                "option[{}='{option}']",
                without_isolation_marks("normalize-space()")
            )))
            .await?;
        option.click().await?;
        Page::pause().await;
        Ok(())
    }

    /// Waits until the dialog of a result open shows the results of its parts not overridden as
    /// `outcomes`, e.g. `"Match: Bob wins"`, the parts in order and the match last.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_result_outcomes(&self, outcomes: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=dialog] .result-outcome output"), outcomes)
            .await
    }

    /// Waits until the dialog of a result open shows `outcomes`, as
    /// [`Self::expect_result_outcomes`] does, with each score in them read from either side, e.g.
    /// `"Match: Alice wins (2–1)"` or `"Match: Alice wins (1–2)"`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_result_outcomes_either_way(
        &self,
        outcomes: &[&str],
    ) -> WebDriverResult<()> {
        let mirrored: Vec<String> = outcomes.iter().map(|outcome| mirrored(outcome)).collect();
        let mirrored: Vec<&str> = mirrored.iter().map(String::as_str).collect();
        self.page
            .expect_texts_of(
                By::Css("[role=dialog] .result-outcome output"),
                &[outcomes, &mirrored],
            )
            .await
    }

    /// Waits until the dialog of a finished match, opened by its details, shows the values of its
    /// parts as one of `alternatives`, those of each part in turn, each side's in the order of the
    /// sides, e.g. `["21", "17", "19", "21"]` of two games; those of the winner of each part
    /// marked as `winners`, in the same order; and its result `outcome`, e.g. `"Match: Alice wins
    /// (2–1)"`, with each score in it read from either side. Take one alternative for each side
    /// Alice may be on.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it shows instead.
    pub async fn expect_reported_result(
        &self,
        alternatives: &[&[&str]],
        winners: &[&str],
        outcome: &str,
    ) -> WebDriverResult<()> {
        self.page
            .expect_texts_of(By::Css("#result-details tbody td"), alternatives)
            .await?;
        self.page
            .expect_texts(By::Css("#result-details tbody td.won"), winners)
            .await?;
        self.page
            .expect_texts_of(
                By::Css("#result-details .reported-outcome"),
                &[&[outcome], &[&mirrored(outcome)]],
            )
            .await
    }

    /// Waits until the dialog of a result open offers to choose the results labelled `labels`,
    /// e.g. `"Result: Match"`, as while overriding them; none if not overriding.
    ///
    /// # Panics
    /// Panics if it does not in time, with the choices offered instead.
    pub async fn expect_result_choices(&self, labels: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=dialog] .result-choice label"), labels)
            .await
    }

    /// Waits until the dialog of a result open points out `warnings`, in the order of its fields.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it points out instead.
    pub async fn expect_result_warnings(&self, warnings: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=dialog] .warnings"), warnings)
            .await
    }

    /// Waits until the dialog open says why values typed into it are not taken, `issues`.
    ///
    /// # Panics
    /// Panics if it does not in time, with what it says instead.
    pub async fn expect_result_issues(&self, issues: &[&str]) -> WebDriverResult<()> {
        self.page
            .expect_texts(By::Css("[role=dialog] .issues p"), issues)
            .await
    }

    fn labelled(&self, label: &str) -> e2e_harness::thirtyfour::extensions::query::ElementQuery {
        self.driver.query(By::XPath(format!(
            "//button[{}='{label}']",
            without_isolation_marks("@aria-label")
        )))
    }
}

/// `text` with each score in it read from the other side, e.g. `"17–21 21–19"` of `"21–17 19–21"`,
/// `"1–2 (17–21 21–19 15–21)"` of `"2–1 (21–17 19–21 21–15)"`, or `"Alice wins (1–2)"` of
/// `"Alice wins (2–1)"`; words without an en dash stay.
fn mirrored(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let score = word.trim_start_matches('(').trim_end_matches(')');
            match score.split_once('–') {
                Some((a, b)) => word.replacen(score, &format!("{b}–{a}"), 1),
                None => word.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The id of the element of the field at `path` of the form of a phase, as the GUI makes it, e.g.
/// `field-metrics-1-points_for_win` for `metrics[1].points_for_win`.
fn field_id(path: &str) -> String {
    let path = path.replace(['.', '[', ']'], "-").replace("--", "-");
    format!("field-{}", path.trim_end_matches('-'))
}

/// The XPath of the string `expression` without the [`ISOLATION_MARKS`] in it.
fn without_isolation_marks(expression: &str) -> String {
    let marks: String = ISOLATION_MARKS.iter().collect();
    format!("translate({expression}, '{marks}', '')")
}
