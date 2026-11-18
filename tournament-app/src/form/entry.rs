//! The name of a phase being configured and where it takes its entrants from: the part of a
//! [`PhaseForm`] before its plugins.

use std::num::NonZeroU32;

use plugin_factory::config::PluginConfig;
use tournament_engine::{
    PhaseId, PhasePlugins, Source, SourceOf,
    plugins::teams::{Keep, Teams},
    vec1::Vec1,
};

use super::PhaseForm;
use crate::{
    error::SourcesError,
    i18n::{Arg, Text},
    rejected::Rejections,
    view::PhaseName,
};

/// What a [`PhaseForm`] knows of the competition its phase is of, to offer and check sources.
#[derive(Debug, Clone)]
pub(crate) struct Context {
    /// The number the phase has, or will have once added, from 1.
    pub(crate) number: usize,
    /// The phases before it, which it can take entrants from.
    pub(crate) earlier: Vec<(PhaseId, PhaseName)>,
    /// How many entrants each ranking of a phase before it stands for, of those where its sources
    /// agree, see [`Competition::entrants_per_ranking`].
    ///
    /// [`Competition::entrants_per_ranking`]: tournament_engine::Competition::entrants_per_ranking
    pub(crate) entrants_per_ranking: Vec<(PhaseId, NonZeroU32)>,
    /// The sources of every other phase, each with the phase it is of.
    pub(crate) taken: Vec<(Source, PhaseName)>,
    /// Whether a phase taking entrants from the registration alone can no longer start: the
    /// competition has started, and the phase did not start with it.
    pub(crate) registration_closed: bool,
    /// The groups of each phase before it played in groups side by side.
    pub(crate) groups: Vec<(PhaseId, Groups)>,
}

/// The groups side by side a phase is played in, whose places are those of the phase in turn: the
/// firsts of all groups, then the seconds, and so on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Groups {
    /// How many there are, at least 2.
    pub(crate) count: usize,
    /// How many places each has, once the phase has started.
    pub(crate) sizes: Option<Vec<usize>>,
}

impl Groups {
    /// Where the places from `from` to `to` (to the last place if `None`) lie in the groups, e.g.
    /// "places 1–6 are places 1–2 of each of the 3 groups"; `None` for every place.
    pub(crate) fn places(&self, from: NonZeroU32, to: Option<NonZeroU32>) -> Option<Text> {
        if from.get() == 1 && to.is_none() {
            return None;
        }
        let (from, groups) = (from.get(), self.count);
        let to = to.map(NonZeroU32::get).or_else(|| {
            self.sizes
                .as_ref()
                .map(|sizes| u32::try_from(sizes.iter().sum::<usize>()).unwrap_or(u32::MAX))
        });
        let mut parts = Vec::new();
        // whether a part takes only some of one place, compared across the groups
        let mut compared = false;
        // the ranks in a group all of whose places are taken, from and to
        let mut whole: Option<(u32, u32)> = None;
        let flush = |whole: &mut Option<(u32, u32)>, parts: &mut Vec<Text>| {
            if let Some((first, last)) = whole.take() {
                parts.push(if first == last {
                    Text::new("group-place").with("rank", first)
                } else {
                    Text::new("group-places")
                        .with("first", first)
                        .with("last", last)
                });
            }
        };
        // the first place of the rank `rank` in the phase
        let (mut start, mut rank) = (1_u32, 1_u32);
        loop {
            let block = match &self.sizes {
                Some(sizes) => sizes.iter().filter(|&&size| size >= rank as usize).count(),
                None => groups,
            };
            let block = u32::try_from(block).unwrap_or(u32::MAX);
            if block == 0 || to.is_some_and(|to| start > to) {
                break;
            }
            let end = start + block - 1;
            if end >= from {
                let first = from.max(start) - start + 1;
                if to.is_none() && first == 1 {
                    // every rank from here on, of groups of sizes not known yet
                    flush(&mut whole, &mut parts);
                    parts.push(Text::new("group-places-on").with("rank", rank));
                    break;
                }
                let last = to.map_or(block, |to| to.min(end) - start + 1);
                if first == 1 && last == block {
                    whole = Some((whole.map_or(rank, |(first, _)| first), rank));
                } else {
                    flush(&mut whole, &mut parts);
                    compared = true;
                    parts.push(
                        if first == 1 {
                            Text::new("group-places-best").with("count", last)
                        } else if last == block {
                            Text::new("group-places-worst").with("count", block - first + 1)
                        } else if first == last {
                            Text::new("group-place-between").with("place", first)
                        } else {
                            Text::new("group-places-between")
                                .with("first", first)
                                .with("last", last)
                        }
                        .with("rank", rank),
                    );
                }
            }
            start = end + 1;
            rank += 1;
        }
        flush(&mut whole, &mut parts);
        let text = match to {
            Some(to) => Text::new("places-in-groups")
                .with("count", to.saturating_sub(from) + 1)
                .with("from", from)
                .with("to", to),
            None => Text::new("places-on-in-groups").with("from", from),
        };
        Some(
            text.with("groups", groups)
                .with("parts", Arg::List(parts))
                .with("compared", if compared { "yes" } else { "no" }),
        )
    }
}

impl Default for Context {
    /// The context of the first phase of a competition that has none.
    fn default() -> Self {
        Self {
            number: 1,
            earlier: Vec::new(),
            entrants_per_ranking: Vec::new(),
            taken: Vec::new(),
            registration_closed: false,
            groups: Vec::new(),
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
    /// Where the places it takes lie in the groups of its phase, if that is played in groups side
    /// by side, e.g. "places 1–6 are places 1–2 of each of the 3 groups".
    pub in_groups: Option<Text>,
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
        self.redraft();
    }

    /// How many entrants each qualifier of the field stands for, as the sources stand: one for the
    /// registration, for a phase as many as each of its rankings; `None` if the sources differ, or
    /// there are none.
    pub(crate) fn entrants_per_qualifier(&self) -> Option<NonZeroU32> {
        let mut sizes = self.entry.sources.iter().map(|source| match source.of() {
            SourceOf::Registration => Some(NonZeroU32::MIN),
            SourceOf::Phase(phase) => self
                .context
                .entrants_per_ranking
                .iter()
                .find(|(of, _)| *of == phase)
                .map(|(_, size)| *size),
        });
        let first = sizes.next()??;
        sizes.all(|size| size == Some(first)).then_some(first)
    }

    /// How many entrants each team of the phase stands for, as the sources and its teams stand
    /// (as registered while none are chosen): `None` if they would not all be of one size, see
    /// [`Teams::entrants_per_team`].
    fn teams_of_one_size(&self) -> Option<NonZeroU32> {
        let teams = self
            .draft
            .answers()
            .get(PhasePlugins::TEAMS)
            .and_then(PluginConfig::from_answer)
            .and_then(|config| config.build::<dyn Teams>().ok())
            .unwrap_or_else(|| Box::new(Keep));
        teams.entrants_per_team(self.entrants_per_qualifier())
    }

    /// Makes the draft anew for the sources as they stand, with its answers, so that what is
    /// compatible follows the sources, see [`PhasePlugins::draft`].
    pub(super) fn redraft(&mut self) {
        self.draft = PhasePlugins::draft(self.entrants_per_qualifier())
            .with_answers(self.draft.answers())
            .expect("a phase's draft takes the answers of another one");
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
        self.redraft();
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
        self.redraft();
    }

    /// Takes the places of the source at `index` from the phase `phase` instead, or from the
    /// registration if `None`, keeping its range.
    pub fn choose_source(&mut self, index: usize, phase: Option<PhaseId>) {
        if let Some(source) = self.entry.sources.get_mut(index) {
            let of = phase.map_or(SourceOf::Registration, SourceOf::Phase);
            *source = Source::new(of, source.from(), source.to());
        }
        self.redraft();
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
                        in_groups: self
                            .context
                            .groups
                            .iter()
                            .find(|(phase, _)| source.of() == SourceOf::Phase(*phase))
                            .filter(|_| !source.is_empty())
                            .and_then(|(_, groups)| groups.places(source.from(), source.to())),
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
        if self.teams_of_one_size().is_none() {
            return Some(SourcesError::MixedTeamSizes);
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

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// What the hint says of the places `from` to `to` of `groups`, in English.
    fn places(groups: &Groups, from: u32, to: Option<u32>) -> Option<String> {
        let place = |place| NonZeroU32::new(place).unwrap();
        groups
            .places(place(from), to.map(place))
            .map(|text| crate::i18n::Translator::plain_english().text(&text))
    }

    const THREE: Groups = Groups {
        count: 3,
        sizes: None,
    };

    #[test]
    fn whole_places_of_each_group() {
        assert_eq!(
            places(&THREE, 1, Some(6)).unwrap(),
            "with 3 groups side by side, places 1–6 are places 1–2 of each group"
        );
        assert_eq!(
            places(&THREE, 4, Some(6)).unwrap(),
            "with 3 groups side by side, places 4–6 are place 2 of each group"
        );
        assert_eq!(
            places(&THREE, 7, None).unwrap(),
            "with 3 groups side by side, the places from 7 on are the places from 3 on of each \
             group"
        );
    }

    #[test]
    fn places_cutting_through_a_place_of_the_groups_are_compared() {
        assert_eq!(
            places(&THREE, 1, Some(8)).unwrap(),
            "with 3 groups side by side, places 1–8 are places 1–2 of each group and the best 2 \
             of those in place 3 of their group, those of one place compared across the groups"
        );
        assert_eq!(
            places(&THREE, 9, Some(10)).unwrap(),
            "with 3 groups side by side, places 9–10 are the worst of those in place 3 of their \
             group and the best of those in place 4 of their group, those of one place compared \
             across the groups"
        );
        assert_eq!(
            places(&THREE, 2, Some(2)).unwrap(),
            "with 3 groups side by side, place 2 is place 2 of those in place 1 of their group, \
             those of one place compared across the groups"
        );
    }

    /// Once the groups are drawn, a place only some groups have is shared by fewer.
    #[test]
    fn places_of_groups_of_different_sizes() {
        let groups = Groups {
            count: 3,
            sizes: Some(vec![4, 4, 3]),
        };
        assert_eq!(
            places(&groups, 10, None).unwrap(),
            "with 3 groups side by side, places 10–11 are place 4 of each group"
        );
        assert_eq!(places(&groups, 1, None), None);
        assert_eq!(places(&THREE, 1, None), None);
    }
}
