//! Adding, removing, renaming and rating entrants.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap, btree_map::Entry},
};

use tournament_engine::{
    AddEntrant, Entrant, EntrantId, RemoveEntrant,
    undoable_state_machine::{LogEntry, LogEntryKind, TransitionId},
    uuid::Uuid,
    vec1::{Vec1, vec1},
};
use unicode_normalization::UnicodeNormalization as _;

use super::{Session, refused};
use crate::{
    error::{AddEntrantError, RateEntrantError, Refusal, RemoveEntrantError, RenameEntrantError},
    people::{Person, Rating},
    view::EntrantRow,
};

impl Session {
    /// The entrants, in the order they were added.
    pub fn entrants(&self) -> Vec<EntrantRow> {
        self.competition()
            .entrants()
            .keys()
            .map(|id| self.entrant_row(id))
            .collect()
    }

    /// The name of an entrant that `name` would look like, e.g. `Alice` for `alice` or for an
    /// `Alice` whose characters are composed otherwise: equal ignoring case and the way Unicode
    /// composes them (NFKC), and leading and trailing whitespace. [`Self::add_entrant`] refuses
    /// such a name, which a screen can say while it is typed.
    pub fn entrant_like(&self, name: &str) -> Option<String> {
        self.entrant_like_besides(name, None)
    }

    /// As [`Self::entrant_like`], but never `besides`, e.g. the entrant being renamed.
    fn entrant_like_besides(&self, name: &str, besides: Option<EntrantId>) -> Option<String> {
        let name = folded(name);
        if name.is_empty() {
            return None;
        }
        self.competition()
            .entrants()
            .keys()
            .filter(|id| Some(**id) != besides)
            .filter_map(|id| self.people.name(id))
            .find(|entered| folded(entered) == name)
            .map(str::to_owned)
    }

    /// Adds an entrant shown by `name`, with leading and trailing whitespace removed.
    ///
    /// # Errors
    /// Returns an error if `name` is empty or looks like another entrant's, see
    /// [`Self::entrant_like`], or if the competition has started.
    pub fn add_entrant(&mut self, name: &str) -> Result<EntrantId, AddEntrantError> {
        let name = name.trim();
        let [id] = self.add_named_entrants(&[name])?[..] else {
            unreachable!("one entrant is added for one name")
        };
        Ok(id)
    }

    /// Adds all of `names` or none if any name is empty, duplicated, or entrants cannot be added.
    /// Names are kept as supplied; whitespace-only names are empty. All of them are one undoable
    /// action, [`Action::AddEntrants`](crate::view::Action::AddEntrants) (or
    /// [`Action::AddEntrant`](crate::view::Action::AddEntrant) for one name).
    ///
    /// # Errors
    /// Returns an error if a name is empty or looks like an existing or earlier name,
    /// see [`Self::entrant_like`], or the competition has started.
    pub fn add_named_entrants(
        &mut self,
        names: &[impl AsRef<str>],
    ) -> Result<Vec<EntrantId>, AddEntrantError> {
        self.check_names(names.iter().map(AsRef::as_ref))?;
        let ids: Vec<_> = names
            .iter()
            .map(|_| EntrantId::new(Uuid::new_v4()))
            .collect();
        let entries = ids.iter().map(|id| {
            LogEntry::new(
                TransitionId::new(),
                LogEntryKind::Apply(AddEntrant::new(Entrant::new(*id)).into()),
            )
        });
        // All names are checked first. The engine accepts adding the first entrant exactly when it
        // accepts adding every subsequent one, so a refusal comes at the first, before anything is
        // appended to the log.
        self.history
            .apply_all(&mut self.machine, entries)
            .map_err(|error| {
                refused(error, AddEntrantError::Rejected, |refusal| {
                    (refusal == Refusal::Started).then_some(AddEntrantError::Started)
                })
            })?;
        for (id, name) in ids.iter().zip(names) {
            self.people.add(Person::new(*id, name.as_ref()));
        }
        self.people_changes += names.len();
        Ok(ids)
    }

    fn check_names<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), AddEntrantError> {
        let mut taken: HashMap<_, _> = self
            .competition()
            .entrants()
            .keys()
            .filter_map(|id| self.people.name(id))
            .map(|name| (folded(name), name))
            .collect();
        for name in names {
            if name.trim().is_empty() {
                return Err(AddEntrantError::EmptyName);
            }
            if let Some(entered) = taken.insert(folded(name), name) {
                return Err(AddEntrantError::DuplicateName(entered.to_owned()));
            }
        }
        Ok(())
    }

    /// Adds an entrant for each line of `lines` that is not blank, e.g. of a list pasted, as
    /// [`Self::add_entrant`] adds one: all of them, in order, or none if any is refused, as one
    /// action. A line of a name, a tab or a `;`, and a number (see
    /// [`Rating::parse`]) rates the entrant by that number, e.g. `Alice;1500`; nothing after the
    /// tab or `;` gives no rating.
    ///
    /// # Errors
    /// Returns an error if every line is blank, a name looks like another entrant's or that of an
    /// earlier line, see [`Self::entrant_like`], a rating is no number, or the competition has
    /// started.
    pub fn add_entrants(&mut self, lines: &str) -> Result<Vec<EntrantId>, AddEntrantError> {
        let entrants = lines
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let Some((name, rating)) = line.rsplit_once(['\t', ';']) else {
                    return Ok((line.trim(), None));
                };
                let rating = rating.trim();
                if rating.is_empty() {
                    return Ok((name.trim(), None));
                }
                Rating::parse(rating)
                    .map(|rating| (name.trim(), Some(rating)))
                    .ok_or_else(|| AddEntrantError::NoRating(rating.to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if entrants.is_empty() {
            return Err(AddEntrantError::EmptyName);
        }
        let names: Vec<&str> = entrants.iter().map(|(name, _)| *name).collect();
        let ids = self.add_named_entrants(&names)?;
        for (id, (_, rating)) in ids.iter().zip(&entrants) {
            if rating.is_some() {
                self.people.set_rating(id, *rating);
            }
        }
        Ok(ids)
    }

    /// Removes the entrant `id`. Its name is kept, in case the removal is undone.
    ///
    /// # Errors
    /// Returns an error if there is no such entrant, or the competition has started.
    pub fn remove_entrant(&mut self, id: EntrantId) -> Result<(), RemoveEntrantError> {
        self.history
            .apply_new(&mut self.machine, RemoveEntrant::new(id).into())
            .map_err(|error| {
                refused(
                    error,
                    RemoveEntrantError::Rejected,
                    |refusal| match refusal {
                        Refusal::Started => Some(RemoveEntrantError::Started),
                        Refusal::UnknownEntrant => Some(RemoveEntrantError::UnknownEntrant),
                        _ => None,
                    },
                )
            })?;
        Ok(())
    }

    /// Shows the entrant `id` by `name` from now on, with leading and trailing whitespace removed,
    /// also after the start. Only its name changes, which is no entry of the log, so this is no
    /// action that [`Self::undo`] undoes; the actions done before name the entrant as it was then.
    ///
    /// # Errors
    /// Returns an error if there is no such entrant, or if `name` is empty or looks like another
    /// entrant's, see [`Self::entrant_like`].
    pub fn rename_entrant(&mut self, id: EntrantId, name: &str) -> Result<(), RenameEntrantError> {
        if !self.competition().entrants().contains_key(&id) {
            return Err(RenameEntrantError::UnknownEntrant);
        }
        let name = name.trim();
        if name.is_empty() {
            return Err(RenameEntrantError::EmptyName);
        }
        if let Some(entered) = self.entrant_like_besides(name, Some(id)) {
            return Err(RenameEntrantError::DuplicateName(entered));
        }
        if self.people.name(&id) != Some(name) {
            let rating = self.people.rating(&id);
            self.people.add(Person::new(id, name).with_rating(rating));
            self.people_changes += 1;
        }
        Ok(())
    }

    /// Rates the entrant `id` `rating`, higher is better, or unrates it if `None`: the start
    /// orders the registration by rating, see [`Self::start`]. Only the people change, which is
    /// no entry of the log, so this is no action that [`Self::undo`] undoes.
    ///
    /// # Errors
    /// Returns an error if there is no such entrant, or the competition has started.
    pub fn set_rating(
        &mut self,
        id: EntrantId,
        rating: Option<Rating>,
    ) -> Result<(), RateEntrantError> {
        if !self.competition().entrants().contains_key(&id) {
            return Err(RateEntrantError::UnknownEntrant);
        }
        if self.competition().is_started() {
            return Err(RateEntrantError::RegistrationClosed);
        }
        if self.people.rating(&id) != rating {
            if !self.people.set_rating(&id, rating) {
                return Err(RateEntrantError::UnknownEntrant);
            }
            self.people_changes += 1;
        }
        Ok(())
    }

    /// Rates the entrant `id` by `entered`, a rating as typed (see [`Rating::parse`]), or unrates
    /// it if `entered` is blank.
    ///
    /// # Errors
    /// Returns an error if `entered` is no rating, as [`Self::set_rating`] does otherwise.
    pub fn enter_rating(&mut self, id: EntrantId, entered: &str) -> Result<(), RateEntrantError> {
        let rating = if entered.trim().is_empty() {
            None
        } else {
            Some(
                Rating::parse(entered)
                    .ok_or_else(|| RateEntrantError::NoRating(entered.trim().to_owned()))?,
            )
        };
        self.set_rating(id, rating)
    }

    /// The order the start gives the registration: the rated entrants by rating, best first,
    /// those of equal ratings a tier, see
    /// [`StartCompetition`](tournament_engine::StartCompetition). Empty if nobody is rated.
    pub(super) fn order_by_rating(&self) -> Vec<Vec1<EntrantId>> {
        let mut tiers: BTreeMap<Reverse<Rating>, Vec1<EntrantId>> = BTreeMap::new();
        for id in self.competition().entrants().keys() {
            let Some(rating) = self.people.rating(id) else {
                continue;
            };
            match tiers.entry(Reverse(rating)) {
                Entry::Vacant(tier) => {
                    tier.insert(vec1![*id]);
                }
                Entry::Occupied(mut tier) => tier.get_mut().push(*id),
            }
        }
        tiers.into_values().collect()
    }

    /// The place of the entrant `id` among the rated entrants by rating, from 1, entrants of
    /// equal ratings sharing it; `None` if it is not rated.
    pub(super) fn seed(&self, id: &EntrantId) -> Option<u32> {
        let rating = self.people.rating(id)?;
        let better = self
            .competition()
            .entrants()
            .keys()
            .filter(|other| self.people.rating(other) > Some(rating))
            .count();
        Some(better as u32 + 1)
    }
}

/// `name` as it is compared to tell names that look alike, see [`Session::entrant_like`].
fn folded(name: &str) -> String {
    name.trim().nfkc().collect::<String>().to_lowercase()
}
