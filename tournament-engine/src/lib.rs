pub mod entities;
pub mod map;
pub mod plugins;
pub mod score;
pub mod transitions;
pub mod type_map;

#[cfg(feature = "test-util")]
pub use entities::DivisionBuilder;
pub use entities::{
    Annotated, Bye, ByeId, Competition, Division, DivisionData, DivisionView, Entrant, EntrantId,
    Match, MatchId, MatchResult, NewMatch, Origin, Phase, PhaseId, Placed, Qualifier, Ranking,
    RankingId, ResultOfTeam, Seed, Source, SourceOf, Team,
    columns::{Columns, Shown, Table},
    details::{Decision, EnteredValue, FieldKey, Part, ResultReason, Sides, Values},
    standings::{PhaseStanding, PhaseStandings, Standing, Standings},
};
pub use plugins::PhasePlugins;
#[cfg(feature = "test-util")]
pub use plugins::PhasePluginsBuilder;
pub use score::Score;
pub use transitions::{
    AddEntrant, AddPhase, CompetitionStateMachine, CompetitionStateMachineExt,
    CompetitionTransition, EntryMeta, RemoveEntrant, ReportMatchResult, SetCompetitionSeed,
    StartCompetition, StartPhase, Timestamp,
};
pub use type_map::{TypeMap, TypeMapValue};
// the crates the API of this one is written in, so that a user does not have to depend on them
pub use {plugin_factory, undoable_state_machine, uuid, vec1};

/// What the macros of this crate generate refers to. Not part of the API.
#[doc(hidden)]
pub mod __private {
    // so that a crate declaring a metric does not have to depend on it
    pub use inventory;
}
