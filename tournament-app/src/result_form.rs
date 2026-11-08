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
//! A score at least two below the one its winner usually has completes the other side's, once a
//! screen asks ([`ResultForm::complete_score`]).
//!
//! A finished match is shown as it was reported by the view of a form of it
//! ([`Session::reported_result`](crate::session::Session::reported_result)).
//!
//! A result chosen by hand overrides the one suggested, which is rare: a screen may offer the
//! choice only on request, except for a part that only a choice decides
//! ([`PartView::needs_choice`]).
//!
//! Nothing entered is refused for breaking the rules of a sport: the form only warns of a score
//! unlike those the match format hints a winner usually has, of a result chosen against the one the
//! values suggest, and of parts entered after those before them decided the match. What is typed as
//! no value its field takes at all, e.g. a count above 65535, stays shown with why, and keeps the
//! match from being reported until it is corrected ([`ResultForm::enter`]).

use plugin_factory::schema::{DecimalRange, IntegerRange, NumberRange};
use tournament_engine::{
    Decision, EnteredValue, FieldKey, MatchId, MatchResult, Part, ResultReason, Score, Sides, Team,
    Values,
    plugins::{
        Phrase,
        match_format::{FieldSpec, Level, MatchFormat, Parts, Template, ValueKind},
    },
};

use crate::{
    form::{NotTaken, typed_number, with_range},
    i18n::Text,
    view,
};

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
    /// What was typed for a side of a field and not taken, which leaves the side without a value.
    rejected: Vec<Rejected>,
    /// One for each part the level can have.
    parts: Vec<Entry>,
}

/// What was typed for `team` into the field `field` of a part and not taken, with why, see
/// [`ResultForm::enter`].
#[derive(Debug, Clone)]
struct Rejected {
    field: FieldKey,
    team: Team,
    entered: String,
    problem: Text,
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

    /// The entry of `part`, reported at `level` and read by `format`: its values, reason and
    /// parts, and its result chosen where the values and parts do not suggest it.
    fn reported(format: &dyn MatchFormat, level: &Level, part: &Part) -> Self {
        let mut entry = Self::empty(level);
        for (index, reported) in part.parts().iter().enumerate() {
            let level = level
                .level_of_part(index)
                .expect("a reported part fits the template");
            entry.parts[index] = Self::reported(format, level, reported);
        }
        entry.values = part.values().clone();
        if let Some(decision) = part.decision() {
            entry.reason = decision.reason().cloned();
            let result = decision.result();
            entry.chosen = (format.suggest(level, part) != Some(result)).then_some(result);
        }
        entry
    }

    fn is_empty(&self) -> bool {
        self.chosen.is_none()
            && self.reason.is_none()
            && self.values.is_empty()
            && self.rejected.is_empty()
            && self.parts.iter().all(Self::is_empty)
    }

    fn has_rejected(&self) -> bool {
        !self.rejected.is_empty() || self.parts.iter().any(Self::has_rejected)
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
    /// The parts each side won, if it is played in parts at one level, e.g. 2–1 in games of a
    /// best of; none while no part is decided.
    pub parts_won: Option<Sides<u32>>,
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
    /// What was typed for each side and not taken, see [`ResultForm::enter`], which the field
    /// shows instead of the side's value, `None` then.
    pub entered: Sides<Option<String>>,
    /// Why what was typed is not taken, said once where both sides are alike.
    pub issues: Vec<Text>,
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
    /// The parts each side won, as in [`PartView::parts_won`].
    pub parts_won: Option<Sides<u32>>,
    /// Why what was typed into any field is not taken, each said once, see [`FieldView::issues`].
    pub issues: Vec<Text>,
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

    /// The form of the match `match_id` as it was reported, `contest`, whose result looks like
    /// `template`, entered by `format`: the result of a part is chosen where its values and parts
    /// do not suggest it.
    pub(crate) fn reported(
        match_id: MatchId,
        format: Box<dyn MatchFormat>,
        template: Template,
        contest: &Part,
    ) -> Self {
        let root = Entry::reported(&*format, &template.root, contest);
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

    /// Enters `value` for `team` into the field `field` of the part at `path`, or clears it, in
    /// place of what was typed there and not taken.
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
        entry
            .rejected
            .retain(|rejected| rejected.field != *field || rejected.team != team);
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

    /// Enters `entered`, as an operator typed it, for `team` into the field `field` of the part at
    /// `path`: nothing clears the value, as [`set_value`](Self::set_value) does, and a number the
    /// field takes is entered as its value, with a point or a comma before its decimals, possibly
    /// with an exponent (`1e3`), a count as a whole number from 0 to 65535. Anything else leaves
    /// the side without a value, and the field shows what was typed, with why it is not taken
    /// ([`FieldView::issues`]), until the side is entered again; meanwhile no score is completed
    /// onto it, and the match is not reported.
    ///
    /// # Panics
    /// Panics if `path` is no path of a part of the [view](Self::view), or the part has no field
    /// `field`.
    pub fn enter(&mut self, path: &[usize], field: &FieldKey, team: Team, entered: &str) {
        let kind = self
            .level(path)
            .field(field)
            .expect("a field of the part")
            .kind;
        match read(entered, kind) {
            Ok(value) => self.set_value(path, field, team, value),
            Err(problem) => {
                self.set_value(path, field, team, None);
                self.root.at(path).rejected.push(Rejected {
                    field: field.clone(),
                    team,
                    entered: entered.to_owned(),
                    problem,
                });
            }
        }
    }

    /// Completes the score of a part won as usual: if `team` has a count in the field `field` of
    /// the part at `path` at least two below the score its winner usually has
    /// ([`Hints::winning_score`](tournament_engine::plugins::match_format::Hints::winning_score)),
    /// and nothing is entered for the other side, not even what was typed and not taken, the other
    /// side is entered as winning with that score, the only one it can win with by the usual lead,
    /// e.g. 21 for 19 or less in a game to 21. Returns whether it entered it.
    ///
    /// # Panics
    /// Panics if `path` is no path of a part of the [view](Self::view).
    pub fn complete_score(&mut self, path: &[usize], field: &FieldKey, team: Team) -> bool {
        let Some(winning) = self
            .level(path)
            .field(field)
            .and_then(|spec| spec.hints.winning_score)
        else {
            return false;
        };
        let entry = self.root.at(path);
        let values = entry
            .values
            .iter()
            .find(|(key, _)| key == field)
            .map_or(Sides::new(None, None), |(_, values)| *values);
        let (entered, other) = (*values.of(team), *values.of(team.opponent()));
        let other_typed = entry
            .rejected
            .iter()
            .any(|rejected| rejected.field == *field && rejected.team == team.opponent());
        // from one below the winning score on, the other side may win by more or lose
        let completes = match entered {
            Some(EnteredValue::Count(count)) => {
                winning.checked_sub(count).is_some_and(|gap| gap >= 2)
            }
            _ => false,
        };
        if !completes || other.is_some() || other_typed {
            return false;
        }
        self.set_value(
            path,
            field,
            team.opponent(),
            Some(EnteredValue::Count(winning)),
        );
        true
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
    /// What the row enters goes into this form as [`enter`](Self::enter) or
    /// [`set_value`](Self::set_value) do; a result is never chosen there, so the form is reported
    /// as it suggests.
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
        let issues = said_once(fields.iter().flat_map(|quick| &quick.field.issues));
        let warnings = said_once(
            fields
                .iter()
                .flat_map(|quick| &quick.field.warnings)
                .chain(&root.warnings),
        );
        Some(QuickEntry {
            match_format: plugin_factory::id_of(&*self.format),
            fields,
            result: root.result,
            parts_won: root.parts_won,
            issues,
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

    /// Whether anything typed into the form was not taken, see [`Self::enter`].
    pub(crate) fn has_rejected(&self) -> bool {
        self.root.has_rejected()
    }

    /// The level of the part at `path`.
    ///
    /// # Panics
    /// Panics if `path` is no path of a part of the [view](Self::view).
    fn level(&self, path: &[usize]) -> &Level {
        path.iter().fold(&self.template.root, |level, &index| {
            level
                .level_of_part(index)
                .expect("a path of a part of the view")
        })
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
                warnings.push(match view::parts_won_of(level, &decided) {
                    Some(won) => Text::new("warning-more-than-decided-at")
                        .with("a", won.a)
                        .with("b", won.b),
                    None => Text::new("warning-more-than-decided"),
                });
            }
        }
        for child in children {
            part = part.with_part(child);
        }
        let suggested = self.format.suggest(level, &part);
        let parts_won = view::parts_won_of(level, &part);
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
                .map(|field| field_view(field, &entry.values, &entry.rejected))
                .collect(),
            result,
            suggested,
            chosen: entry.chosen.is_some(),
            parts_won,
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
            Some(EnteredValue::Decimal(Score::from(u32::from(count))))
        }
        (EnteredValue::Decimal(decimal), ValueKind::Count) => {
            let number = decimal.get();
            (number.fract() == 0.0 && (0.0..=f64::from(u16::MAX)).contains(&number))
                .then_some(EnteredValue::Count(number as u16))
        }
        (value, _) => Some(value),
    }
}

/// What `entered`, as an operator typed it, is in a field of `kind`, see [`ResultForm::enter`]:
/// `None` if nothing, else its value, or why it is none.
fn read(entered: &str, kind: ValueKind) -> Result<Option<EnteredValue>, Text> {
    if entered.trim().is_empty() {
        return Ok(None);
    }
    let out_of_range = || with_range(Text::new("issue-out-of-range"), range_of(kind));
    match typed_number(entered) {
        Ok(number) => of_kind(EnteredValue::Decimal(Score::new(number)), kind)
            .map(Some)
            .ok_or_else(out_of_range),
        Err(NotTaken::OutOfRange) => Err(out_of_range()),
        Err(NotTaken::NoNumber) => {
            Err(Text::new("issue-not-a-number").with("entered", entered.trim()))
        }
    }
}

/// The numbers a field of `kind` takes.
fn range_of(kind: ValueKind) -> NumberRange {
    match kind {
        ValueKind::Count => NumberRange::Integer(IntegerRange::new(0, u16::MAX.into())),
        ValueKind::Decimal => NumberRange::Decimal(DecimalRange::ALL),
    }
}

/// Each of `texts` once, in the order they first come in.
fn said_once<'a>(texts: impl IntoIterator<Item = &'a Text>) -> Vec<Text> {
    let mut once: Vec<Text> = Vec::new();
    for text in texts {
        if !once.contains(text) {
            once.push(text.clone());
        }
    }
    once
}

/// The view of `field`, with its values among `values` and what was typed for it and not taken
/// among `rejected`, warning of a score unlike one the winner of the part usually has.
fn field_view(field: &FieldSpec, values: &Values, rejected: &[Rejected]) -> FieldView {
    let values = values
        .iter()
        .find(|(key, _)| *key == field.key)
        .map_or(Sides::new(None, None), |(_, values)| *values);
    let rejected_of = |team: Team| {
        rejected
            .iter()
            .find(|rejected| rejected.field == field.key && rejected.team == team)
    };
    let typed = Sides::new(rejected_of(Team::A), rejected_of(Team::B));
    let entered = Sides::new(
        typed.a.map(|rejected| rejected.entered.clone()),
        typed.b.map(|rejected| rejected.entered.clone()),
    );
    let issues = said_once(
        [typed.a, typed.b]
            .into_iter()
            .flatten()
            .map(|rejected| &rejected.problem),
    );
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
        entered,
        issues,
        warnings,
    }
}

/// The warning of a score of `high` to `low` in a part usually won with `winning`, or more than
/// it, then by one or two, if it is no such score.
fn winning_score_warning(winning: u16, high: u16, low: u16) -> Option<Text> {
    let lead = high - low;
    if high < winning {
        Some(Text::new("warning-below-winning-score").with("winning", u32::from(winning)))
    } else if (high > winning && !(1..=2).contains(&lead)) || lead == 0 {
        Some(Text::new("warning-winning-lead").with("winning", u32::from(winning)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
