//! Competitions stored in the browser's IndexedDB.
//!
//! A database holds two object stores, each record of them a JSON text: `competitions`, the
//! listing of each competition under its key, and `saves`, its record under its key. A write is
//! one transaction over both.
//!
//! Up to version 2 of the database a competition was stored as records of its log entries
//! (`entries`, under `[competition, position]`), its people (`people`, under
//! `[competition, entrant]`) and what its log replays into (`replayed`, added in version 2, under
//! its key). Such a competition is read as the record those make up, and its next write stores the
//! record and removes them.

use std::rc::Rc;

use futures_channel::oneshot;
use futures_util::future::{try_join, try_join5};
use idb::{
    Database, KeyRange, ObjectStore, Query, TransactionMode, TransactionResult,
    builder::ObjectStoreBuilder,
};
use js_sys::Array;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tournament_app::{
    file::{File, Format},
    storage::{CompetitionKey, Listing, Progress, Save, Storage, Stored},
};
use wasm_bindgen::JsValue;

/// The version of the layout of the database, raised with every change of its object stores.
/// Opening it at a higher version closes the connections of pages of older versions, which then
/// fail to write, rather than waiting for them.
const VERSION: u32 = 3;

const COMPETITIONS: &str = "competitions";
/// Added in version 3.
const SAVES: &str = "saves";
/// Up to version 2, read and emptied since.
const ENTRIES: &str = "entries";
const PEOPLE: &str = "people";
const REPLAYED: &str = "replayed";
const STORES: [&str; 5] = [COMPETITIONS, SAVES, ENTRIES, PEOPLE, REPLAYED];

/// Stores competitions in a database of the browser's IndexedDB. Its clones share the connection
/// to it.
#[derive(Debug, Clone)]
pub struct IndexedDb(Rc<Database>);

/// Why the database could not be opened, read or written.
#[derive(Debug, Error)]
pub enum IndexedDbError {
    #[error(transparent)]
    Database(#[from] idb::Error),
    #[error("the transaction was aborted")]
    Aborted,
    #[error("a record is not what was stored")]
    Record,
}

/// What the listing of a competition is stored as.
#[derive(Serialize, Deserialize)]
struct ListingRecord {
    key: CompetitionKey,
    name: String,
    /// Its place in the order the competitions were stored in: one more than the number of the
    /// latest stored before it.
    number: u32,
    /// Absent in the records stored before it was.
    #[serde(default)]
    progress: Option<Progress>,
}

impl IndexedDb {
    /// Opens the database `name` of the page, creating it if there is none.
    ///
    /// # Errors
    /// Returns an error if the browser does not let the page open it, e.g. in a private window of
    /// some browsers, or it is of a newer version of the page.
    pub async fn open(name: &str) -> Result<Self, IndexedDbError> {
        let database = STORES
            .iter()
            .fold(
                Database::builder(name).version(VERSION),
                |builder, store| builder.add_object_store(ObjectStoreBuilder::new(store)),
            )
            .build()
            .await?;
        Ok(Self(Rc::new(database)))
    }
}

impl Storage for IndexedDb {
    type Error = IndexedDbError;

    async fn list(&self) -> Result<Vec<Listing>, IndexedDbError> {
        let transaction = self
            .0
            .transaction(&[COMPETITIONS], TransactionMode::ReadOnly)?;
        let records = transaction
            .object_store(COMPETITIONS)?
            .get_all(None, None)?
            .await?;
        let mut listings = records
            .into_iter()
            .map(|record| parse::<ListingRecord>(&record))
            .collect::<Result<Vec<_>, _>>()?;
        listings.sort_by_key(|listing| listing.number);
        Ok(listings
            .into_iter()
            .map(|listing| Listing {
                key: listing.key,
                name: listing.name,
                progress: listing.progress,
            })
            .collect())
    }

    async fn read(&self, key: CompetitionKey) -> Result<Option<Stored>, IndexedDbError> {
        let transaction = self.0.transaction(&STORES, TransactionMode::ReadOnly)?;
        let (listing, record, entries, people, replayed) = try_join5(
            transaction
                .object_store(COMPETITIONS)?
                .get(competition(key))?
                .into_future(),
            transaction
                .object_store(SAVES)?
                .get(competition(key))?
                .into_future(),
            transaction
                .object_store(ENTRIES)?
                .get_all(Some(records_of(key)?), None)?
                .into_future(),
            transaction
                .object_store(PEOPLE)?
                .get_all(Some(records_of(key)?), None)?
                .into_future(),
            transaction
                .object_store(REPLAYED)?
                .get(competition(key))?
                .into_future(),
        )
        .await?;
        let Some(listing) = listing else {
            return Ok(None);
        };
        let record = match record {
            Some(record) => text(&record)?,
            // in the order of their keys, i.e. of their positions
            None => old_record(&entries, &people, replayed.as_ref())?,
        };
        Ok(Some(Stored {
            name: parse::<ListingRecord>(&listing)?.name,
            record,
        }))
    }

    /// Starts the transaction right away, so that saves are stored in the order this is called in.
    fn write(&self, save: &Save) -> impl Future<Output = Result<(), IndexedDbError>> + use<> {
        // The transaction stays active only while the browser hands it the result of a request, and
        // the microtasks queued then, so the writes following the reads have to be made right after
        // them. The executor of wasm-bindgen-futures continues in such a microtask; another one,
        // e.g. a GUI's, might not. It runs the tasks in the order they are spawned in, and
        // IndexedDB runs transactions over the same stores in the order they are created in.
        let (sender, receiver) = oneshot::channel();
        let database = Rc::clone(&self.0);
        let save = save.clone();
        wasm_bindgen_futures::spawn_local(async move {
            _ = sender.send(write(&database, &save).await);
        });
        async { receiver.await.unwrap_or(Err(IndexedDbError::Aborted)) }
    }

    /// Starts the transaction right away, as [`Self::write`] does.
    fn rename(
        &self,
        key: CompetitionKey,
        name: &str,
    ) -> impl Future<Output = Result<(), IndexedDbError>> {
        let (sender, receiver) = oneshot::channel();
        let database = Rc::clone(&self.0);
        let name = name.to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            _ = sender.send(rename(&database, key, name).await);
        });
        async { receiver.await.unwrap_or(Err(IndexedDbError::Aborted)) }
    }

    async fn delete(&self, key: CompetitionKey) -> Result<(), IndexedDbError> {
        let transaction = self.0.transaction(&STORES, TransactionMode::ReadWrite)?;
        transaction
            .object_store(COMPETITIONS)?
            .delete(competition(key))?;
        transaction.object_store(SAVES)?.delete(competition(key))?;
        delete_old(&transaction, key)?;
        match transaction.commit()?.await? {
            TransactionResult::Committed => Ok(()),
            TransactionResult::Aborted => Err(IndexedDbError::Aborted),
        }
    }
}

async fn rename(
    database: &Database,
    key: CompetitionKey,
    name: String,
) -> Result<(), IndexedDbError> {
    let transaction = database.transaction(&[COMPETITIONS], TransactionMode::ReadWrite)?;
    let competitions = transaction.object_store(COMPETITIONS)?;
    let Some(listing) = competitions.get(competition(key))?.await? else {
        transaction.abort()?;
        return Ok(());
    };
    let listing = ListingRecord {
        name,
        ..parse(&listing)?
    };
    put(
        &competitions,
        &serde_json::to_string(&listing).expect("a listing serializes as JSON"),
        &competition(key),
    )?;
    match transaction.commit()?.await? {
        TransactionResult::Committed => Ok(()),
        TransactionResult::Aborted => Err(IndexedDbError::Aborted),
    }
}

async fn write(database: &Database, save: &Save) -> Result<(), IndexedDbError> {
    let transaction = database.transaction(&STORES, TransactionMode::ReadWrite)?;
    let competitions = transaction.object_store(COMPETITIONS)?;
    let key = save.listing().key;
    let (listing, listings) = try_join(
        competitions.get(competition(key))?.into_future(),
        competitions.get_all(None, None)?.into_future(),
    )
    .await?;
    let listing = match listing {
        Some(listing) => ListingRecord {
            progress: save.listing().progress.clone(),
            ..parse(&listing)?
        },
        None => ListingRecord {
            key,
            name: save.listing().name.clone(),
            // after the latest stored, whatever was deleted since
            number: listings
                .iter()
                .map(|listing| parse::<ListingRecord>(listing).map(|listing| listing.number + 1))
                .try_fold(0, |highest, number| {
                    number.map(|number| highest.max(number))
                })?,
            progress: save.listing().progress.clone(),
        },
    };
    put(
        &competitions,
        &serde_json::to_string(&listing).expect("a listing serializes as JSON"),
        &competition(key),
    )?;
    put(
        &transaction.object_store(SAVES)?,
        save.record(),
        &competition(key),
    )?;
    delete_old(&transaction, key)?;
    match transaction.commit()?.await? {
        TransactionResult::Committed => Ok(()),
        TransactionResult::Aborted => Err(IndexedDbError::Aborted),
    }
}

/// The record the stores of version 2 hold of a competition: the file of its `entries`, `people`
/// and what it `replayed` into, if anything. A text that is no JSON, which no session stored, is
/// kept as the text it is.
fn old_record(
    entries: &[JsValue],
    people: &[JsValue],
    replayed: Option<&JsValue>,
) -> Result<String, IndexedDbError> {
    let value = |record: &JsValue| -> Result<Value, IndexedDbError> {
        let text = text(record)?;
        Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
    };
    let file = File {
        format: Format::FIRST,
        name: None,
        log: entries.iter().map(value).collect::<Result<Vec<_>, _>>()?,
        people: people.iter().map(value).collect::<Result<Vec<_>, _>>()?,
        competition: replayed.map(value).transpose()?,
    };
    Ok(serde_json::to_string(&file).expect("a file serializes as JSON"))
}

/// Deletes what the stores of version 2 hold of the competition `key`, without waiting for it: the
/// transaction fails if it does.
fn delete_old(transaction: &idb::Transaction, key: CompetitionKey) -> Result<(), idb::Error> {
    transaction
        .object_store(ENTRIES)?
        .delete(records_of(key)?)?;
    transaction.object_store(PEOPLE)?.delete(records_of(key)?)?;
    transaction
        .object_store(REPLAYED)?
        .delete(competition(key))?;
    Ok(())
}

/// Puts `text` into `store` under `key`, without waiting for it: the transaction fails if it does.
fn put(store: &ObjectStore, text: &str, key: &JsValue) -> Result<(), idb::Error> {
    store.put(&JsValue::from_str(text), Some(key))?;
    Ok(())
}

/// The key of the competition `key` in the stores of competitions, saves and what they replay
/// into.
fn competition(key: CompetitionKey) -> JsValue {
    JsValue::from(key.to_string())
}

/// The keys of every record of the competition `key` in the store of entries or of people: those
/// after `[key]` and before `[key, []]`, since an array is greater than any number or text.
fn records_of(key: CompetitionKey) -> Result<Query, idb::Error> {
    let lower = Array::of1(&competition(key));
    let upper = Array::of2(&competition(key), &Array::new());
    Ok(KeyRange::bound(&lower, &upper, None, None)?.into())
}

fn text(record: &JsValue) -> Result<String, IndexedDbError> {
    record.as_string().ok_or(IndexedDbError::Record)
}

fn parse<T: for<'de> Deserialize<'de>>(record: &JsValue) -> Result<T, IndexedDbError> {
    serde_json::from_str(&text(record)?).map_err(|_| IndexedDbError::Record)
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests;
