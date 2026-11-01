//! The help, at `#help`: how a competition is run, what each format, criterion of the standings
//! and metric is, that the competitions are kept in the browser and nowhere else, what follows
//! from that, where the source code is, and who owns the page. `#help-<section>` opens it at the
//! section whose heading has that id, e.g. [`RUNNING`].

use dioxus::prelude::*;
use tournament_app::{
    i18n::{PluginText, Translator},
    session::Session,
    view::Plugin,
};

use crate::{Page, REPOSITORY, capitalized, i18n::t, icons, sentence};

/// The media owner the help names, name and town (e.g. `Jane Doe, Vienna`), from the environment
/// variable `TOURNAMO_MEDIA_OWNER` at build time; without it (or with it empty) the help names
/// none.
const MEDIA_OWNER: Option<&str> = option_env!("TOURNAMO_MEDIA_OWNER");

/// The fragment of the URL of the section on running a competition.
pub const RUNNING: &str = "help-running";

/// The fragment of the URL of the section on the formats and the standings.
pub const FORMATS: &str = "help-formats";

#[component]
pub fn Help() -> Element {
    let page = use_context::<Page>();
    // at the section the URL names, once it is shown and styled, without which it would be
    // elsewhere
    use_effect(|| {
        document::eval(
            "const id = decodeURIComponent(location.hash.slice(1));
            const styles = [...document.querySelectorAll('link[rel=stylesheet]')]
                .filter((link) => !link.sheet)
                .map((link) => new Promise((loaded) => {
                    link.addEventListener('load', loaded, { once: true });
                    link.addEventListener('error', loaded, { once: true });
                }));
            await Promise.all(styles);
            requestAnimationFrame(() => document.getElementById(id)?.scrollIntoView());",
        );
    });
    let tr = page.translator.read();
    let glossary = Session::glossary();
    rsx! {
        main { class: "narrow help",
            section { class: "card", aria_labelledby: RUNNING,
                h2 { id: RUNNING, {tr.get("help-running")} }
                p { {tr.get("help-running-steps")} }
                ol {
                    li { {tr.get("help-running-entrants")} }
                    li { {tr.get("help-running-phase")} }
                    li { {tr.get("help-running-start")} }
                    li { {tr.get("help-running-results")} }
                }
                p { {tr.get("help-running-mistakes")} }
                p { {tr.get("help-running-byes")} }
            }
            section { class: "card", aria_labelledby: FORMATS,
                h2 { id: FORMATS, {tr.get("help-formats")} }
                Glossary { plugins: glossary.formats }
                h3 { {tr.get("help-standings")} }
                p { {tr.get("help-standings-text")} }
                h4 { {tr.get("help-criteria")} }
                Glossary { plugins: glossary.criteria }
                h4 { {tr.get("help-metrics")} }
                Glossary { plugins: glossary.metrics }
            }
            section { class: "card", aria_labelledby: "help-stored",
                h2 { id: "help-stored", {tr.get("help-stored")} }
                p { {tr.get("help-stored-here")} }
                p { {tr.get("help-stored-hosting")} }
                p { {tr.get("help-stored-lost")} }
                p { {tr.get("help-stored-export")} }
            }
            section { class: "card", aria_labelledby: "help-offline",
                h2 { id: "help-offline", {tr.get("help-offline")} }
                p { {tr.get("help-offline-text")} }
                p { {tr.get("help-tabs")} }
            }
            section { class: "card", aria_labelledby: "help-open-source",
                h2 { id: "help-open-source", {tr.get("help-open-source")} }
                p { {tr.get("help-open-source-text")} }
                p {
                    a { href: REPOSITORY,
                        icons::GitHub {}
                        {tr.get("source-code")}
                    }
                }
                if let Some(owner) = MEDIA_OWNER.filter(|owner| !owner.is_empty()) {
                    p { {t!(tr, "help-media-owner", owner = owner)} }
                }
            }
        }
    }
}

/// `plugins`, each by its label and what is said about it, in the order of their labels in the
/// language shown.
#[component]
fn Glossary(plugins: Vec<Plugin>) -> Element {
    let page = use_context::<Page>();
    let tr = page.translator.read();
    let mut entries: Vec<_> = plugins.iter().map(|plugin| entry(&tr, plugin)).collect();
    entries.sort_by_cached_key(|(label, _)| label.to_lowercase());
    rsx! {
        dl { class: "glossary",
            for (label , description) in entries {
                dt { key: "{label}", {label} }
                dd { {description} }
            }
        }
    }
}

/// The label of `plugin` and what is said about it, in the language of `tr`.
fn entry(tr: &Translator, plugin: &Plugin) -> (String, String) {
    let label = capitalized(&tr.plugin(plugin.id, PluginText::Label, plugin.label));
    let description = plugin
        .description
        .map(|english| sentence(&tr.plugin(plugin.id, PluginText::Description, english)))
        .unwrap_or_default();
    (label, description)
}
