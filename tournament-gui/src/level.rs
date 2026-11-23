//! How many settings of a phase the operator sees: those of the level they chose last, which the
//! browser keeps, as it keeps the theme.

use tournament_engine::plugin_factory::schema::Level;

/// The levels an operator chooses among, in order.
pub const ALL: [Level; 3] = [Level::Basic, Level::Advanced, Level::Expert];

/// How local storage names `level`.
fn tag(level: Level) -> &'static str {
    match level {
        Level::Basic => "basic",
        Level::Advanced => "advanced",
        Level::Expert => "expert",
    }
}

/// The key of the message naming `level`.
pub fn message(level: Level) -> &'static str {
    match level {
        Level::Basic => "level-basic",
        Level::Advanced => "level-advanced",
        Level::Expert => "level-expert",
    }
}

/// Where the browser's local storage keeps the level chosen.
const CHOSEN: &str = "tournamo.level";

/// The level chosen last in this browser, or else advanced.
pub fn initial() -> Level {
    local_storage()
        .and_then(|storage| storage.get_item(CHOSEN).ok().flatten())
        .and_then(|chosen| ALL.into_iter().find(|level| tag(*level) == chosen))
        .unwrap_or(Level::Advanced)
}

/// Keeps `level` as the one chosen in this browser.
pub fn remember(level: Level) {
    // without local storage, the choice lasts until the page is left
    if let Some(storage) = local_storage() {
        _ = storage.set_item(CHOSEN, tag(level));
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
