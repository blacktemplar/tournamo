//! The fields of the form of a phase that take a list of plugins: ordered, or a set.

use dioxus::prelude::*;
use tournament_app::{
    form::{
        self, Because, By, Item, ListInput, MetricShown, Path, PhaseForm, PluginOption, SetEntry,
    },
    i18n::Translator,
};
use tournament_engine::Table;

use super::fields::{Fields, Issues, PluginChoice, id, plugin_description, plugin_label};
use crate::{
    capitalized, competition::Ctx, components::select::Select, focus::focus_later, i18n::t, icons,
    toggletip::Toggletip,
};

/// A list of plugins whose order matters: each can be moved, removed and configured, and one more
/// added at the end.
#[component]
pub(super) fn OrderedList(
    form: Signal<PhaseForm>,
    path: Path,
    id: String,
    list: ListInput,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let last = list.items.len().saturating_sub(1);
    let implied = implied(&tr, &list.implied);
    // the plugin added, at the end, or the list adding another one
    let added = format!(
        "#{}-remove",
        self::id(&path.clone().index(list.items.len()))
    );
    let add_trigger = format!("#{id}-add");
    rsx! {
        ol { class: "plugin-list", id: "{id}",
            // keyed by the plugin, so that one moved keeps its element
            for (index, item) in list.items.into_iter().enumerate() {
                OrderedItem {
                    key: "{item.plugin.id}",
                    list: id.clone(),
                    form,
                    list_path: path.clone(),
                    index,
                    item,
                    last: index == last,
                    disabled,
                }
            }
            for implied in implied {
                li { key: "{implied.option.id}", class: "implied",
                    "{implied.text} "
                    About { list: id.clone(), option: implied.option }
                }
            }
        }
        if !list.options.is_empty() {
            div { class: "add",
                span { class: "label", id: "{id}-add-label", {tr.get("add-plugin")} }
                " "
                Select {
                    id: "{id}-add",
                    labelled_by: "{id}-add-label",
                    value: None,
                    placeholder: tr.get("choose-plugin"),
                    disabled,
                    on_change: move |chosen: String| {
                        _ = form.write().add(&path, &chosen);
                        focus_later(&[&add_trigger, &added]);
                    },
                    for (index, option) in list.options.iter().enumerate() {
                        PluginChoice {
                            key: "{option.id}",
                            select: "{id}-add",
                            index,
                            option: *option,
                        }
                    }
                }
            }
        }
    }
}

/// A plugin of a list whose order matters, at `index` of the list at `list_path`, with the buttons
/// moving and removing it, and its fields.
#[component]
fn OrderedItem(
    form: Signal<PhaseForm>,
    list: String,
    list_path: Path,
    index: usize,
    item: Item,
    last: bool,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let name = plugin_label(&tr, &item.plugin);
    let (up, down, remove) = (item.path.clone(), item.path.clone(), item.path.clone());
    // the button of the item at `index`, by its position
    let button = move |index: usize, button: &str| {
        format!("#{}-{button}", id(&list_path.clone().index(index)))
    };
    // moving it moves its element, which loses the focus, to the position it is moved to
    let (moved_up, moved_down) = (
        button(index.saturating_sub(1), "up"),
        button(index + 1, "down"),
    );
    // after the removal the next item is at the position of the one removed, else the one before
    // it is the last, else the list adding one is all there is
    let mut after_removal = vec![button(index, "remove")];
    if let Some(before) = index.checked_sub(1) {
        after_removal.push(button(before, "remove"));
    }
    after_removal.push(format!("#{list}-add"));
    let own = id(&item.path);
    rsx! {
        li {
            span { class: "name", {capitalized(&name)} }
            " "
            About { list, option: item.plugin }
            span { class: "item-buttons",
                // kept focusable at the end of the list, so that the focus stays on a plugin
                // moved there
                button {
                    class: "small icon-only",
                    r#type: "button",
                    id: "{own}-up",
                    aria_label: t!(tr, "move-up", plugin = name.as_str()),
                    title: t!(tr, "move-up", plugin = name.as_str()),
                    disabled,
                    aria_disabled: (index == 0).then_some("true"),
                    onclick: move |_| {
                        _ = form.write().move_up(&up);
                        focus_later(&[&moved_up]);
                    },
                    icons::ArrowUp {}
                }
                button {
                    class: "small icon-only",
                    r#type: "button",
                    id: "{own}-down",
                    aria_label: t!(tr, "move-down", plugin = name.as_str()),
                    title: t!(tr, "move-down", plugin = name.as_str()),
                    disabled,
                    aria_disabled: last.then_some("true"),
                    onclick: move |_| {
                        _ = form.write().move_down(&down);
                        // nothing at the end, where it stays
                        focus_later(&[&moved_down]);
                    },
                    icons::ArrowDown {}
                }
                // an icon, so that the buttons fit beside the name on a phone
                button {
                    class: "small icon-only",
                    r#type: "button",
                    id: "{own}-remove",
                    aria_label: t!(tr, "remove-plugin", plugin = name.as_str()),
                    title: t!(tr, "remove-plugin", plugin = name.as_str()),
                    disabled,
                    onclick: move |_| {
                        _ = form.write().remove(&remove);
                        let selectors: Vec<&str> =
                            after_removal.iter().map(String::as_str).collect();
                        focus_later(&selectors);
                    },
                    icons::Trash {}
                }
            }
            Fields { form, fields: item.fields, disabled }
            Issues { id: "{id(&item.path)}-issues", issues: item.issues }
        }
    }
}

/// A plugin added to a list since another one needs it.
struct Implied {
    option: PluginOption,
    /// What the list shows for it.
    text: String,
}

/// What a list shows for the plugins of `implied`.
fn implied(tr: &Translator, implied: &[form::Implied]) -> Vec<Implied> {
    implied
        .iter()
        .map(|implied| Implied {
            option: implied.plugin,
            text: implied_text(tr, implied),
        })
        .collect()
}

/// What a list shows for `implied`: its label and what needs it.
fn implied_text(tr: &Translator, implied: &form::Implied) -> String {
    let plugin = plugin_label(tr, &implied.plugin);
    let text = if implied.needed_by.is_empty() {
        t!(tr, "implied-plugin", plugin = plugin.as_str())
    } else {
        let needed_by: Vec<_> = implied
            .needed_by
            .iter()
            .map(|option| plugin_label(tr, option))
            .collect();
        t!(
            tr,
            "implied-plugin-needed-by",
            plugin = plugin.as_str(),
            needed_by = needed_by.join(&tr.get("list-separator")),
        )
    };
    capitalized(&text)
}

/// The toggletip saying the description of `option`, a plugin of the list whose id is `list`;
/// nothing if it has none.
#[component]
fn About(list: String, option: PluginOption) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let Some(text) = plugin_description(&tr, &option) else {
        return rsx! {};
    };
    // ids of plugins are namespaced by `::`
    let id = format!("{list}-about-{}", option.id.replace("::", "-"));
    rsx! {
        Toggletip { id, subject: plugin_label(&tr, &option), text }
    }
}

/// A set of plugins, each chosen or not by a checkbox, and configured once chosen.
#[component]
pub(super) fn UnorderedList(
    form: Signal<PhaseForm>,
    path: Path,
    list_id: String,
    list: ListInput,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    // in one order, whether chosen or not, so that a plugin stays where it is when it is clicked
    let entries: Vec<_> = list
        .set()
        .into_iter()
        .map(|entry| match entry {
            SetEntry::Chosen(item) => SetItem {
                option: item.plugin,
                text: capitalized(&plugin_label(&tr, &item.plugin)),
                state: SetState::Chosen(item.clone()),
            },
            SetEntry::Implied(implied) => SetItem {
                option: implied.plugin,
                text: implied_text(&tr, implied),
                state: SetState::Implied,
            },
            SetEntry::Offered(option) => SetItem {
                option: *option,
                text: capitalized(&plugin_label(&tr, option)),
                state: SetState::Offered,
            },
        })
        .collect();
    rsx! {
        ul { class: "plugin-set", id: "{list_id}",
            // keyed by the plugin, which a list has once, and with the same elements whether
            // chosen or not, so that its checkbox keeps the focus when it is clicked; an implied
            // one has elements of its own, since the page would not check again a checkbox just
            // unchecked whose plugin turned out to be implied, having rendered it checked before
            for entry in entries {
                li { key: "{entry.key()}",
                    {
                        let plugin = entry.option.id;
                        let chosen = match &entry.state {
                            SetState::Chosen(item) => Some(item.path.clone()),
                            _ => None,
                        };
                        let add = path.clone();
                        rsx! {
                            label {
                                input {
                                    r#type: "checkbox",
                                    checked: !matches!(entry.state, SetState::Offered),
                                    disabled: disabled || matches!(entry.state, SetState::Implied),
                                    onchange: move |_| {
                                        _ = match &chosen {
                                            Some(item) => form.write().remove(item),
                                            None => form.write().add(&add, plugin),
                                        };
                                    },
                                }
                                " {entry.text}"
                            }
                        }
                    }
                    " "
                    About { list: list_id.clone(), option: entry.option }
                    if let SetState::Chosen(item) = entry.state {
                        Fields { form, fields: item.fields, disabled }
                        Issues { id: "{id(&item.path)}-issues", issues: item.issues }
                    }
                }
            }
        }
    }
}

/// A plugin of a set, as [`UnorderedList`] shows it.
struct SetItem {
    option: PluginOption,
    state: SetState,
    /// What the set shows for it.
    text: String,
}

impl SetItem {
    /// What its element is keyed by, see [`UnorderedList`].
    fn key(&self) -> String {
        match self.state {
            SetState::Implied => format!("implied-{}", self.option.id),
            _ => self.option.id.to_owned(),
        }
    }
}

#[derive(Clone)]
enum SetState {
    Chosen(Item),
    Implied,
    Offered,
}

/// The metrics of the phase, chosen, implied or offered, each with where it is shown: a checkbox
/// for the standings of the divisions and, where the phase compares its divisions (`compared`),
/// one for the rows compared, as a table with a column per table of standings; otherwise a
/// list of metrics, each with its checkbox. A metric checked is calculated, with its fields
/// below it. A box checked by rule shows an outlined tick, and says why as its description.
#[component]
pub(super) fn MetricsList(
    form: Signal<PhaseForm>,
    list_id: String,
    list: ListInput,
    compared: bool,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    // in one order, whether calculated or not, so that a metric stays where it is when it is
    // clicked; one without a column is listed only while it is calculated, without a box
    let rows: Vec<MetricRow> = list
        .shown
        .iter()
        .filter(|metric| metric.column || metric.calculated)
        .map(|metric| MetricRow::new(&tr, &list, metric))
        .collect();
    let tables: &[Table] = if compared {
        &[Table::Division, Table::Across]
    } else {
        &[Table::Division]
    };
    if !compared {
        return rsx! {
            ul { class: "plugin-set metrics", id: "{list_id}",
                // keyed by the metric, so that its checkbox keeps the focus when it is clicked
                for row in rows {
                    li { key: "{row.metric.plugin.id}",
                        if row.metric.column {
                            label {
                                MetricBox {
                                    form,
                                    list: list_id.clone(),
                                    metric: row.metric.clone(),
                                    table: Table::Division,
                                    name: None,
                                    disabled,
                                }
                                " {row.name}"
                            }
                            WhyShown {
                                list: list_id.clone(),
                                metric: row.metric.clone(),
                                table: Table::Division,
                            }
                        } else {
                            span { "{row.name}" }
                        }
                        if let Some(needed) = &row.needed {
                            " "
                            span { class: "needed", "{needed}" }
                        }
                        " "
                        About { list: list_id.clone(), option: row.metric.plugin }
                        MetricFields { form, row, disabled }
                    }
                }
            }
        };
    }
    rsx! {
        table { class: "metrics-table", id: "{list_id}",
            thead {
                tr {
                    th { scope: "col", {tr.get("metrics-name")} }
                    th { scope: "col", {tr.get("shown-in-divisions")} }
                    th { scope: "col", {tr.get("shown-in-comparison")} }
                }
            }
            // a body per metric, its fields in a row below its own
            for row in rows {
                tbody { key: "{row.metric.plugin.id}",
                    tr {
                        th { scope: "row",
                            span { class: "name", "{row.name}" }
                            if let Some(needed) = &row.needed {
                                " "
                                span { class: "needed", "{needed}" }
                            }
                            " "
                            About { list: list_id.clone(), option: row.metric.plugin }
                        }
                        for &table in tables {
                            td {
                                if row.metric.column {
                                    MetricBox {
                                        form,
                                        list: list_id.clone(),
                                        metric: row.metric.clone(),
                                        table,
                                        name: Some(row.label.clone()),
                                        disabled,
                                    }
                                    WhyShown {
                                        list: list_id.clone(),
                                        metric: row.metric.clone(),
                                        table,
                                    }
                                }
                            }
                        }
                    }
                    if row.item.is_some() {
                        tr { class: "metric-fields",
                            td { colspan: "3", MetricFields { form, row, disabled } }
                        }
                    }
                }
            }
        }
    }
}

/// A metric of [`MetricsList`].
#[derive(Clone, PartialEq)]
struct MetricRow {
    metric: MetricShown,
    /// Its label, as a text goes on.
    label: String,
    /// Its label, standing alone.
    name: String,
    /// What needs it, if anything does.
    needed: Option<String>,
    /// The metric as chosen, with its fields; `None` if it is implied or offered.
    item: Option<Item>,
}

impl MetricRow {
    fn new(tr: &Translator, list: &ListInput, metric: &MetricShown) -> Self {
        let label = plugin_label(tr, &metric.plugin);
        let needed = (!metric.needed_by.is_empty()).then(|| {
            let needed_by: Vec<_> = metric
                .needed_by
                .iter()
                .map(|option| plugin_label(tr, option))
                .collect();
            t!(
                tr,
                "metric-needed-by",
                needed_by = needed_by.join(&tr.get("list-separator")),
            )
        });
        Self {
            metric: metric.clone(),
            name: capitalized(&label),
            label,
            needed,
            item: list
                .items
                .iter()
                .find(|item| item.plugin.id == metric.plugin.id)
                .cloned(),
        }
    }
}

/// The fields of the metric of `row` and their issues, if it is chosen.
#[component]
fn MetricFields(form: Signal<PhaseForm>, row: MetricRow, disabled: bool) -> Element {
    let Some(item) = row.item else {
        return rsx! {};
    };
    rsx! {
        Fields { form, fields: item.fields, disabled }
        Issues { id: "{id(&item.path)}-issues", issues: item.issues }
    }
}

/// The checkbox showing `metric` in `table` or not, of the list of metrics whose id is `list`,
/// named `name` where its label does not name it. Checked by rule, it is described by why, see
/// [`WhyShown`].
#[component]
fn MetricBox(
    form: Signal<PhaseForm>,
    list: String,
    metric: MetricShown,
    table: Table,
    name: Option<String>,
    disabled: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let shown = *metric.of(table);
    let own = box_id(&list, &metric, table);
    let name = name.map(|metric| match table {
        Table::Division => t!(tr, "show-in-divisions", metric = metric.as_str()),
        Table::Across => t!(tr, "show-in-comparison", metric = metric.as_str()),
    });
    let described_by = why_shown(&tr, &metric, table).map(|_| format!("{own}-why"));
    let plugin = metric.plugin.id;
    rsx! {
        input {
            r#type: "checkbox",
            id: "{own}",
            class: if described_by.is_some() { "by-rule" },
            checked: shown.on,
            disabled,
            aria_label: name,
            aria_describedby: described_by,
            onchange: move |event| {
                _ = form.write().set_shown(plugin, table, event.checked());
            },
        }
    }
}

/// Why the checkbox of [`MetricBox`] is checked by rule, hidden, as its description; nothing if
/// it is not.
#[component]
fn WhyShown(list: String, metric: MetricShown, table: Table) -> Element {
    let ctx = use_context::<Ctx>();
    let tr = ctx.translator.read();
    let Some(why) = why_shown(&tr, &metric, table) else {
        return rsx! {};
    };
    rsx! {
        span { id: "{box_id(&list, &metric, table)}-why", hidden: true, "{why}" }
    }
}

/// The id of the checkbox showing `metric` in `table`, of the list of metrics whose id is `list`.
fn box_id(list: &str, metric: &MetricShown, table: Table) -> String {
    let table = match table {
        Table::Division => "divisions",
        Table::Across => "comparison",
    };
    // ids of plugins are namespaced by `::`
    format!("{list}-{table}-{}", metric.plugin.id.replace("::", "-"))
}

/// Why `table` shows `metric` by rule, if it does.
fn why_shown(tr: &Translator, metric: &MetricShown, table: Table) -> Option<String> {
    match metric.of(table).by {
        By::Rule(Some(Because::RankedBy(_))) => Some(tr.get(match table {
            Table::Division => "shown-ranked-in-divisions",
            Table::Across => "shown-ranked-in-comparison",
        })),
        By::Rule(Some(Because::CalculatedFrom(from))) => Some(t!(
            tr,
            "shown-calculated-from",
            metric = plugin_label(tr, &from).as_str()
        )),
        _ => None,
    }
}
