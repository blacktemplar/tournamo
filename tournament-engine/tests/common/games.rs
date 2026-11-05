//! Match rules of games, standing in for those of a third party: who won how many games, each of
//! up to three games scored in points.

use serde::{Deserialize, Serialize};
use tournament_engine::{
    AddEntrant, AddPhase, CompetitionStateMachine, CompetitionStateMachineExt, Entrant, EntrantId,
    FieldKey, Match, MatchId, MatchResult, PhaseData, PhaseId, PhasePluginsBuilder, ResultReason,
    Seed, SetCompetitionSeed, Sides, StartCompetition, TypeMap, TypeMapValue, Value,
    plugin_factory::{Plugin, compatibility::Declaration, register},
    plugins::{
        Phrase,
        facts::MatchTally,
        match_rules::{
            FieldSpec, Hints, Level, MatchRules, Parts, ReasonSpec, Template, ValueKind,
        },
    },
    uuid::Uuid,
};

/// The games each side won.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamesWon(pub Sides<u32>);

#[typetag::serde(name = "test::games_won")]
impl TypeMapValue for GamesWon {}

/// Rules entering the games won by each side, with the points of each of up to three games, and
/// tallying the games each side won.
#[derive(Debug, Clone, Serialize, Deserialize, Plugin)]
#[plugin(id = "test::games", label = "games", declare_static = Self::declare_static)]
pub struct Games;

impl Games {
    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(MatchTally::<GamesWon>::new());
    }
}

#[register]
impl MatchRules for Games {
    fn template(&self, _: &PhaseData, _: &Match) -> Template {
        let points = FieldSpec {
            key: FieldKey::new("points"),
            label: Phrase::new("points", "points"),
            kind: ValueKind::Count,
            hints: Hints::default(),
        };
        let game = Level {
            fields: vec![points],
            ..Level::new(Phrase::new("game", "game"))
        };
        Template {
            root: Level {
                fields: vec![FieldSpec {
                    key: FieldKey::new("games"),
                    label: Phrase::new("games", "games"),
                    kind: ValueKind::Count,
                    hints: Hints::default(),
                }],
                reasons: vec![ReasonSpec {
                    reason: ResultReason::new("retired"),
                    label: Phrase::new("retired", "retired"),
                }],
                parts: Parts::Same {
                    level: Box::new(game),
                    max: 3,
                },
                ..Level::new(Phrase::new("match", "match"))
            },
        }
    }

    fn tally(&self, _: &PhaseData, r#match: &Match) -> TypeMap {
        let mut won = Sides::new(0, 0);
        for game in r#match.contest().parts() {
            match game.decision().map(|decision| decision.result()) {
                Some(MatchResult::AWins) => won.a += 1,
                Some(MatchResult::BWins) => won.b += 1,
                _ => {}
            }
        }
        let mut tallies = TypeMap::new();
        tallies.insert(GamesWon(won));
        tallies
    }
}

pub const SEED: Seed = Seed::new(*b"games!!!!!!!");

pub fn phase_id() -> PhaseId {
    PhaseId::new(Uuid::from_u128(0x9a3e5))
}

/// A competition of two entrants whose single match is open, its results entered by [`Games`].
pub fn competition_entering_games() -> (CompetitionStateMachine, MatchId) {
    let mut machine = CompetitionStateMachine::default();
    machine
        .apply_transition(SetCompetitionSeed::new(SEED))
        .unwrap();
    for entrant in 0..2 {
        machine
            .apply_transition(AddEntrant::new(Entrant::new(EntrantId::new(
                Uuid::from_u128(entrant),
            ))))
            .unwrap();
    }
    let plugins = PhasePluginsBuilder::new().results(Games).build().unwrap();
    machine
        .apply_transition(AddPhase::new(phase_id(), plugins))
        .unwrap();
    machine.apply_transition(StartCompetition).unwrap();
    let open = machine.current().phases()[&phase_id()]
        .data()
        .matches()
        .values()
        .next()
        .unwrap()
        .id();
    (machine, open)
}

/// A count for each side.
pub fn count(a: u32, b: u32) -> Sides<Option<Value>> {
    Sides::new(Some(Value::Count(a)), Some(Value::Count(b)))
}
