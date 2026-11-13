#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;

/// A file without a name, or with a blank one, is named after the file, or else by a name of its
/// own, never by nothing, which the list could not show.
#[test]
fn an_imported_competition_is_never_nameless() {
    let file = Given::new().added().export();
    let blank = file.replacen("\"name\": \"cup\"", "\"name\": \"  \"", 1);
    assert_ne!(blank, file, "the file names its competition");

    assert_eq!(
        Session::import(&blank, "league", &[]).unwrap().name(),
        "league"
    );
    assert_eq!(
        Session::import(&blank, " ", &[]).unwrap().name(),
        "Competition"
    );
}

/// A listing stored before the winners were reads as it did.
#[test]
fn a_progress_stored_without_the_winners_still_reads() {
    assert_eq!(
        serde_json::from_str::<Progress>(r#""Finished""#).unwrap(),
        Progress::Finished
    );
}

#[test]
fn a_save_written_after_another_stores_over_it_whichever_finishes_first() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    let older = session.to_save();
    session.add_entrant("Bob").unwrap();
    let newer = session.to_save();

    let older = storage.write(&older);
    let newer = storage.write(&newer);
    now(newer).unwrap();
    now(older).unwrap();

    let listed = now(storage.list()).unwrap();
    assert_eq!(listed[0].progress, Some(Progress::Setup { entrants: 2 }));
    let loaded = now(Session::load(&storage, session.key())).unwrap();
    assert_eq!(names(&loaded.entrants()), ["Alice", "Bob"]);
}

/// What can be undone is what a session did itself, so a loaded competition has the state it was
/// saved with but nothing to undo.
#[test]
fn a_saved_competition_loads_as_it_was_saved_without_anything_to_undo() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    now(session.save(&storage)).unwrap();
    let mut loaded = now(Session::load(&storage, session.key())).unwrap();

    assert_eq!(loaded.name(), "cup");
    assert!(!loaded.has_unsaved_changes());
    assert_eq!(loaded.entrants(), session.entrants());
    assert_eq!(matches(&loaded), matches(&session));
    assert_eq!(loaded.standings(), session.standings());
    assert!(matches!(loaded.undo(), Err(UndoError::NothingToUndo)));
    // a single result can still be undone, as it is found in the log
    loaded.undo_report(numbered(&loaded, 1)).unwrap();
    assert_eq!(
        now(storage.list()).unwrap(),
        [Listing {
            key: session.key(),
            name: "cup".to_owned(),
            progress: Some(session.progress()),
        }]
    );
}

#[test]
fn every_change_is_unsaved_until_the_next_save() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    assert!(session.has_unsaved_changes(), "the seed is not saved yet");
    now(session.save(&storage)).unwrap();
    assert!(!session.has_unsaved_changes());

    session.add_entrant("Alice").unwrap();
    assert!(session.has_unsaved_changes());
    now(session.save(&storage)).unwrap();
    assert!(!session.has_unsaved_changes());
    session.undo().unwrap();
    assert!(
        session.has_unsaved_changes(),
        "an undo is an entry of the log too"
    );
}

/// The record of the competition `key` in `storage`, as JSON.
fn record(storage: &MemoryStorage, key: CompetitionKey) -> serde_json::Value {
    let stored = now(storage.read(key)).unwrap().unwrap();
    serde_json::from_str(&stored.record).unwrap()
}

/// Leaves the competition `key` as a version that did not store the competition its log replays
/// into would have.
fn forget_competition(storage: &MemoryStorage, key: CompetitionKey) {
    let mut record = record(storage, key);
    record.as_object_mut().unwrap().remove("competition");
    storage.set_record(key, record.to_string());
}

/// A competition is stored as its file, so that it is exported as it is.
#[test]
fn a_save_stores_the_file_without_the_name() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    now(session.save(&storage)).unwrap();

    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    file.as_object_mut().unwrap().remove("name");
    assert_eq!(record(&storage, session.key()), file);
}

#[test]
fn a_competition_saved_by_a_newer_version_is_refused_as_such() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    now(session.save(&storage)).unwrap();
    let mut record = record(&storage, session.key());
    record["format"] = serde_json::json!(1000);
    storage.set_record(session.key(), record.to_string());

    assert!(matches!(
        now(Session::load(&storage, session.key())),
        Err(LoadError::Newer(_))
    ));
}

/// Opening a competition an older version saved is enough to store what its log replays into,
/// while that version's log still replays.
#[test]
fn a_competition_stored_without_what_its_log_replays_into_gets_it_on_the_next_save() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    now(session.save(&storage)).unwrap();
    forget_competition(&storage, session.key());

    let mut loaded = now(Session::load(&storage, session.key())).unwrap();

    assert!(loaded.has_unsaved_changes());
    now(loaded.save(&storage)).unwrap();
    assert!(!loaded.has_unsaved_changes());
    assert_eq!(
        record(&storage, session.key())["competition"],
        serde_json::to_value(session.competition()).unwrap()
    );
    let reloaded = now(Session::load(&storage, session.key())).unwrap();
    assert!(!reloaded.has_unsaved_changes());
}

/// A tab that opened such a competition read-only follows the one changing it.
#[test]
fn a_competition_stored_without_what_its_log_replays_into_is_caught_up_with() {
    let storage = MemoryStorage::default();
    let mut changing = Given::new().started();
    now(changing.save(&storage)).unwrap();
    forget_competition(&storage, changing.key());
    let mut following = now(Session::load(&storage, changing.key())).unwrap();

    changing
        .report(numbered(&changing, 1), MatchResult::AWins)
        .unwrap();
    now(changing.save(&storage)).unwrap();

    assert_eq!(now(following.catch_up(&storage)).unwrap(), 1);
    assert!(
        !following.has_unsaved_changes(),
        "the storage holds it again"
    );
}

/// A GUI writes changes while the operator goes on, so a write may finish after a later one.
#[test]
fn saves_written_out_of_order_leave_nothing_unsaved() {
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    let first = session.to_save();
    session.add_entrant("Bob").unwrap();
    let second = session.to_save();

    session.mark_saved(&second);
    session.mark_saved(&first);

    assert!(!session.has_unsaved_changes());
}

/// A save taken before a person changed leaves that change unsaved once it is written.
#[test]
fn a_person_changed_after_the_save_was_taken_stays_unsaved() {
    let mut session = Session::new("cup");
    let alice = session.add_entrant("Alice").unwrap();
    let save = session.to_save();
    session.rename_entrant(alice, "Alicia").unwrap();

    session.mark_saved(&save);

    assert!(session.has_unsaved_changes());
}

/// Another tab, say, shows what the session changing the competition saved, without replaying the
/// log each time.
#[test]
fn a_session_catches_up_with_what_another_one_saved() {
    let storage = MemoryStorage::default();
    let mut changing = Given::new().started();
    now(changing.save(&storage)).unwrap();
    let mut following = now(Session::load(&storage, changing.key())).unwrap();

    changing
        .report(numbered(&changing, 1), MatchResult::AWins)
        .unwrap();
    changing
        .report(numbered(&changing, 2), MatchResult::BWins)
        .unwrap();
    // entries that undo an earlier one, which replay the log after it
    changing.undo_report(numbered(&changing, 1)).unwrap();
    changing.undo().unwrap();
    now(changing.save(&storage)).unwrap();

    assert_eq!(now(following.catch_up(&storage)).unwrap(), 4);
    assert_eq!(matches(&following), matches(&changing));
    assert_eq!(following.standings(), changing.standings());
    assert!(!following.has_unsaved_changes());
    assert!(matches!(following.undo(), Err(UndoError::NothingToUndo)));
    assert_eq!(now(following.catch_up(&storage)).unwrap(), 0);
}

#[test]
fn catching_up_takes_the_people_as_stored() {
    let storage = MemoryStorage::default();
    let mut changing = Session::new("cup");
    changing.add_entrant("Alice").unwrap();
    now(changing.save(&storage)).unwrap();
    let mut following = now(Session::load(&storage, changing.key())).unwrap();

    changing.add_entrant("Bob").unwrap();
    now(changing.save(&storage)).unwrap();
    now(following.catch_up(&storage)).unwrap();

    assert_eq!(names(&following.entrants()), ["Alice", "Bob"]);
}

/// What another session saved may have reported a match this one reopened, and reopened it again,
/// so after catching up, the form of the match no longer starts out at the result this one took
/// back.
#[test]
fn catching_up_drops_the_results_taken_back() {
    let storage = MemoryStorage::default();
    let mut session = Given::new()
        .match_format(PluginConfig::of::<Scored>())
        .started();
    session.report_entered("1 21-17").unwrap();
    session.undo_report(numbered(&session, 1)).unwrap();
    now(session.save(&storage)).unwrap();
    let mut other = now(Session::load(&storage, session.key())).unwrap();
    other.report_entered("1 21-19").unwrap();
    other.undo_report(numbered(&other, 1)).unwrap();
    now(other.save(&storage)).unwrap();

    now(session.catch_up(&storage)).unwrap();

    let form = session.result_form(numbered(&session, 1)).unwrap();
    assert_eq!(form.view().root.fields[0].values, Sides::new(None, None));
}

/// The started competition of `session` saved to `storage`, and a session of another tab that
/// loaded it.
fn shared(session: &mut Session, storage: &MemoryStorage) -> Session {
    now(session.save(storage)).unwrap();
    now(Session::load(storage, session.key())).unwrap()
}

/// A result another session reported stays when this one undoes its own.
#[test]
fn undo_takes_back_the_session_s_own_result_and_not_one_caught_up_with() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 2), MatchResult::BWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    let undone = session.undo().unwrap();

    assert!(matches!(undone, Action::Report(row) if row.number == 1));
    let results: Vec<_> = matches(&session).iter().map(|row| row.result).collect();
    assert_eq!(results[..2], [None, Some(MatchResult::BWins)]);
}

#[test]
fn redo_brings_back_what_the_session_undid_after_catching_up() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    session.undo().unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 2), MatchResult::BWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    assert!(session.actions().redo);
    assert!(matches!(session.redo().unwrap(), Action::Report(row) if row.number == 1));
}

/// Undoing a result another session reopened already would undo an older action while the result
/// could still come back, so the session undoes nothing more.
#[test]
fn a_result_another_session_reopened_leaves_nothing_to_undo() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other.undo_report(numbered(&other, 1)).unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    assert!(!session.actions().undo);
    assert_eq!(session.next_undo(), None);
    assert!(matches!(session.undo(), Err(UndoError::ChangedElsewhere)));
    assert!(matches!(session.undo(), Err(UndoError::NothingToUndo)));
    assert!(
        session.competition().is_started(),
        "the start is not undone"
    );
}

/// The final another session reported depends on the semifinal this one reported last.
#[test]
fn a_result_another_session_s_result_depends_on_is_not_undone_and_ends_undoing() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().format(SingleElimination::ID).started();
    let mut other = shared(&mut session, &storage);
    for number in [1, 2] {
        session
            .report(numbered(&session, number), MatchResult::AWins)
            .unwrap();
    }
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 3), MatchResult::AWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();
    let before = session.export();

    assert!(matches!(session.next_undo(), Some(Action::Report(row)) if row.number == 2));
    assert!(matches!(session.undo(), Err(UndoError::ChangedElsewhere)));

    assert_eq!(session.export(), before);
    assert!(!session.actions().undo);
}

/// A result this session undid and another one reported again cannot be redone, while what this
/// session did before stays undoable.
#[test]
fn a_result_another_session_reported_again_leaves_nothing_to_redo() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    let mut other = shared(&mut session, &storage);
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();
    session
        .report(numbered(&session, 2), MatchResult::AWins)
        .unwrap();
    session.undo().unwrap();
    now(session.save(&storage)).unwrap();
    now(other.catch_up(&storage)).unwrap();
    other
        .report(numbered(&other, 2), MatchResult::BWins)
        .unwrap();
    now(other.save(&storage)).unwrap();
    now(session.catch_up(&storage)).unwrap();

    assert!(matches!(session.redo(), Err(RedoError::ChangedElsewhere)));

    assert!(!session.actions().redo);
    assert!(matches!(session.undo().unwrap(), Action::Report(row) if row.number == 1));
}

#[test]
fn an_entry_stored_that_does_not_apply_is_not_caught_up_with() {
    let storage = MemoryStorage::default();
    let mut changing = Session::new("cup");
    now(changing.save(&storage)).unwrap();
    let mut following = now(Session::load(&storage, changing.key())).unwrap();
    changing.add_entrant("Alice").unwrap();
    now(changing.save(&storage)).unwrap();
    let mut record = record(&storage, changing.key());
    record["log"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "unknown": true }));
    storage.set_record(changing.key(), record.to_string());

    assert!(matches!(
        now(following.catch_up(&storage)),
        Err(LoadError::Damaged(_))
    ));
    assert!(following.entrants().is_empty(), "nothing is appended");
}

#[test]
#[should_panic = "a session catches up only with nothing unsaved"]
#[cfg_attr(target_arch = "wasm32", ignore = "WebAssembly aborts on a panic")]
fn a_session_with_unsaved_changes_does_not_catch_up() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    now(session.save(&storage)).unwrap();
    session.add_entrant("Alice").unwrap();

    _ = now(session.catch_up(&storage));
}

/// A storage whose writes fail as long as it is told to.
#[derive(Default)]
struct Flaky {
    storage: MemoryStorage,
    failing: Cell<bool>,
}

#[derive(Debug)]
struct WriteFailed;

impl fmt::Display for WriteFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the write failed")
    }
}

impl std::error::Error for WriteFailed {}

impl Storage for Flaky {
    type Error = WriteFailed;

    async fn list(&self) -> Result<Vec<Listing>, WriteFailed> {
        Ok(now(self.storage.list()).unwrap())
    }

    async fn read(&self, key: CompetitionKey) -> Result<Option<Stored>, WriteFailed> {
        Ok(now(self.storage.read(key)).unwrap())
    }

    fn write(&self, save: &Save) -> impl Future<Output = Result<(), WriteFailed>> + use<> {
        let written = if self.failing.get() {
            Err(WriteFailed)
        } else {
            now(self.storage.write(save)).unwrap();
            Ok(())
        };
        std::future::ready(written)
    }

    async fn rename(&self, key: CompetitionKey, name: &str) -> Result<(), WriteFailed> {
        now(self.storage.rename(key, name)).unwrap();
        Ok(())
    }

    async fn delete(&self, key: CompetitionKey) -> Result<(), WriteFailed> {
        now(self.storage.delete(key)).unwrap();
        Ok(())
    }
}

/// A competition renamed keeps its name through the saves of a session holding the old one, and
/// one deleted is gone.
#[test]
fn a_competition_stored_is_renamed_and_deleted() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    let mut other = Session::new("league");
    now(session.save(&storage)).unwrap();
    now(other.save(&storage)).unwrap();

    now(storage.rename(session.key(), "Cup 2026")).unwrap();
    session.add_entrant("Alice").unwrap();
    now(session.save(&storage)).unwrap();
    now(storage.delete(other.key())).unwrap();

    let listings = now(storage.list()).unwrap();
    assert_eq!(listings.len(), 1);
    assert_eq!(listings[0].name, "Cup 2026");
    assert_eq!(
        now(Session::load(&storage, session.key())).unwrap().name(),
        "Cup 2026"
    );
    assert!(now(storage.read(other.key())).unwrap().is_none());
}

#[test]
fn a_failed_save_leaves_the_changes_to_the_next_one() {
    let storage = Flaky::default();
    let mut session = Session::new("cup");
    now(session.save(&storage)).unwrap();
    session.add_entrant("Alice").unwrap();

    storage.failing.set(true);
    assert!(now(session.save(&storage)).is_err());
    assert!(session.has_unsaved_changes());
    storage.failing.set(false);
    session.add_entrant("Bob").unwrap();
    now(session.save(&storage)).unwrap();

    let loaded = now(Session::load(&storage, session.key())).unwrap();
    assert_eq!(names(&loaded.entrants()), ["Alice", "Bob"]);
}

/// The log knows the entrants by id only, so that their names can be deleted without touching the
/// log.
#[test]
fn names_are_stored_apart_from_the_log() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();

    now(session.save(&storage)).unwrap();

    let record = record(&storage, session.key());
    assert!(!record["log"].to_string().contains("Alice"));
    assert!(record["people"].to_string().contains("Alice"));
}

#[test]
fn an_exported_competition_is_imported_as_a_new_one_as_it_was() {
    let storage = MemoryStorage::default();
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    let mut imported = Session::import(&session.export(), "other", &[]).unwrap();

    assert_eq!(imported.name(), "cup", "the file names the competition");
    assert_ne!(imported.key(), session.key());
    assert_eq!(imported.entrants(), session.entrants());
    assert_eq!(matches(&imported), matches(&session));
    assert_eq!(imported.standings(), session.standings());
    assert!(matches!(imported.undo(), Err(UndoError::NothingToUndo)));
    assert!(imported.has_unsaved_changes(), "nothing of it is saved yet");
    now(imported.save(&storage)).unwrap();
    let loaded = now(Session::load(&storage, imported.key())).unwrap();
    assert_eq!(loaded.entrants(), session.entrants());
    assert_eq!(matches(&loaded), matches(&session));
}

/// For a version that can no longer replay its log.
#[test]
fn an_exported_competition_holds_what_its_log_replays_into() {
    let mut session = Given::new().started();
    session
        .report(numbered(&session, 1), MatchResult::AWins)
        .unwrap();

    let file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();

    assert_eq!(
        file["competition"],
        serde_json::to_value(session.competition()).unwrap()
    );
}

/// The log is what is imported: the competition is only read by a version that cannot replay it.
#[test]
fn a_file_is_imported_from_its_log_whatever_its_competition_holds() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();

    for competition in [
        None,
        Some(serde_json::json!({ "of a later version": true })),
    ] {
        match competition {
            None => file.as_object_mut().unwrap().remove("competition"),
            Some(competition) => file
                .as_object_mut()
                .unwrap()
                .insert("competition".to_owned(), competition),
        };

        let imported = Session::import(&file.to_string(), "cup", &[]).unwrap();

        assert_eq!(imported.entrants(), session.entrants());
    }
}

/// The CLI saves the same file without the name.
#[test]
fn a_file_without_a_name_is_imported_under_the_name_given() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    file.as_object_mut().unwrap().remove("name");

    let imported = Session::import(&file.to_string(), "cup.json", &[]).unwrap();

    assert_eq!(imported.name(), "cup.json");
    assert_eq!(imported.entrants(), session.entrants());
}

/// A copy of a competition stored already is told apart from it in the list.
#[test]
fn a_competition_imported_under_a_name_stored_already_is_numbered() {
    let session = Given::new().added();
    let stored = |name: &str| Listing {
        key: CompetitionKey::new(Uuid::new_v4()),
        name: name.to_owned(),
        progress: None,
    };

    let imported =
        Session::import(&session.export(), "", &[stored("cup"), stored("cup (2)")]).unwrap();

    assert_eq!(imported.name(), "cup (3)");
}

/// Files were written without their format before it was added.
#[test]
fn a_file_without_a_format_is_imported_as_of_the_first() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    assert_eq!(
        file["format"],
        serde_json::to_value(file::Format::CURRENT).unwrap()
    );
    file.as_object_mut().unwrap().remove("format");

    let imported = Session::import(&file.to_string(), "cup", &[]).unwrap();

    assert_eq!(imported.entrants(), session.entrants());
}

#[test]
fn a_file_of_a_newer_format_is_not_imported_for_its_format() {
    let session = Given::new().added();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    let newer = file["format"].as_u64().unwrap() + 1;
    file["format"] = newer.into();
    // what a newer format might hold instead of a log
    file["log"] = serde_json::json!({ "entries": [] });

    let result = Session::import(&file.to_string(), "cup", &[]);

    assert!(
        matches!(
            result,
            Err(ImportError::Newer(format)) if format.to_string() == newer.to_string()
        ),
        "{:?}",
        result.err()
    );
}

/// A file that is no JSON, or no object with a log and people, is no competition at all.
#[test]
fn a_file_that_is_no_competition_is_not_imported() {
    for file in [
        "",
        "<html>",
        "[1, 2]",
        r#"{"people": []}"#,
        r#"{"format": "x"}"#,
    ] {
        let result = Session::import(file, "cup", &[]);

        assert!(
            matches!(result, Err(ImportError::NoCompetition(_))),
            "{file}: {:?}",
            result.err()
        );
    }
}

/// A file with a log and people that do not load is a competition, but damaged.
#[test]
fn a_damaged_competition_is_not_imported() {
    let session = Given::new().started();
    let mut file: serde_json::Value = serde_json::from_str(&session.export()).unwrap();
    file["log"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({}));

    for file in [
        file.to_string(),
        r#"{"log": [{}], "people": []}"#.to_owned(),
    ] {
        let result = Session::import(&file, "cup", &[]);

        assert!(
            matches!(result, Err(ImportError::Damaged(_))),
            "{:?}",
            result.err()
        );
    }
}

/// A log written by a version with a plugin this one does not have, say, cannot be loaded, but the
/// operator can still keep a copy of it.
#[test]
fn what_is_stored_is_exported_whether_it_replays_or_not() {
    let storage = MemoryStorage::default();
    let mut session = Session::new("cup");
    session.add_entrant("Alice").unwrap();
    now(session.save(&storage)).unwrap();
    let mut record = record(&storage, session.key());
    record["log"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "unknown": true }));
    storage.set_record(session.key(), record.to_string());
    assert!(now(Session::load(&storage, session.key())).is_err());

    let file = now(storage.read(session.key())).unwrap().unwrap().export();

    let exported: serde_json::Value = serde_json::from_str(&file).unwrap();
    assert_eq!(exported["name"], "cup");
    assert_eq!(exported["log"].as_array().unwrap().len(), 3);
    assert_eq!(exported["log"][2], serde_json::json!({ "unknown": true }));
    assert_eq!(exported["people"][0]["name"], "Alice");
    assert_eq!(
        exported["competition"],
        serde_json::to_value(session.competition()).unwrap(),
        "what the log replayed into in the version that wrote it"
    );
    assert!(Session::import(&file, "cup", &[]).is_err());
}

#[test]
fn what_does_not_replay_into_a_competition_is_not_loaded() {
    let storage = MemoryStorage::default();
    let session = Session::new("cup");
    let key = session.key();
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::NotFound)
    ));
    now(storage.write(&session.to_save())).unwrap();
    let write = |log: serde_json::Value| {
        let mut record = record(&storage, key);
        record["log"] = log;
        storage.set_record(key, record.to_string());
    };

    write(serde_json::json!([{}]));
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::Damaged(_))
    ));
    let seed = serde_json::to_value(&session.machine.log()[0]).unwrap();
    let undo_of_nothing = serde_json::json!({
        "id": Uuid::from_u128(2),
        "kind": { "Undo": Uuid::from_u128(3) },
    });
    write(serde_json::json!([seed, undo_of_nothing]));
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::Replay(_))
    ));
    storage.set_record(key, "no file".to_owned());
    assert!(matches!(
        now(Session::load(&storage, key)),
        Err(LoadError::Damaged(_))
    ));
}
