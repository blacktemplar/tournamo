//! How many settings of a phase the operator sees: those of the level a competition opens at
//! (see [`Session::opening_level`](tournament_app::session::Session::opening_level)), until the
//! operator chooses another while it is open.

use tournament_engine::plugin_factory::schema::Level;

/// The levels an operator chooses among, in order.
pub const ALL: [Level; 3] = [Level::Basic, Level::Advanced, Level::Expert];

/// The key of the message naming `level`.
pub fn message(level: Level) -> &'static str {
    match level {
        Level::Basic => "level-basic",
        Level::Advanced => "level-advanced",
        Level::Expert => "level-expert",
    }
}
