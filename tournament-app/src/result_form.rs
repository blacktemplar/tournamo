//! The result of a match being entered on a screen, as a form generated from the template of its
//! match format.
//!
//! A [`ResultForm`] holds what was entered for each part of a match: the match itself, and its
//! games, sets or rubbers, as deep as the match format divides it. It hands out a [`ResultView`]:
//! every part with its fields and their values, the result chosen or else the one the match format
//! suggests from what was entered, and warnings of what looks like a typo. A screen lays it out and
//! hands what the operator does back to the form, by the path of the part it was done to; once
//! done, it hands the form to [`Session::report_form`](crate::session::Session::report_form).
//!
//! Nothing entered is refused for breaking the rules of a sport: the form only warns of a value
//! beyond what the match format calls usual, and of a result chosen against the one the values
//! suggest.

use tournament_engine::{
    Decision, EnteredValue, FieldKey, MatchId, MatchResult, Part, ResultReason, Sides, Team,
    Values,
    plugins::{
        Phrase,
        match_format::{FieldSpec, Level, MatchFormat, Parts, Template, ValueKind},
    },
};

use crate::i18n::Text;

/// The result of a match being entered, see the [module](self).
#[derive(Clone)]
pub struct ResultForm {
    match_id: MatchId,
    format: Box<dyn MatchFormat>,
    template: Template,
    root: Entry,
}

/// What was entered for a part.
#[derive(Debug, Clone, Default)]
struct Entry {
    /// The result chosen by hand, which overrides the one suggested.
    chosen: Option<MatchResult>,
    reason: Option<ResultReason>,
    values: Values,
    /// One for each part the level can have.
    parts: Vec<Entry>,
}

impl Entry {
    /// An entry with nothing entered for a part at `level`, and for each of its parts.
    fn empty(level: &Level) -> Self {
        let parts = match &level.parts {
            Parts::None => Vec::new(),
            Parts::Same { level, max } => (0..*max).map(|_| Self::empty(level)).collect(),
            Parts::Each(levels) => levels.iter().map(Self::empty).collect(),
        };
        Self {
            parts,
            ..Self::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.chosen.is_none()
            && self.reason.is_none()
            && self.values.is_empty()
            && self.parts.iter().all(Self::is_empty)
    }

    fn at(&mut self, path: &[usize]) -> &mut Self {
        path.iter().fold(self, |entry, &index| {
            entry
                .parts
                .get_mut(index)
                .expect("a path of a part of the view")
        })
    }
}

/// What a [`ResultForm`] shows: plain data for a screen to lay out.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultView {
    /// The id of the match format, whose message translates the phrases of the parts, see
    /// [`Translator::plugin_phrase`](crate::i18n::Translator::plugin_phrase).
    pub match_format: &'static str,
    /// The match itself.
    pub root: PartView,
}

/// A part of a match in a [`ResultView`].
#[derive(Debug, Clone, PartialEq)]
pub struct PartView {
    /// Where the part is: the index of each part from the match down, empty for the match.
    pub path: Vec<usize>,
    /// What the part is called, e.g. `game`.
    pub name: Phrase,
    pub fields: Vec<FieldView>,
    /// The result: the one chosen, or else the one suggested.
    pub result: Option<MatchResult>,
    /// The result the values and parts suggest.
    pub suggested: Option<MatchResult>,
    /// Whether the result was chosen by hand rather than suggested.
    pub chosen: bool,
    /// Why the part may be decided other than in the usual way.
    pub reasons: Vec<(ResultReason, Phrase)>,
    pub reason: Option<ResultReason>,
    /// What looks wrong about the result, though it is taken.
    pub warnings: Vec<Text>,
    pub parts: Vec<PartView>,
}

/// A field of a part in a [`ResultView`].
#[derive(Debug, Clone, PartialEq)]
pub struct FieldView {
    pub key: FieldKey,
    pub label: Phrase,
    pub kind: ValueKind,
    pub values: Sides<Option<EnteredValue>>,
    /// What looks like a typo, though it is taken.
    pub warnings: Vec<Text>,
}

impl ResultForm {
    /// A form with nothing entered for the match `match_id`, whose result looks like `template`,
    /// entered by `format`.
    pub(crate) fn new(match_id: MatchId, format: Box<dyn MatchFormat>, template: Template) -> Self {
        let root = Entry::empty(&template.root);
        Self {
            match_id,
            format,
            template,
            root,
        }
    }

    /// The match whose result this is.
    pub fn match_id(&self) -> MatchId {
        self.match_id
    }

    /// Enters `value` for `team` into the field `field` of the part at `path`, or clears it.
    ///
    /// # Panics
    /// Panics if `path` is no path of a part of the [view](Self::view).
    pub fn set_value(
        &mut self,
        path: &[usize],
        field: &FieldKey,
        team: Team,
        value: Option<EnteredValue>,
    ) {
        let entry = self.root.at(path);
        let index = match entry.values.iter().position(|(key, _)| key == field) {
            Some(index) => index,
            None => {
                entry.values.push((field.clone(), Sides::new(None, None)));
                entry.values.len() - 1
            }
        };
        let sides = &mut entry.values[index].1;
        match team {
            Team::A => sides.a = value,
            Team::B => sides.b = value,
        }
        if sides.a.is_none() && sides.b.is_none() {
            entry.values.remove(index);
        }
    }

    /// Chooses `result` for the part at `path` by hand, or follows what its values and parts
    /// suggest again, with `None`.
    ///
    /// # Panics
    /// Panics if `path` is no path of a part of the [view](Self::view).
    pub fn choose(&mut self, path: &[usize], result: Option<MatchResult>) {
        self.root.at(path).chosen = result;
    }

    /// Says why the part at `path` was decided, if not in the usual way.
    ///
    /// # Panics
    /// Panics if `path` is no path of a part of the [view](Self::view).
    pub fn set_reason(&mut self, path: &[usize], reason: Option<ResultReason>) {
        self.root.at(path).reason = reason;
    }

    /// The result of the match, as the [view](Self::view) shows it: `None` while neither chosen
    /// nor suggested.
    pub fn result(&self) -> Option<MatchResult> {
        self.view().root.result
    }

    pub fn view(&self) -> ResultView {
        ResultView {
            match_format: plugin_factory::id_of(&*self.format),
            root: self
                .part_view(&self.template.root, &self.root, Vec::new())
                .0,
        }
    }

    /// The decision of the match and the values and parts entered for it, the parts after the
    /// last one with anything entered left out: what is reported.
    pub(crate) fn report(&self) -> (Option<Decision>, Values, Vec<Part>) {
        let (_, part) = self.part_view(&self.template.root, &self.root, Vec::new());
        let decision = part.decision().cloned();
        (decision, part.values().clone(), part.parts().to_vec())
    }

    /// The view of the part `entry` makes up at `level`, at `path`, with that part.
    fn part_view(&self, level: &Level, entry: &Entry, path: Vec<usize>) -> (PartView, Part) {
        let mut children = Vec::new();
        let mut views = Vec::new();
        for (index, child) in entry.parts.iter().enumerate() {
            let child_level = level.level_of_part(index).expect("an entry for each part");
            let mut child_path = path.clone();
            child_path.push(index);
            let (view, part) = self.part_view(child_level, child, child_path);
            views.push(view);
            children.push(part);
        }
        let last = entry.parts.iter().rposition(|part| !part.is_empty());
        let mut part = Part::new();
        for (key, values) in &entry.values {
            part = part.with_value(key.clone(), *values);
        }
        for child in children.into_iter().take(last.map_or(0, |last| last + 1)) {
            part = part.with_part(child);
        }
        let suggested = self.format.suggest(level, &part);
        let result = entry.chosen.or(suggested);
        let mut warnings = Vec::new();
        if let (Some(chosen), Some(suggested)) = (entry.chosen, suggested)
            && chosen != suggested
        {
            warnings.push(Text::new("warning-result-disagrees"));
        }
        if let Some(result) = result {
            let decision = Decision::new(result);
            part = part.with_decision(match &entry.reason {
                Some(reason) => decision.with_reason(reason.clone()),
                None => decision,
            });
        }
        let view = PartView {
            path,
            name: level.name.clone(),
            fields: level
                .fields
                .iter()
                .map(|field| field_view(field, &entry.values))
                .collect(),
            result,
            suggested,
            chosen: entry.chosen.is_some(),
            reasons: level
                .reasons
                .iter()
                .map(|spec| (spec.reason.clone(), spec.label.clone()))
                .collect(),
            reason: entry.reason.clone(),
            warnings,
            parts: views,
        };
        (view, part)
    }
}

/// The view of `field`, with its values among `values`.
fn field_view(field: &FieldSpec, values: &Values) -> FieldView {
    let values = values
        .iter()
        .find(|(key, _)| *key == field.key)
        .map_or(Sides::new(None, None), |(_, values)| *values);
    let warnings = [values.a, values.b]
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let max = field.hints.max?;
            above(value, max).then(|| {
                Text::new("warning-above-usual-top")
                    .with("value", number(value))
                    .with("top", number(max))
            })
        })
        .collect();
    FieldView {
        key: field.key.clone(),
        label: field.label.clone(),
        kind: field.kind,
        values,
        warnings,
    }
}

/// Whether `value` is above `max`, both of the same kind.
fn above(value: EnteredValue, max: EnteredValue) -> bool {
    match (value, max) {
        (EnteredValue::Count(value), EnteredValue::Count(max)) => value > max,
        (EnteredValue::Decimal(value), EnteredValue::Decimal(max)) => value > max,
        _ => false,
    }
}

/// `value` as a number of a message.
fn number(value: EnteredValue) -> f64 {
    match value {
        EnteredValue::Count(count) => count.into(),
        EnteredValue::Decimal(decimal) => decimal.get(),
    }
}

#[cfg(test)]
mod tests;
