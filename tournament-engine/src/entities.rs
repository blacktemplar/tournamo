mod annotated;
mod bye;
mod competition;
pub mod details;
mod entrant;
mod ids;
mod r#match;
mod phase;
mod ranking;
mod seed;
pub mod standings;

pub use annotated::Annotated;
pub use bye::Bye;
pub use competition::Competition;
pub use entrant::Entrant;
pub use ids::{ByeId, EntrantId, MatchId, PhaseId, RankingId};
pub use r#match::{Match, MatchResult, NewMatch, ResultOfTeam, Team};
#[cfg(feature = "test-util")]
pub use phase::builder::PhaseBuilder;
pub use phase::{Phase, PhaseData, PhaseView};
pub use ranking::Ranking;
pub use seed::Seed;
