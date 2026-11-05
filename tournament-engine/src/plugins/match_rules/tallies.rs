//! The tallies of the engine's match rules, see [`MatchRules::tally`](super::MatchRules::tally).

use serde::{Deserialize, Serialize};

use crate::{entities::details::Sides, type_map::TypeMapValue};

/// The points, or goals, each side of a match scored: in all its parts, if it is played in parts.
/// A side without a score entered scored none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PointsScored(pub Sides<u32>);

#[typetag::serde(name = "tournament_engine::points_scored")]
impl TypeMapValue for PointsScored {}

/// The games each side of a match won.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GamesWon(pub Sides<u32>);

#[typetag::serde(name = "tournament_engine::games_won")]
impl TypeMapValue for GamesWon {}
