//! The match format of a match decided by a score, e.g. 21–17 or 3–1.

use plugin_factory::{Plugin, compatibility::Declaration, register};
use serde::{Deserialize, Serialize};

use crate::{
    entities::{
        competition::{Match, MatchResult, PhaseData},
        details::{EnteredValue, FieldKey, Part, Sides},
    },
    plugins::{
        Phrase,
        facts::MatchTally,
        match_format::{FieldSpec, Hints, Level, MatchFormat, PointsScored, Template, ValueKind},
    },
    type_map::TypeMap,
};

/// The field of the score of each side.
pub(crate) const SCORE: FieldKey = FieldKey::new("score");

/// The match format of a match decided by its score: the points or goals each side scored. More
/// wins, the same is a draw.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Plugin)]
#[plugin(
    id = "tournament_engine::scored",
    label = "score",
    description = "the points or goals each side scored",
    declare_static = Self::declare_static
)]
pub struct Scored {
    #[serde(default)]
    #[plugin(
        label = "winning score",
        description = "what a winner usually has, or more, then winning by one or two; any other \
                       score is pointed out, as it may be a typo",
        min = 1,
        default = None
    )]
    winning_score: Option<u16>,
}

impl Scored {
    /// A match format pointing out a score other than one of `winning_score`, if any.
    ///
    /// Requires the `test-util` feature.
    #[cfg(feature = "test-util")]
    pub const fn new(winning_score: Option<u16>) -> Self {
        Self { winning_score }
    }

    fn declare_static(declaration: &mut Declaration) {
        declaration.provide(MatchTally::<PointsScored>::new());
    }
}

/// The score of each side of `part`, a side without one entered scoring none, or `None` if there is
/// no score of either side.
pub(crate) fn score(part: &Part) -> Option<Sides<u32>> {
    let sides = part.value(SCORE.key())?;
    let count = |value: Option<EnteredValue>| match value {
        Some(EnteredValue::Count(count)) => u32::from(count),
        _ => 0,
    };
    (sides.a.is_some() || sides.b.is_some()).then(|| Sides::new(count(sides.a), count(sides.b)))
}

/// The result of a score: more wins, the same is a draw.
pub(crate) fn result_of<T: Ord>(score: Sides<T>) -> MatchResult {
    match score.a.cmp(&score.b) {
        std::cmp::Ordering::Greater => MatchResult::AWins,
        std::cmp::Ordering::Less => MatchResult::BWins,
        std::cmp::Ordering::Equal => MatchResult::Draw,
    }
}

/// The result the score of `part` suggests, once each side has one.
pub(crate) fn suggest_by_score(part: &Part) -> Option<MatchResult> {
    match *part.value(SCORE.key())? {
        Sides {
            a: Some(EnteredValue::Count(a)),
            b: Some(EnteredValue::Count(b)),
        } => Some(result_of(Sides::new(a, b))),
        _ => None,
    }
}

/// The field of a score, pointing out one other than one of `winning_score`.
pub(crate) fn score_field(winning_score: Option<u16>) -> FieldSpec {
    FieldSpec {
        key: SCORE,
        label: Phrase::new("score", "score"),
        kind: ValueKind::Count,
        hints: Hints { winning_score },
    }
}

#[register]
impl MatchFormat for Scored {
    fn template(&self, _: &PhaseData, _: &Match) -> Template {
        Template {
            root: Level {
                fields: vec![score_field(self.winning_score)],
                ..Level::new(Phrase::new("match", "match"))
            },
        }
    }

    fn tally(&self, _: &PhaseData, r#match: &Match) -> TypeMap {
        let mut tallies = TypeMap::new();
        tallies.insert(PointsScored(score(r#match.contest()).unwrap_or_default()));
        tallies
    }

    fn suggest(&self, _: &Level, part: &Part) -> Option<MatchResult> {
        suggest_by_score(part)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;
    use crate::{
        entities::{
            competition::{Phase, PhaseBuilder},
            details::{Decision, Values},
        },
        plugins::metrics_calculator::test_utils::{match_id, phase_with},
    };

    fn count(a: u16, b: u16) -> Sides<Option<EnteredValue>> {
        Sides::new(Some(EnteredValue::Count(a)), Some(EnteredValue::Count(b)))
    }

    fn scored(score: Sides<Option<EnteredValue>>) -> Part {
        Part::new().with_value(SCORE, score)
    }

    /// A phase of two players, with one match in round 1, its results entered by `format`.
    fn phase(format: Scored) -> Phase {
        phase_with(
            PhaseBuilder::new().match_format(format).add_players(2),
            vec![(1, vec![0], vec![1])],
        )
    }

    fn reported(values: Values) -> PointsScored {
        let mut phase = phase(Scored::default());
        let id = match_id(&phase, 1, &[0], &[1]);
        phase.report_match(id, Decision::new(MatchResult::BWins), values, Vec::new());
        *phase.data().matches()[&id].tally::<PointsScored>().unwrap()
    }

    #[test]
    fn a_match_tallies_its_score() {
        assert_eq!(
            reported(vec![(SCORE, count(17, 21))]),
            PointsScored(Sides::new(17, 21))
        );
    }

    #[test]
    fn a_side_without_a_score_scored_none() {
        assert_eq!(
            reported(vec![(
                SCORE,
                Sides::new(None, Some(EnteredValue::Count(3)))
            )]),
            PointsScored(Sides::new(0, 3))
        );
        assert_eq!(reported(Vec::new()), PointsScored(Sides::new(0, 0)));
    }

    #[test]
    fn the_higher_score_suggests_its_side_won_and_the_same_a_draw() {
        let level = Level::new(Phrase::new("match", "match"));
        let suggest = |score| Scored::default().suggest(&level, &scored(score));

        assert_eq!(suggest(count(21, 17)), Some(MatchResult::AWins));
        assert_eq!(suggest(count(0, 2)), Some(MatchResult::BWins));
        assert_eq!(suggest(count(1, 1)), Some(MatchResult::Draw));
        assert_eq!(
            suggest(Sides::new(Some(EnteredValue::Count(1)), None)),
            None
        );
        assert_eq!(
            Scored::default().suggest(&level, &Part::new()),
            None,
            "nothing entered suggests nothing"
        );
    }

    #[test]
    fn the_winning_score_is_hinted_at() {
        let phase = phase(Scored::new(Some(21)));
        let r#match = phase.data().matches().values().next().unwrap();

        let template = phase.match_format().template(phase.data(), r#match);

        assert_eq!(
            template.root.field(&SCORE).unwrap().hints.winning_score,
            Some(21)
        );
    }

    /// The tags are part of every log and snapshot written, see `transition_log_serialization.rs`.
    #[test]
    fn the_rules_and_their_tally_are_serialized_under_their_tags() {
        let format: Box<dyn MatchFormat> = Box::new(Scored::default());
        let mut tallies = TypeMap::new();
        tallies.insert(PointsScored::default());

        assert_eq!(
            serde_json::to_value(&format).unwrap()["type"],
            "tournament_engine::scored"
        );
        assert_eq!(
            serde_json::to_value(&tallies).unwrap()[0]["type"],
            "tournament_engine::points_scored"
        );
    }
}
