//! The result of a match being entered on a screen, as a form generated from the template of its
//! match format.
//!
//! A [`ResultForm`] holds what was entered for each part of a match: the match itself, and its
//! games, sets or rubbers, as deep as the match format divides it. It hands out a [`ResultView`]:
//! every part played so far with its fields and their values (a game of a best of appears once the
//! match needs it), the result chosen or else the one the match format suggests from what was
//! entered, and warnings of what looks like a typo. A screen lays it out and
//! hands what the operator does back to the form, by the path of the part it was done to; once
//! done, it hands the form to [`Session::report_form`](crate::session::Session::report_form).
//!
//! A match whose result takes no more than a row, e.g. a score or the score of each game, may be
//! entered beside it instead ([`ResultForm::quick_entry`]), into the same form.
//!
//! A result chosen by hand overrides the one suggested, which is rare: a screen may offer the
//! choice only on request, except for a part that only a choice decides
//! ([`PartView::needs_choice`]).
//!
//! Nothing entered is refused for breaking the rules of a sport: the form only warns of a score
//! unlike those the match format hints a winner usually has, of a result chosen against the one the
//! values suggest, and of parts entered after those before them decided the match.

use tournament_engine::{
    Decision, EnteredValue, FieldKey, MatchId, MatchResult, Part, ResultReason, Score, Sides, Team,
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
            Parts::Same { level, max, .. } => (0..*max).map(|_| Self::empty(level)).collect(),
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

    fn is_chosen(&self) -> bool {
        self.chosen.is_some() || self.parts.iter().any(Self::is_chosen)
    }

    fn follow_suggestions(&mut self) {
        self.chosen = None;
        self.parts.iter_mut().for_each(Self::follow_suggestions);
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
    /// Whether only a result chosen decides the part, as it has neither values nor parts that
    /// could suggest one.
    pub needs_choice: bool,
    /// Why the part may be decided other than in the usual way.
    pub reasons: Vec<(ResultReason, Phrase)>,
    pub reason: Option<ResultReason>,
    /// What looks wrong about the result, though it is taken.
    pub warnings: Vec<Text>,
    /// Its parts as far as they are played: of parts at the same level, the fewest that may
    /// decide it and those up to the last one entered, and one more while each of them is decided
    /// but the part is not, e.g. the fourth game of a best of five at 2–1.
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

/// A result entered in one row, beside the match rather than in a form of its own: a value for
/// each side of each field, see [`ResultForm::quick_entry`].
#[derive(Debug, Clone, PartialEq)]
pub struct QuickEntry {
    /// The id of the match format, whose message translates the phrases, as in [`ResultView`].
    pub match_format: &'static str,
    /// The fields in the order they are entered: the one of the match, or the one of each of its
    /// parts.
    pub fields: Vec<QuickField>,
    /// The result the values suggest, `None` while they suggest none.
    pub result: Option<MatchResult>,
    /// What looks like a typo in any field, or wrong about the match, each said once.
    pub warnings: Vec<Text>,
}

/// A field of a [`QuickEntry`].
#[derive(Debug, Clone, PartialEq)]
pub struct QuickField {
    /// The path of its part, as in [`PartView::path`].
    pub path: Vec<usize>,
    /// The part it is of, unless it is the match's: its name and number, from 1, e.g. game 2.
    pub part: Option<(Phrase, usize)>,
    pub field: FieldView,
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

    /// What the result of the match looks like, which changes with the match format of its phase.
    pub fn template(&self) -> &Template {
        &self.template
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

    /// Enters `scores` as [`MatchRow::scores`](crate::view::MatchRow::scores) shows them: the one
    /// of the match into its first field, if it has one, or else one into the first field of each
    /// part in order, e.g. the games of a best of; a count goes into a field of decimals as its
    /// number. Returns whether the match takes them, having entered nothing if not: it does not if
    /// they are more than its parts, a score is of a part without a field, or a decimal is of a
    /// field of counts.
    pub fn enter_scores(&mut self, scores: &[Sides<Option<EnteredValue>>]) -> bool {
        let root = &self.template.root;
        // where each score goes: the path of its part and its field, none for a part without one;
        // of every part, not only those the view shows yet
        let places: Vec<Option<(Vec<usize>, &FieldSpec)>> = match root.fields.first() {
            Some(field) => vec![Some((Vec::new(), field))],
            None => (0..self.root.parts.len())
                .map(|index| {
                    let level = root.level_of_part(index).expect("an entry for each part");
                    Some((vec![index], level.fields.first()?))
                })
                .collect(),
        };
        if scores.len() > places.len() {
            return false;
        }
        let Some(entries) = places
            .into_iter()
            .zip(scores)
            .map(|(place, score)| {
                let (path, field) = place?;
                // `None` if the value does not fit the field, `Some(None)` for no value
                let fit = |value: Option<EnteredValue>| match value {
                    Some(value) => of_kind(value, field.kind).map(Some),
                    None => Some(None),
                };
                let (a, b) = (fit(score.a)?, fit(score.b)?);
                Some((path, field.key.clone(), Sides::new(a, b)))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        for (path, key, score) in entries {
            self.set_value(path.as_slice(), &key, Team::A, score.a);
            self.set_value(&path, &key, Team::B, score.b);
        }
        true
    }

    /// What is entered of the match in one row, if its result takes no more than a row: if the
    /// match has a single field and no parts, e.g. its score, or else no field of its own and parts
    /// of one level with a single field and no parts of their own, e.g. the score of each game of a
    /// best of. A match decided by its winner alone takes none, and neither does one of parts of
    /// several levels, e.g. the rubbers of a fixture.
    ///
    /// What the row enters goes into this form as [`set_value`](Self::set_value) does; a result is
    /// never chosen there, so the form is reported as it suggests.
    pub fn quick_entry(&self) -> Option<QuickEntry> {
        let level = &self.template.root;
        let mut root = self.view().root;
        let fields: Vec<QuickField> = match (&level.parts, level.fields.len()) {
            (Parts::None, 1) => std::mem::take(&mut root.fields)
                .into_iter()
                .map(|field| QuickField {
                    path: Vec::new(),
                    part: None,
                    field,
                })
                .collect(),
            (Parts::Same { level, .. }, 0)
                if level.fields.len() == 1 && matches!(level.parts, Parts::None) =>
            {
                std::mem::take(&mut root.parts)
                    .into_iter()
                    .enumerate()
                    .flat_map(|(index, part)| {
                        let name = part.name;
                        part.fields.into_iter().map(move |field| QuickField {
                            path: part.path.clone(),
                            part: Some((name.clone(), index + 1)),
                            field,
                        })
                    })
                    .collect()
            }
            _ => return None,
        };
        let mut warnings: Vec<Text> = Vec::new();
        let of_fields = fields.iter().flat_map(|quick| &quick.field.warnings);
        for warning in of_fields.chain(&root.warnings) {
            if !warnings.contains(warning) {
                warnings.push(warning.clone());
            }
        }
        Some(QuickEntry {
            match_format: plugin_factory::id_of(&*self.format),
            fields,
            result: root.result,
            warnings,
        })
    }

    /// Chooses `result` for the part at `path` by hand, or follows what its values and parts
    /// suggest again, with `None`.
    ///
    /// # Panics
    /// Panics if `path` is no path of a part of the [view](Self::view).
    pub fn choose(&mut self, path: &[usize], result: Option<MatchResult>) {
        self.root.at(path).chosen = result;
    }

    /// Whether a result was chosen by hand for any part.
    pub fn is_chosen(&self) -> bool {
        self.root.is_chosen()
    }

    /// Follows what the values and parts suggest again, for every part.
    pub fn follow_suggestions(&mut self) {
        self.root.follow_suggestions();
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
        let entered = entry
            .parts
            .iter()
            .rposition(|part| !part.is_empty())
            .map_or(0, |last| last + 1);
        children.truncate(entered);
        let mut part = Part::new();
        for (key, values) in &entry.values {
            part = part.with_value(key.clone(), *values);
        }
        let mut warnings = Vec::new();
        // whether the parts before the last one entered already decided the part
        if let Some((_, before)) = children.split_last()
            && matches!(level.parts, Parts::Same { .. })
        {
            let decided = before.iter().cloned().fold(part.clone(), Part::with_part);
            if self.format.suggest(level, &decided).is_some() {
                warnings.push(Text::new("warning-more-than-decided"));
            }
        }
        for child in children {
            part = part.with_part(child);
        }
        let suggested = self.format.suggest(level, &part);
        let result = entry.chosen.or(suggested);
        if let Parts::Same { min, .. } = level.parts {
            let mut shown = entered.max(min as usize).min(views.len());
            if result.is_none()
                && shown < views.len()
                && views[..shown].iter().all(|view| view.result.is_some())
            {
                shown += 1;
            }
            views.truncate(shown);
        }
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
            needs_choice: level.fields.is_empty() && matches!(level.parts, Parts::None),
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

/// `value` as a value of `kind`: a count as a decimal of its number, a decimal as a count only if
/// it is a whole number.
fn of_kind(value: EnteredValue, kind: ValueKind) -> Option<EnteredValue> {
    match (value, kind) {
        (EnteredValue::Count(count), ValueKind::Decimal) => {
            Some(EnteredValue::Decimal(Score::from(count)))
        }
        (EnteredValue::Decimal(decimal), ValueKind::Count) => {
            let number = decimal.get();
            (number.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&number))
                .then_some(EnteredValue::Count(number as u32))
        }
        (value, _) => Some(value),
    }
}

/// The view of `field`, with its values among `values`, warning of a score unlike one the winner
/// of the part usually has.
fn field_view(field: &FieldSpec, values: &Values) -> FieldView {
    let values = values
        .iter()
        .find(|(key, _)| *key == field.key)
        .map_or(Sides::new(None, None), |(_, values)| *values);
    let warnings = match (field.hints.winning_score, values.a, values.b) {
        (Some(winning), Some(EnteredValue::Count(a)), Some(EnteredValue::Count(b))) => {
            winning_score_warning(winning, a.max(b), a.min(b))
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    };
    FieldView {
        key: field.key.clone(),
        label: field.label.clone(),
        kind: field.kind,
        values,
        warnings,
    }
}

/// The warning of a score of `high` to `low` in a part usually won with `winning`, or more than
/// it, then by one or two, if it is no such score.
fn winning_score_warning(winning: u32, high: u32, low: u32) -> Option<Text> {
    let lead = high - low;
    if high < winning {
        Some(Text::new("warning-below-winning-score").with("winning", winning))
    } else if (high > winning && !(1..=2).contains(&lead)) || lead == 0 {
        Some(Text::new("warning-winning-lead").with("winning", winning))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
