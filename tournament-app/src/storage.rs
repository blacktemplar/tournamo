//! Where a [`Session`](crate::session::Session) saves its competition and loads it from.
//!
//! A competition is stored as its [`Listing`] and one record, rewritten by every save, see
//! [`Save`]. What the record holds is the session's business (its file, see [`crate::file`]); a
//! storage keeps it as the text it is handed.

use std::{
    cell::RefCell,
    convert::Infallible,
    error::Error,
    fmt::{self, Display},
    future,
    num::NonZeroU32,
    rc::Rc,
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tournament_engine::uuid::Uuid;

/// The key a competition is stored under, independent of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompetitionKey(Uuid);

impl CompetitionKey {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl Display for CompetitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A stored competition as a list of competitions to open shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub key: CompetitionKey,
    /// The name the operator gave the competition.
    pub name: String,
    /// How far it is, as of its last save; `None` if it was stored before the progress was.
    pub progress: Option<Progress>,
}

impl Listing {
    /// Whether a competition of `listings` other than `besides`, e.g. the one being renamed, is
    /// named `name`, without the spaces around it, so that an operator naming another one so may
    /// want to know.
    pub fn is_taken(listings: &[Self], name: &str, besides: Option<CompetitionKey>) -> bool {
        let name = name.trim();
        listings
            .iter()
            .any(|listing| Some(listing.key) != besides && listing.name == name)
    }
}

/// How far a competition is, as a list of competitions shows it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Progress {
    /// Not started yet.
    Setup { entrants: usize },
    /// Started, with matches without a result.
    Playing {
        /// The first round with a match without a result.
        round: NonZeroU32,
        /// The matches of that round without a result.
        open: usize,
    },
    /// Started, and every match scheduled has a result, as stored before the winners were; a
    /// session tells [`Self::Won`] instead.
    Finished,
    /// Started, and every match scheduled has a result.
    Won {
        /// The teams ranked first, more than one if they are tied, each by the names of its
        /// entrants, `None` for one without a name (see [`EntrantRow::name`]).
        ///
        /// [`EntrantRow::name`]: crate::view::EntrantRow::name
        winners: Vec<Vec<Option<String>>>,
    },
}

/// What is stored of a competition, as [`Storage::read`] returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub name: String,
    /// The record the latest save wrote, see [`Save::record`].
    pub record: String,
}

/// A competition as a session saves it, to be written by [`Storage::write`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Save {
    pub(crate) listing: Listing,
    pub(crate) record: String,
    /// The number of changes the session had made when it was taken, see
    /// [`Session::mark_saved`](crate::session::Session::mark_saved).
    pub(crate) changes: usize,
}

impl Save {
    /// The competition saved. Writing creates its listing if there is none yet, and otherwise
    /// updates its [progress](Listing::progress).
    pub fn listing(&self) -> &Listing {
        &self.listing
    }

    /// Everything stored of the competition but its listing, replacing what is stored.
    pub fn record(&self) -> &str {
        &self.record
    }
}

/// Holds stored competitions.
///
/// Its methods return futures, since storage in a browser is asynchronous. A storage that is not
/// returns futures that are ready right away, which a synchronous caller can poll once.
pub trait Storage {
    type Error: Error + 'static;

    /// Every stored competition, in the order they were first written.
    ///
    /// # Errors
    /// Returns an error if the storage cannot be read.
    fn list(&self) -> impl Future<Output = Result<Vec<Listing>, Self::Error>>;

    /// What is stored of the competition `key`, or `None` if nothing is.
    ///
    /// # Errors
    /// Returns an error if the storage cannot be read.
    fn read(
        &self,
        key: CompetitionKey,
    ) -> impl Future<Output = Result<Option<Stored>, Self::Error>>;

    /// Stores `save`, all of it or, if that fails, nothing. Saves are stored in the order this is
    /// called in: a save written after another one stores over it, even if the write of the other
    /// one finishes later. So the write starts when this is called, and the future, which only
    /// tells when it is done, borrows neither the storage nor the save.
    ///
    /// # Errors
    /// Returns an error if the save cannot be stored, in which case what was stored before is still
    /// there.
    fn write(&self, save: &Save) -> impl Future<Output = Result<(), Self::Error>> + use<Self>;

    /// Names the competition `key` `name`, which later writes keep; nothing if nothing is stored
    /// of it. A session holding it keeps the name it had.
    ///
    /// # Errors
    /// Returns an error if the name cannot be stored.
    fn rename(
        &self,
        key: CompetitionKey,
        name: &str,
    ) -> impl Future<Output = Result<(), Self::Error>>;

    /// Removes everything stored of the competition `key`; nothing if nothing is.
    ///
    /// # Errors
    /// Returns an error if it cannot be removed, in which case all of it is still there.
    fn delete(&self, key: CompetitionKey) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Stores in memory, for tests and until a page stores in the browser. Its clones share what is
/// stored, so that a test can load in one session what another one saved.
#[derive(Debug, Clone, Default)]
pub struct MemoryStorage(Rc<RefCell<IndexMap<CompetitionKey, Competition>>>);

#[derive(Debug)]
struct Competition {
    name: String,
    progress: Option<Progress>,
    record: String,
}

#[cfg(test)]
impl MemoryStorage {
    /// Replaces the record of the competition `key`, e.g. with one an older version wrote.
    ///
    /// # Panics
    /// Panics if nothing is stored of it.
    pub(crate) fn set_record(&self, key: CompetitionKey, record: String) {
        self.0.borrow_mut()[&key].record = record;
    }
}

impl Storage for MemoryStorage {
    type Error = Infallible;

    async fn list(&self) -> Result<Vec<Listing>, Infallible> {
        Ok(self
            .0
            .borrow()
            .iter()
            .map(|(key, competition)| Listing {
                key: *key,
                name: competition.name.clone(),
                progress: competition.progress.clone(),
            })
            .collect())
    }

    async fn read(&self, key: CompetitionKey) -> Result<Option<Stored>, Infallible> {
        Ok(self.0.borrow().get(&key).map(|competition| Stored {
            name: competition.name.clone(),
            record: competition.record.clone(),
        }))
    }

    /// Stores when called, before the future is polled.
    fn write(&self, save: &Save) -> impl Future<Output = Result<(), Infallible>> + use<> {
        let mut competitions = self.0.borrow_mut();
        let competition = competitions
            .entry(save.listing.key)
            .or_insert_with(|| Competition {
                name: save.listing.name.clone(),
                progress: None,
                record: String::new(),
            });
        competition.progress.clone_from(&save.listing.progress);
        competition.record.clone_from(&save.record);
        future::ready(Ok(()))
    }

    async fn rename(&self, key: CompetitionKey, name: &str) -> Result<(), Infallible> {
        if let Some(competition) = self.0.borrow_mut().get_mut(&key) {
            name.clone_into(&mut competition.name);
        }
        Ok(())
    }

    async fn delete(&self, key: CompetitionKey) -> Result<(), Infallible> {
        self.0.borrow_mut().shift_remove(&key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn a_name_is_taken_by_another_competition_only() {
        let listing = |number, name: &str| Listing {
            key: CompetitionKey::new(Uuid::from_u128(number)),
            name: name.to_owned(),
            progress: None,
        };
        let listings = [listing(1, "Cup"), listing(2, "League")];

        assert!(Listing::is_taken(&listings, " Cup ", None));
        assert!(!Listing::is_taken(&listings, "Cup", Some(listings[0].key)));
        assert!(Listing::is_taken(
            &listings,
            "League",
            Some(listings[0].key)
        ));
        assert!(!Listing::is_taken(&listings, "Final", None));
    }
}
