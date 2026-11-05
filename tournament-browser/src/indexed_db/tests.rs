use tournament_app::session::Session;
use tournament_engine::map::IdAble as _;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

use super::*;

wasm_bindgen_test_configure!(run_in_browser);

/// A database of its own, as the tests run in the same browser.
async fn database() -> IndexedDb {
    IndexedDb::open(&format!("test-{}", uuid::Uuid::new_v4()))
        .await
        .unwrap()
}

fn names(session: &Session) -> Vec<Option<String>> {
    session
        .entrants()
        .into_iter()
        .map(|entrant| entrant.name)
        .collect()
}

/// The key of the record `id` of the competition `key` in the store of entries or of people.
fn record(key: CompetitionKey, id: JsValue) -> JsValue {
    Array::of2(&competition(key), &id).into()
}

/// The record stored of the competition `key`, as JSON.
async fn stored_record(storage: &IndexedDb, key: CompetitionKey) -> Value {
    let stored = storage.read(key).await.unwrap().unwrap();
    serde_json::from_str(&stored.record).unwrap()
}

#[wasm_bindgen_test]
async fn a_saved_competition_loads_as_it_was_saved() {
    let storage = database().await;
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    session.save(&storage).await.unwrap();
    session.add_entrant("Bob").unwrap();
    session.save(&storage).await.unwrap();

    let loaded = Session::load(&storage, session.key()).await.unwrap();

    assert_eq!(loaded.name(), "cup");
    assert_eq!(names(&loaded), names(&session));
    assert!(!loaded.has_unsaved_changes());
    assert!(
        Session::load(&storage, CompetitionKey::new(uuid::Uuid::new_v4()))
            .await
            .is_err()
    );
}

#[wasm_bindgen_test]
async fn a_session_catches_up_with_what_another_one_saved() {
    let storage = database().await;
    let mut changing = Session::new("cup");
    changing.add_entrant("Alice").unwrap();
    changing.save(&storage).await.unwrap();
    let mut following = Session::load(&storage, changing.key()).await.unwrap();

    changing.add_entrant("Bob").unwrap();
    changing.undo().unwrap();
    changing.save(&storage).await.unwrap();

    assert_eq!(following.catch_up(&storage).await.unwrap(), 2);
    assert_eq!(names(&following), names(&changing));
}

#[wasm_bindgen_test]
async fn competitions_are_listed_in_the_order_they_were_first_saved() {
    let storage = database().await;
    let mut sessions: Vec<_> = ["league", "cup", "open"].map(Session::new).into();
    for session in &mut sessions {
        session.save(&storage).await.unwrap();
    }
    sessions[0].add_entrant("Alice").unwrap();
    sessions[0].save(&storage).await.unwrap();

    let listed = storage.list().await.unwrap();

    let expected: Vec<_> = sessions
        .iter()
        .map(|session| Listing {
            key: session.key(),
            name: session.name().to_owned(),
            // that of league updated by its second save
            progress: Some(session.progress()),
        })
        .collect();
    assert_eq!(listed, expected);
}

/// A competition renamed keeps its name through later saves, and one deleted is gone; one saved
/// after a deletion is still listed last.
#[wasm_bindgen_test]
async fn competitions_are_renamed_and_deleted() {
    let storage = database().await;
    let mut sessions: Vec<_> = ["league", "cup"].map(Session::new).into();
    for session in &mut sessions {
        session.add_entrant("Alice").unwrap();
        session.save(&storage).await.unwrap();
    }

    storage.rename(sessions[1].key(), "Cup 2026").await.unwrap();
    sessions[1].add_entrant("Bob").unwrap();
    sessions[1].save(&storage).await.unwrap();
    storage.delete(sessions[0].key()).await.unwrap();
    let mut open = Session::new("open");
    open.save(&storage).await.unwrap();

    let names: Vec<_> = storage
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|listing| listing.name)
        .collect();
    assert_eq!(names, ["Cup 2026", "open"]);
    assert!(storage.read(sessions[0].key()).await.unwrap().is_none());
    let loaded = Session::load(&storage, sessions[1].key()).await.unwrap();
    assert_eq!(loaded.name(), "Cup 2026");
    assert_eq!(loaded.entrants().len(), 2);
}

/// A GUI writes while the operator goes on, so a write may be awaited after a later one.
#[wasm_bindgen_test]
async fn saves_are_stored_in_the_order_they_are_written() {
    let storage = database().await;
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    let first = session.to_save();
    session.add_entrant("Bob").unwrap();
    let second = session.to_save();

    let first = storage.write(&first);
    let second = storage.write(&second);
    second.await.unwrap();
    first.await.unwrap();

    let loaded = Session::load(&storage, session.key()).await.unwrap();
    assert_eq!(names(&loaded), names(&session));
    let listed = storage.list().await.unwrap();
    assert_eq!(listed[0].progress, Some(session.progress()));
}

/// A competition is stored as its file, so that it is exported as it is.
#[wasm_bindgen_test]
async fn a_competition_is_stored_as_its_file_without_the_name() {
    let storage = database().await;
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    session.save(&storage).await.unwrap();

    let mut file: Value = serde_json::from_str(&session.export()).unwrap();
    file.as_object_mut().unwrap().remove("name");
    assert_eq!(stored_record(&storage, session.key()).await, file);
}

/// Writes `session` to the database `name` as a page of version 1 (`replayed: false`) or 2 stored
/// it.
async fn store_as_before(name: &str, session: &Session, replayed: bool) {
    let (version, stores) = if replayed {
        (2, &[COMPETITIONS, ENTRIES, PEOPLE, REPLAYED][..])
    } else {
        (1, &[COMPETITIONS, ENTRIES, PEOPLE][..])
    };
    let database = stores
        .iter()
        .fold(
            Database::builder(name).version(version),
            |builder, store| builder.add_object_store(ObjectStoreBuilder::new(store)),
        )
        .build()
        .await
        .unwrap();
    let transaction = database
        .transaction(stores, TransactionMode::ReadWrite)
        .unwrap();
    let listing = ListingRecord {
        key: session.key(),
        name: session.name().to_owned(),
        number: 0,
        progress: None,
    };
    put(
        &transaction.object_store(COMPETITIONS).unwrap(),
        &serde_json::to_string(&listing).unwrap(),
        &competition(session.key()),
    )
    .unwrap();
    for (position, entry) in session.file().log.log().iter().enumerate() {
        put(
            &transaction.object_store(ENTRIES).unwrap(),
            &serde_json::to_string(entry).unwrap(),
            &record(session.key(), JsValue::from(position)),
        )
        .unwrap();
    }
    for person in session.people().iter() {
        let entrant = serde_json::to_value(person.id()).unwrap();
        put(
            &transaction.object_store(PEOPLE).unwrap(),
            &serde_json::to_string(person).unwrap(),
            &record(session.key(), JsValue::from(entrant.as_str().unwrap())),
        )
        .unwrap();
    }
    if replayed {
        put(
            &transaction.object_store(REPLAYED).unwrap(),
            &serde_json::to_string(session.competition()).unwrap(),
            &competition(session.key()),
        )
        .unwrap();
    }
    transaction.commit().unwrap().await.unwrap();
    database.close();
}

/// A tab of an older version still open does not keep a newer one from opening the database, and
/// what the older version stored is read as the record it makes up, and moved to one by the next
/// save.
#[wasm_bindgen_test]
async fn a_database_of_version_2_is_read_and_moved_over_by_the_next_save() {
    let name = format!("test-{}", uuid::Uuid::new_v4());
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    store_as_before(&name, &session, true).await;

    let storage = IndexedDb::open(&name).await.unwrap();

    let mut loaded = Session::load(&storage, session.key()).await.unwrap();
    assert_eq!(names(&loaded), names(&session));
    assert!(!loaded.has_unsaved_changes(), "nothing is missing");
    let mut file: Value = serde_json::from_str(&session.export()).unwrap();
    file.as_object_mut().unwrap().remove("name");
    // version 2 stored the first format, which this session's log fits, holding no result
    file["format"] = 1.into();
    assert_eq!(stored_record(&storage, session.key()).await, file);
    loaded.add_entrant("Bob").unwrap();
    loaded.save(&storage).await.unwrap();
    let reloaded = Session::load(&storage, session.key()).await.unwrap();
    assert_eq!(names(&reloaded), names(&loaded));
    let transaction = storage
        .0
        .transaction(&[ENTRIES, PEOPLE, REPLAYED], TransactionMode::ReadOnly)
        .unwrap();
    let left = (
        transaction
            .object_store(ENTRIES)
            .unwrap()
            .count(None)
            .unwrap()
            .await
            .unwrap(),
        transaction
            .object_store(PEOPLE)
            .unwrap()
            .count(None)
            .unwrap()
            .await
            .unwrap(),
        transaction
            .object_store(REPLAYED)
            .unwrap()
            .count(None)
            .unwrap()
            .await
            .unwrap(),
    );
    assert_eq!(left, (0, 0, 0), "the old records are removed");
}

/// What the first version stored lacks the competition, which the next save adds.
#[wasm_bindgen_test]
async fn a_database_of_the_first_version_is_read_with_what_it_holds() {
    let name = format!("test-{}", uuid::Uuid::new_v4());
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    store_as_before(&name, &session, false).await;

    let storage = IndexedDb::open(&name).await.unwrap();

    let mut loaded = Session::load(&storage, session.key()).await.unwrap();
    assert_eq!(names(&loaded), names(&session));
    assert!(
        loaded.has_unsaved_changes(),
        "the competition is not stored"
    );
    loaded.save(&storage).await.unwrap();
    assert_eq!(
        stored_record(&storage, session.key()).await["competition"],
        serde_json::to_value(session.competition()).unwrap()
    );
}

/// The log knows the entrants by id only, so that their names can be deleted without touching the
/// log.
#[wasm_bindgen_test]
async fn names_are_stored_apart_from_the_log() {
    let storage = database().await;
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    session.save(&storage).await.unwrap();

    let record = stored_record(&storage, session.key()).await;
    assert!(!record["log"].to_string().contains("Alice"));
    assert!(!record["competition"].to_string().contains("Alice"));
    assert!(record["people"].to_string().contains("Alice"));
}

#[wasm_bindgen_test]
async fn competitions_are_stored_apart_from_each_other() {
    let storage = database().await;
    let (mut cup, mut league) = (Session::new("cup"), Session::new("league"));
    cup.add_entrant("Alice").unwrap();
    league.add_entrant("Bob").unwrap();

    cup.save(&storage).await.unwrap();
    league.save(&storage).await.unwrap();

    let loaded = Session::load(&storage, cup.key()).await.unwrap();
    assert_eq!(names(&loaded), [Some("Alice".to_owned())]);
}
