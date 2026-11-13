use std::num::NonZeroU32;

use itertools::Either;
use serde::{Deserialize, Serialize};
use vec1::Vec1;

#[cfg(doc)]
use crate::plugins::{
    match_format::MatchFormat,
    phase_format::{PhaseFormat, Schedule},
};
use crate::{
    Annotated, Decision, MatchId, Part, RankingId,
    map::IdAble,
    type_map::{TypeMap, TypeMapValue},
};

/// The result of a finished match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatchResult {
    AWins,
    BWins,
    Draw,
}

impl MatchResult {
    /// The team that won, or `None` for a draw.
    pub fn winner(self) -> Option<Team> {
        match self {
            MatchResult::AWins => Some(Team::A),
            MatchResult::BWins => Some(Team::B),
            MatchResult::Draw => None,
        }
    }

    /// The team that lost, or `None` for a draw.
    pub fn loser(self) -> Option<Team> {
        self.winner().map(Team::opponent)
    }

    /// What this result is for `team`.
    pub fn for_team(self, team: Team) -> ResultOfTeam {
        match self.winner() {
            Some(winner) if winner == team => ResultOfTeam::Win,
            Some(_) => ResultOfTeam::Loss,
            None => ResultOfTeam::Draw,
        }
    }
}

/// A match between two teams of rankings in a round of a phase, numbered within the phase.
///
/// Beyond who plays it, its round and its number, a match is its [`contest`](Self::contest): how it
/// was decided, once it is finished, what was entered for it, and its `annotations`: data that
/// describes it, keyed by type, such as which stage of a knockout it belongs to. The
/// [`PhaseFormat`] scheduling the match gives them when creating it as a [`NewMatch`], with
/// [`Annotated::with_annotation`].
#[derive(Debug, Serialize, Deserialize)]
pub struct Match {
    id: MatchId,
    team_a: Vec1<RankingId>,
    team_b: Vec1<RankingId>,
    round: NonZeroU32,
    number: NonZeroU32,
    pub(super) contest: Part,
    pub(super) tallies: TypeMap,
}

/// A match a [`PhaseFormat`] schedules, see [`Schedule`]: who plays it in which round, what it is
/// annotated with, and the number it asks for, if any. The phase stores it as a [`Match`], numbered
/// as asked, or else with the number after the highest one its matches have.
#[derive(Debug)]
pub struct NewMatch {
    id: MatchId,
    team_a: Vec1<RankingId>,
    team_b: Vec1<RankingId>,
    round: NonZeroU32,
    number: Option<NonZeroU32>,
    annotations: TypeMap,
}

impl NewMatch {
    /// A match of `team_a` against `team_b` in `round`. Its id is derived from the teams and the
    /// round.
    ///
    /// # Panics
    /// Panics if a ranking is on both teams.
    pub fn new(team_a: Vec1<RankingId>, team_b: Vec1<RankingId>, round: NonZeroU32) -> Self {
        assert!(team_a.iter().all(|id| !team_b.contains(id)));
        let id = MatchId::derive(team_a.iter().chain(team_b.iter()), round);
        Self {
            id,
            team_a,
            team_b,
            round,
            number: None,
            annotations: TypeMap::new(),
        }
    }

    /// Asks for the match to be numbered `number`, which no other match of the phase may have, e.g.
    /// its place in a bracket, numbered before it is scheduled.
    #[must_use]
    pub fn with_number(mut self, number: NonZeroU32) -> Self {
        self.number = Some(number);
        self
    }

    /// The id the match will have, see [`Match::id`].
    pub fn id(&self) -> MatchId {
        self.id
    }

    pub fn round(&self) -> NonZeroU32 {
        self.round
    }

    pub fn rankings(&self) -> impl Iterator<Item = &RankingId> {
        self.team_a.iter().chain(self.team_b.iter())
    }

    /// The number asked for, if any.
    pub fn number(&self) -> Option<NonZeroU32> {
        self.number
    }

    /// The match as the phase stores it, numbered `number`.
    pub(crate) fn numbered(self, number: NonZeroU32) -> Match {
        Match {
            id: self.id,
            team_a: self.team_a,
            team_b: self.team_b,
            round: self.round,
            number,
            contest: Part::new().with_annotations(self.annotations),
            tallies: TypeMap::new(),
        }
    }
}

impl Annotated for NewMatch {
    fn annotations(&self) -> &TypeMap {
        &self.annotations
    }

    fn annotations_mut(&mut self) -> &mut TypeMap {
        &mut self.annotations
    }
}

impl IdAble for Match {
    type Id = MatchId;

    fn id(&self) -> Self::Id {
        self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    A,
    B,
}

impl Team {
    pub fn opponent(self) -> Self {
        match self {
            Team::A => Team::B,
            Team::B => Team::A,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultOfTeam {
    Win,
    Draw,
    Loss,
}

impl Match {
    pub fn team_of(&self, ranking_id: &RankingId) -> Option<Team> {
        if self.team_a().contains(ranking_id) {
            Some(Team::A)
        } else if self.team_b().contains(ranking_id) {
            Some(Team::B)
        } else {
            None
        }
    }

    pub fn id(&self) -> MatchId {
        self.id
    }

    pub fn team_a(&self) -> &Vec1<RankingId> {
        &self.team_a
    }

    pub fn team_b(&self) -> &Vec1<RankingId> {
        &self.team_b
    }

    pub fn rankings_of(&self, team: Team) -> &Vec1<RankingId> {
        match team {
            Team::A => self.team_a(),
            Team::B => self.team_b(),
        }
    }

    /// What the result of this match is for `team`, or `None` if it is not finished.
    pub fn result_of_team(&self, team: Team) -> Option<ResultOfTeam> {
        self.result().map(|result| result.for_team(team))
    }

    pub fn rankings(&self) -> impl Iterator<Item = &RankingId> {
        self.team_a.iter().chain(self.team_b.iter())
    }

    pub fn non_losing_rankings(&self) -> impl Iterator<Item = &RankingId> {
        // technically no team is losing yet if result is not finished
        match self.result().and_then(MatchResult::winner) {
            Some(winner) => Either::Left(self.rankings_of(winner).iter()),
            None => Either::Right(self.rankings()),
        }
    }

    /// The round this match is played in. A ranking plays at most once per round, and its matches
    /// in later rounds are played after those in earlier ones.
    pub fn round(&self) -> NonZeroU32 {
        self.round
    }

    /// Its number, unique within its phase: the one its format asked for, e.g. its place in a
    /// bracket, or else the one after the highest number the phase's matches had when it was
    /// scheduled, so that matches are numbered in the order they are scheduled.
    pub fn number(&self) -> NonZeroU32 {
        self.number
    }

    /// The result of this match, or `None` if it is not finished.
    pub fn result(&self) -> Option<MatchResult> {
        self.decision().map(Decision::result)
    }

    /// How this match was decided, or `None` if it is not finished.
    pub fn decision(&self) -> Option<&Decision> {
        self.contest.decision()
    }

    /// The match as a part: its decision, the values entered for it, its parts and its annotations.
    pub fn contest(&self) -> &Part {
        &self.contest
    }

    pub fn is_finished(&self) -> bool {
        self.decision().is_some()
    }

    /// The tally `T` of this match, see [`MatchFormat::tally`]: `None` while it is not finished, or
    /// if its match format does not tally `T`.
    pub fn tally<T: TypeMapValue>(&self) -> Option<&T> {
        self.tallies.get::<T>()
    }
}

impl Annotated for Match {
    fn annotations(&self) -> &TypeMap {
        self.contest.annotations()
    }

    fn annotations_mut(&mut self) -> &mut TypeMap {
        self.contest.annotations_mut()
    }
}
