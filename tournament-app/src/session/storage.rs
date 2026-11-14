//! Loading, saving, catching up with, importing and exporting a session's competition.

use std::fmt::Display;

use tournament_engine::{
    Competition, CompetitionStateMachine, CompetitionTransition, undoable_state_machine::LogEntry,
    uuid::Uuid,
};

use super::{Session, UNNAMED};
use crate::{
    error::{ImportError, LoadError},
    file::{self, File},
    people::People,
    storage::{CompetitionKey, Listing, Save, Storage, Stored},
};

impl Session {
    /// Opens the competition stored in `storage` under `key`. The session has done nothing yet, so
    /// there is nothing to undo. If a version that did not store the competition the log replays
    /// into saved it last, that is unsaved.
    ///
    /// # Errors
    /// Returns an error if nothing is stored under `key`, the storage cannot be read, or what is
    /// stored is no log that replays into a competition.
    pub async fn load<S: Storage>(
        storage: &S,
        key: CompetitionKey,
    ) -> Result<Self, LoadError<S::Error>> {
        let stored = read(storage, key).await?;
        let file = Record::read(&stored.record)?;
        let machine = CompetitionStateMachine::build(file.log).map_err(LoadError::Replay)?;
        Ok(Self {
            saved_changes: machine.log().len(),
            competition_saved: file.competition.is_some(),
            ..Self::open(key, stored.name, machine, file.people)
        })
    }

    /// Catches up with what was saved of the competition in `storage` since this session loaded or
    /// caught up last, e.g. by a session in another tab, see [`Self::catch_up_with`]. Returns how
    /// many entries it appended.
    ///
    /// # Errors
    /// Returns an error if nothing is stored under the session's key, the storage cannot be read,
    /// or what is stored cannot be read or an entry does not apply, see [`Self::catch_up_with`].
    ///
    /// # Panics
    /// Panics if the session has unsaved changes, see [`Self::catch_up_with`].
    pub async fn catch_up<S: Storage>(
        &mut self,
        storage: &S,
    ) -> Result<usize, LoadError<S::Error>> {
        let stored = read(storage, self.key).await?;
        self.catch_up_with(&stored)
    }

    /// Catches up with `stored`, what a storage holds of this session's competition: appends the
    /// entries of its log after those of the session's, each as [`Self::load`] would have
    /// replayed it, and takes the people as stored. Returns how many entries it appended. They are
    /// no actions of this session, which undoes and redoes its own as before, see [`Self::undo`];
    /// since they may have reported and reopened again the matches this session reopened, the
    /// forms of those start out empty again. [`Self::catch_up`] reads and catches up in one;
    /// reading apart lets a GUI hold no borrow of the session while it reads.
    ///
    /// An entry that undoes or replaces an earlier one replays the log after that one, as it did
    /// when it was appended; any other entry only applies to the current state.
    ///
    /// # Errors
    /// Returns an error if what is stored cannot be read or an entry does not apply. If it cannot
    /// be read, nothing is appended; if an entry does not apply, the entries before it stay
    /// appended.
    ///
    /// # Panics
    /// Panics if the session has unsaved entries or people: what was stored after them is of
    /// another session that took over the competition, and the log would fork. An unsaved
    /// competition is no change of the session's own, e.g. of one opened read-only that an older
    /// version saved.
    pub fn catch_up_with<E: Display>(&mut self, stored: &Stored) -> Result<usize, LoadError<E>> {
        assert!(
            self.changes() == self.saved_changes,
            "a session catches up only with nothing unsaved"
        );
        let file = Record::read(&stored.record)?;
        self.people = file.people;
        self.competition_saved = file.competition.is_some();
        let entries = file.log.into_iter().skip(self.machine.log().len());
        let appended = entries.len();
        if appended > 0 {
            self.reopened.clear();
        }
        for entry in entries {
            self.machine.apply(entry).map_err(LoadError::Replay)?;
            self.saved_changes = self.changes();
        }
        Ok(appended)
    }

    /// Opens the already decoded file as a competition named `name`, with no undo history.
    /// Nothing is saved yet; the caller chooses its storage and when to save it.
    pub fn from_file(file: File<CompetitionStateMachine, People>, name: impl Into<String>) -> Self {
        let File {
            log: machine,
            people,
            ..
        } = file;
        Self::open(
            CompetitionKey::new(Uuid::new_v4()),
            name.into(),
            machine,
            people,
        )
    }

    /// The competition's file data, borrowing its log, people and competition. Its optional name
    /// can be changed or omitted before serialization, without changing the session.
    pub fn file(&self) -> File<&CompetitionStateMachine, &People, &Competition> {
        File {
            format: file::Format::CURRENT,
            name: Some(self.name.clone()),
            log: &self.machine,
            people: &self.people,
            competition: Some(self.machine.current()),
        }
    }

    /// Whether the storage lacks anything: a change since the last save, or the competition the log
    /// replays into, see [`Self::load`].
    pub fn has_unsaved_changes(&self) -> bool {
        self.changes() != self.saved_changes || !self.competition_saved
    }

    /// The competition as it is to be saved, to be written to a storage and then handed to
    /// [`Self::mark_saved`]; [`Self::save`] does both. Saving a session that has not been saved yet
    /// creates the stored competition.
    ///
    /// # Panics
    /// Panics if the log, a person or the competition does not serialize as JSON.
    pub fn to_save(&self) -> Save {
        Save {
            listing: Listing {
                key: self.key,
                name: self.name.clone(),
                progress: Some(self.progress()),
            },
            record: file::to_record(self.file()),
            changes: self.changes(),
        }
    }

    /// Notes that `save`, which [`Self::to_save`] of this session returned, has been written to its
    /// storage. What changed after it was taken stays unsaved, so that saves whose writes finish
    /// out of order leave nothing unsaved once every write has.
    ///
    /// # Panics
    /// Panics if `save` is of another competition.
    pub fn mark_saved(&mut self, save: &Save) {
        assert_eq!(
            save.listing.key, self.key,
            "the save must be of this session's competition"
        );
        self.saved_changes = self.saved_changes.max(save.changes);
        self.competition_saved = true;
    }

    /// Writes the competition to `storage`.
    ///
    /// # Errors
    /// Returns the error of the storage, in which case the changes stay unsaved and the next save
    /// writes them.
    pub async fn save<S: Storage>(&mut self, storage: &S) -> Result<(), S::Error> {
        let save = self.to_save();
        storage.write(&save).await?;
        self.mark_saved(&save);
        Ok(())
    }

    /// The competition as a file, with its name, log and people, see [`crate::file`].
    pub fn export(&self) -> String {
        file::to_json(&self.file())
    }

    /// A new competition with what the file `file` holds, e.g. one [`Self::export`] or the CLI
    /// wrote, named as in the file or else `name`, or else [`UNNAMED`] where both are blank, and
    /// without the spaces around it, followed by the first number from 2 on in
    /// parentheses that makes it a name none of `stored` has, if one has it, e.g. `"Cup (2)"`. It
    /// is stored under a key of its own, so that importing a file twice makes two competitions;
    /// nothing of it is saved yet.
    ///
    /// # Errors
    /// Returns an error if `file` is no competition's file, one of a newer version, or one whose
    /// log does not replay, see [`ImportError`].
    pub fn import(file: &str, name: &str, stored: &[Listing]) -> Result<Self, ImportError> {
        let file = File::<CompetitionStateMachine, People>::read(file)?;
        // a blank name, which the list could not show, is no name; nor is a blank file name
        let name = [file.name.as_deref(), Some(name)]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|name| !name.is_empty())
            .unwrap_or(UNNAMED)
            .to_owned();
        let taken = |name: &str| stored.iter().any(|listing| listing.name == name);
        let name = if taken(&name) {
            (2..)
                .map(|number| format!("{name} ({number})"))
                .find(|copy| !taken(copy))
                .expect("a name of the numbers is not taken")
        } else {
            name
        };
        Ok(Self::from_file(file, name))
    }
}

/// What is stored of the competition `key` in `storage`.
async fn read<S: Storage>(storage: &S, key: CompetitionKey) -> Result<Stored, LoadError<S::Error>> {
    storage
        .read(key)
        .await
        .map_err(LoadError::Storage)?
        .ok_or(LoadError::NotFound)
}

/// A stored record as a session reads it: the entries of its log, not replayed yet.
type Record = File<Vec<LogEntry<CompetitionTransition>>, People>;
