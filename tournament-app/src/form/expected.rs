//! What each field of a phase is expected to hold: what it offers for the phase as configured,
//! unless the phase of a template it was added as overrides it, see [`PhaseForm::compared_with`].
//! A field holding what it is expected to follows when that changes, see [`PhaseForm`].
//!
//! [`PhaseForm`]: super::PhaseForm
//! [`PhaseForm::compared_with`]: super::PhaseForm::compared_with

use plugin_factory::{
    config::{Config, Path, Value},
    draft::Draft,
};
use tournament_engine::PhasePlugins;

/// The names of the fields of the phase, in schema order.
pub(super) fn fields(draft: &Draft<PhasePlugins>) -> Vec<&'static str> {
    draft
        .fields()
        .iter()
        .map(|node| node.field.name())
        .collect()
}

/// The answers of `answers`, those of the phase of a template, that override what their fields
/// offer: answered with the defaults of their plugins, they differ from what the field offers in
/// the phase of the template, i.e. with its own format and match format. An answer the same as
/// what is offered is no override, and follows what is offered. `draft` is a draft of the phase,
/// whose context the template's phase is configured in.
pub(super) fn overrides(draft: &Draft<PhasePlugins>, answers: &Config) -> Config {
    let names = fields(draft);
    let mut blank = draft.clone();
    for name in &names {
        _ = blank.unset(&Path::ROOT.field(*name));
    }
    let Ok(template) = blank
        .with_answers(answers)
        .and_then(PhasePlugins::with_suggestions)
    else {
        return Config::new();
    };
    let mut overrides = Config::new();
    for name in names {
        let Some(answer) = answers.get(name).and_then(|_| template.answers().get(name)) else {
            continue;
        };
        if Some(answer) != offered(&template, &Path::ROOT.field(name)).as_ref() {
            overrides = overrides.with(name, answer.clone());
        }
    }
    overrides
}

/// What the field at `path`, a field of the phase, offers in `draft`, the plugins of the answer
/// configured as they offer; `None` where it offers nothing.
pub(super) fn offered(draft: &Draft<PhasePlugins>, path: &Path) -> Option<Value> {
    let offered = PhasePlugins::default_value(draft, path).ok()??;
    let mut offering = draft.clone();
    offering.set(path, offered).ok()?;
    offering
        .with_defaults()
        .ok()?
        .answers()
        .get_at(path)
        .cloned()
}

/// The override of `overrides` for the field `name` of the phase, if there is one that still
/// fits `draft` and matters there: the field is not one of `irrelevant`, and answered with it the
/// draft has no more issues than answered with what it offers.
pub(super) fn fitting<'a>(
    draft: &Draft<PhasePlugins>,
    overrides: &'a Config,
    name: &str,
    irrelevant: &[Path],
) -> Option<&'a Value> {
    let answer = overrides.get(name)?;
    let path = Path::ROOT.field(name);
    if irrelevant.contains(&path) {
        return None;
    }
    let issues = |answer: Value| -> Option<usize> {
        let mut answered = draft.clone();
        answered.set(&path, answer).ok()?;
        let mut answered = answered.with_defaults().ok()?;
        answered.add_implied();
        Some(answered.build().err().map_or(0, |issues| issues.len()))
    };
    let with = issues(answer.clone())?;
    let without = offered(draft, &path).and_then(issues).unwrap_or(usize::MAX);
    (with <= without).then_some(answer)
}

/// The overrides of `overrides` that still fit `draft` and matter there, see [`fitting`].
pub(super) fn fitting_all(
    draft: &Draft<PhasePlugins>,
    overrides: &Config,
    irrelevant: &[Path],
) -> Config {
    fields(draft)
        .into_iter()
        .filter_map(|name| {
            fitting(draft, overrides, name, irrelevant).map(|answer| (name, answer.clone()))
        })
        .fold(Config::new(), |config, (name, answer)| {
            config.with(name, answer)
        })
}

/// What the field `name` of the phase is expected to hold in `draft`: the override of
/// `overrides` while it fits, else what it offers.
pub(super) fn expected(
    draft: &Draft<PhasePlugins>,
    overrides: &Config,
    name: &str,
    irrelevant: &[Path],
) -> Option<Value> {
    fitting(draft, overrides, name, irrelevant)
        .cloned()
        .or_else(|| offered(draft, &Path::ROOT.field(name)))
}
