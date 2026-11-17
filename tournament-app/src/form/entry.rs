//! The name of a phase being configured and where it takes its entrants from: the part of a
//! [`PhaseForm`] before its plugins.

use std::num::NonZeroU32;

use tournament_engine::{PhaseId, Source, SourceOf, vec1::Vec1};

use super::PhaseForm;
use crate::{error::SourcesError, i18n::Text, rejected::Rejections, view::PhaseName};

/// What a [`PhaseForm`] knows of the competition its phase is of, to offer and check sources.
#[derive(Debug, Clone)]
pub(crate) struct Context {
    /// The number the phase has, or will have once added, from 1.
    pub(crate) number: usize,
    /// The phases before it, which it can take entrants from.
    pub(crate) earlier: Vec<(PhaseId, PhaseName)>,
    /// The sources of every other phase, each with the phase it is of.
    pub(crate) taken: Vec<(Source, PhaseName)>,
    /// Whether a phase taking entrants from the registration alone can no longer start: the
    /// competition has started, and the phase did not start with it.
    pub(crate) registration_closed: bool,
}

impl Default for Context {
    /// The context of the first phase of a competition that has none.
    fn default() -> Self {
        Self {
            number: 1,
            earlier: Vec::new(),
            taken: Vec::new(),
            registration_closed: false,
        }
    }
}

/// The name and sources of a phase being configured, see [`PhaseForm::entry_view`].
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// As typed, spaces around it included.
    pub(crate) name: String,
    pub(crate) sources: Vec<Source>,
    /// What was typed for a place and not taken, by the index of its source and its end.
    pub(crate) rejected: Rejections<(usize, End)>,
    /// Whether the operator asked to choose the places of the first phase, which otherwise takes
    /// every entrant registered without asking, see [`PhaseForm::take_some_places`].
    pub(crate) some_places: bool,
}

impl Default for Entry {
    /// No name, taking every entrant registered.
    fn default() -> Self {
        Self {
            name: String::new(),
            sources: vec![Source::registration()],
            rejected: Rejections::default(),
            some_places: false,
        }
    }
}

/// An end of a range of places of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum End {
    From,
    To,
}

/// What the form shows of the name and the sources of a phase.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryView {
    /// The name as typed.
    pub name: String,
    /// The number of the phase, which with the label of its format is what it is called without a
    /// name, see [`PhaseName`].
    pub number: usize,
    /// Whether the phase is the first and takes every entrant registered, which is no choice
    /// unless the operator asks for one ([`PhaseForm::take_some_places`]): [`Self::rows`] is
    /// empty.
    pub everyone_registered: bool,
    /// One per source, in order.
    pub rows: Vec<SourceRowView>,
    /// What a source can take places of, in order: the registration, then each earlier phase.
    pub choices: Vec<SourceChoice>,
    /// What is wrong with the sources that no row in particular fixes, e.g. that there are none.
    pub issues: Vec<Text>,
}

/// A source as the form shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceRowView {
    /// The phase it takes places of, `None` for the registration.
    pub of: Option<PhaseId>,
    /// Its first place as typed, or as taken.
    pub from: String,
    /// Its last place as typed, or as taken; empty for the last place there is.
    pub to: String,
    /// Why what was typed for the first place was not taken.
    pub from_rejected: Option<Text>,
    /// Why what was typed for the last place was not taken.
    pub to_rejected: Option<Text>,
    /// What is wrong with the source, e.g. that it takes places another one takes.
    pub issue: Option<Text>,
}

/// What a source can take places of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChoice {
    /// The phase, `None` for the registration.
    pub phase: Option<PhaseId>,
    /// What the phase is called; `None` for the registration.
    pub name: Option<PhaseName>,
}

impl PhaseForm {
    /// The name typed, empty if none.
    pub fn name(&self) -> &str {
        &self.entry.name
    }

    /// Names the phase `name`; nothing but spaces for no name.
    pub fn set_name(&mut self, name: &str) {
        name.clone_into(&mut self.entry.name);
    }

    /// The sources as taken so far, in order.
    pub fn sources(&self) -> &[Source] {
        &self.entry.sources
    }

    /// Takes entrants from `sources` instead, in this order, forgetting places not taken.
    pub fn set_sources(&mut self, sources: Vec<Source>) {
        self.entry.sources = sources;
        self.entry.rejected.clear();
    }

    /// Shows the sources of the first phase as those of any other, to take only some places of
    /// the registration, e.g. to leave the best rated for a later phase.
    pub fn take_some_places(&mut self) {
        self.entry.some_places = true;
    }

    /// Adds a source after the others: of the phase before this one, or the registration if
    /// there is none, from the first place no other source of it takes, to its last.
    pub fn add_source(&mut self) {
        let of = self
            .context
            .earlier
            .last()
            .map_or(SourceOf::Registration, |(id, _)| SourceOf::Phase(*id));
        let from = self
            .context
            .taken
            .iter()
            .map(|(source, _)| source)
            .chain(&self.entry.sources)
            .filter(|source| source.of() == of)
            .filter_map(|source| source.to()?.checked_add(1))
            .max()
            .unwrap_or(NonZeroU32::MIN);
        self.entry.sources.push(Source::new(of, from, None));
    }

    /// Removes the source at `index`, if there is one.
    pub fn remove_source(&mut self, index: usize) {
        if index >= self.entry.sources.len() {
            return;
        }
        self.entry.sources.remove(index);
        let rejected = std::mem::take(&mut self.entry.rejected);
        for ((at, end), rejected) in rejected.iter() {
            let at = match at.cmp(&index) {
                std::cmp::Ordering::Less => *at,
                std::cmp::Ordering::Equal => continue,
                std::cmp::Ordering::Greater => at - 1,
            };
            self.entry
                .rejected
                .reject((at, *end), &rejected.entered, rejected.problem.clone());
        }
    }

    /// Takes the places of the source at `index` from the phase `phase` instead, or from the
    /// registration if `None`, keeping its range.
    pub fn choose_source(&mut self, index: usize, phase: Option<PhaseId>) {
        if let Some(source) = self.entry.sources.get_mut(index) {
            let of = phase.map_or(SourceOf::Registration, SourceOf::Phase);
            *source = Source::new(of, source.from(), source.to());
        }
    }

    /// Enters `entered` as the place `end` of the source at `index`: a whole number from 1, or
    /// for its last place nothing, for the last place there is. What it does not take stays
    /// shown with why, and keeps the phase from being added or saved, see [`EntryView`].
    pub fn enter_place(&mut self, index: usize, end: End, entered: &str) {
        let Some(source) = self.entry.sources.get_mut(index) else {
            return;
        };
        let trimmed = entered.trim();
        let place = match (end, trimmed) {
            (End::To, "") => Ok(None),
            (End::From, "") => Ok(Some(NonZeroU32::MIN)),
            (_, typed) => typed.parse::<NonZeroU32>().map(Some),
        };
        let Ok(place) = place else {
            self.entry
                .rejected
                .reject((index, end), entered, Text::new("error-place-not-taken"));
            return;
        };
        self.entry.rejected.forget(&(index, end));
        *source = match (end, place) {
            (End::From, place) => {
                Source::new(source.of(), place.unwrap_or(NonZeroU32::MIN), source.to())
            }
            (End::To, place) => Source::new(source.of(), source.from(), place),
        };
    }

    /// What the form shows of the name and the sources.
    pub fn entry_view(&self) -> EntryView {
        let everyone_registered = self.is_first();
        let choices = std::iter::once(SourceChoice {
            phase: None,
            name: None,
        })
        .chain(self.context.earlier.iter().map(|(id, name)| SourceChoice {
            phase: Some(*id),
            name: Some(name.clone()),
        }))
        .collect();
        let rows = if everyone_registered {
            Vec::new()
        } else {
            self.entry
                .sources
                .iter()
                .enumerate()
                .map(|(index, source)| {
                    let rejected = |end| {
                        self.entry
                            .rejected
                            .get(&(index, end))
                            .map(|rejected| (rejected.entered.clone(), rejected.problem.clone()))
                    };
                    let (from, from_rejected) = rejected(End::From)
                        .map_or((source.from().to_string(), None), |(entered, problem)| {
                            (entered, Some(problem))
                        });
                    let (to, to_rejected) = rejected(End::To).map_or(
                        (
                            source.to().map(|to| to.to_string()).unwrap_or_default(),
                            None,
                        ),
                        |(entered, problem)| (entered, Some(problem)),
                    );
                    SourceRowView {
                        of: match source.of() {
                            SourceOf::Registration => None,
                            SourceOf::Phase(id) => Some(id),
                        },
                        from,
                        to,
                        from_rejected,
                        to_rejected,
                        issue: self
                            .issue_of(index)
                            .map(|error| crate::i18n::Message::text(&error)),
                    }
                })
                .collect()
        };
        let issues = match self.sources_issue() {
            Some(error @ (SourcesError::NoSources | SourcesError::RegistrationClosed)) => {
                vec![crate::i18n::Message::text(&error)]
            }
            _ => Vec::new(),
        };
        EntryView {
            name: self.entry.name.clone(),
            number: self.context.number,
            everyone_registered,
            rows,
            choices,
            issues,
        }
    }

    /// Whether the phase is the first and takes every entrant registered, which is no choice
    /// unless the operator asked for one.
    fn is_first(&self) -> bool {
        self.context.number == 1
            && self.entry.sources == [Source::registration()]
            && !self.entry.some_places
    }

    /// What is wrong with the source at `index` alone.
    fn issue_of(&self, index: usize) -> Option<SourcesError> {
        let source = self.entry.sources.get(index)?;
        if source.is_empty() {
            return Some(SourcesError::EmptyRange);
        }
        if self.entry.sources[..index]
            .iter()
            .any(|other| source.overlaps(other))
        {
            return Some(SourcesError::Overlap(None));
        }
        self.context
            .taken
            .iter()
            .find(|(other, _)| source.overlaps(other))
            .map(|(_, phase)| SourcesError::Overlap(Some(phase.clone())))
    }

    /// The first thing wrong with the sources, if anything is.
    pub(crate) fn sources_issue(&self) -> Option<SourcesError> {
        if self.is_first() {
            return None;
        }
        if !self.entry.rejected.is_empty() {
            return Some(SourcesError::NotTaken);
        }
        if self.entry.sources.is_empty() {
            return Some(SourcesError::NoSources);
        }
        if let Some(issue) = (0..self.entry.sources.len()).find_map(|index| self.issue_of(index)) {
            return Some(issue);
        }
        let registration_alone = self
            .entry
            .sources
            .iter()
            .all(|source| source.of() == SourceOf::Registration);
        (self.context.registration_closed && registration_alone)
            .then_some(SourcesError::RegistrationClosed)
    }

    /// The name and the sources the phase is added or saved with.
    ///
    /// # Errors
    /// Returns what is wrong with the sources, as [`EntryView`] shows it.
    pub fn entry(&self) -> Result<(Option<String>, Vec1<Source>), SourcesError> {
        if let Some(issue) = self.sources_issue() {
            return Err(issue);
        }
        let name = Some(self.entry.name.trim())
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        let sources =
            Vec1::try_from_vec(self.entry.sources.clone()).map_err(|_| SourcesError::NoSources)?;
        Ok((name, sources))
    }
}
