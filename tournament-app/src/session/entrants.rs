//! Adding, removing and renaming entrants.

use std::collections::HashMap;

use tournament_engine::{AddEntrant, Entrant, EntrantId, RemoveEntrant, uuid::Uuid};
use unicode_normalization::UnicodeNormalization as _;

use super::{Session, refused};
use crate::{
    error::{AddEntrantError, Refusal, RemoveEntrantError, RenameEntrantError},
    people::Person,
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
        self.check_names(std::iter::once(name))?;
        self.add_named_entrant(name)
    }

    /// Adds all of `names` or none if any name is empty, duplicated, or entrants cannot be added.
    /// Names are kept as supplied; whitespace-only names are empty. Each entrant is a separate
    /// undoable action, as when calling [`Self::add_entrant`] for each one.
    ///
    /// # Errors
    /// Returns an error if a name is empty or looks like an existing or earlier name,
    /// see [`Self::entrant_like`], or the competition has started.
    pub fn add_named_entrants(
        &mut self,
        names: &[impl AsRef<str>],
    ) -> Result<Vec<EntrantId>, AddEntrantError> {
        self.check_names(names.iter().map(AsRef::as_ref))?;
        names
            .iter()
            .map(|name| self.add_named_entrant(name.as_ref()))
            .collect()
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

    // All names in a batch are checked first. The engine accepts adding its first entrant
    // exactly when it accepts adding every subsequent one, so a refusal adds none of them.
    fn add_named_entrant(&mut self, name: &str) -> Result<EntrantId, AddEntrantError> {
        let id = EntrantId::new(Uuid::new_v4());
        self.history
            .apply_new(&mut self.machine, AddEntrant::new(Entrant::new(id)).into())
            .map_err(|error| {
                refused(error, AddEntrantError::Rejected, |refusal| {
                    (refusal == Refusal::Started).then_some(AddEntrantError::Started)
                })
            })?;
        self.people.add(Person::new(id, name));
        self.people_changes += 1;
        Ok(id)
    }

    /// Adds an entrant for each line of `lines` that is not blank, e.g. of a list pasted, as
    /// [`Self::add_entrant`] adds one: all of them, in order, or none if any is refused. Each is
    /// an action of its own.
    ///
    /// # Errors
    /// Returns an error if every line is blank, a name looks like another entrant's or that of an
    /// earlier line, see [`Self::entrant_like`], or the competition has started.
    pub fn add_entrants(&mut self, lines: &str) -> Result<Vec<EntrantId>, AddEntrantError> {
        let names: Vec<&str> = lines
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect();
        if names.is_empty() {
            return Err(AddEntrantError::EmptyName);
        }
        self.add_named_entrants(&names)
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
            self.people.add(Person::new(id, name));
            self.people_changes += 1;
        }
        Ok(())
    }
}

/// `name` as it is compared to tell names that look alike, see [`Session::entrant_like`].
fn folded(name: &str) -> String {
    name.trim().nfkc().collect::<String>().to_lowercase()
}
