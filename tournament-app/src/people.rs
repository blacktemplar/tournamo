//! The personal data of the entrants of a competition, such as the names they are shown by.
//!
//! It is kept apart from the competition, whose transition log holds only the ids of the entrants:
//! the log has to replay as it was written, whereas personal data can be deleted.

use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};
use tournament_engine::{
    EntrantId, Score,
    map::{IdAble, IdMap},
};

use crate::i18n::Language;

/// Who an entrant is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    id: EntrantId,
    name: String,
    rating: Option<Rating>,
}

impl Person {
    /// The person `id` is the entrant of, shown and addressed by `name`, without a rating.
    pub fn new(id: EntrantId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            rating: None,
        }
    }

    /// This person, rated `rating`.
    pub fn with_rating(mut self, rating: Option<Rating>) -> Self {
        self.rating = rating;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// How strong the entrant is rated, higher is better; the registration is ordered by it.
    pub fn rating(&self) -> Option<Rating> {
        self.rating
    }
}

/// How strong an entrant is rated, higher is better, e.g. an Elo rating: a finite number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rating(Score);

impl Rating {
    /// The rating `text` says as an operator in `language` types it, spaces around it ignored: a
    /// number, perhaps negative, grouped by thousands or with a fraction, e.g. `1500`, `1.800,5`
    /// in German or `-5`, but no exponent.
    pub fn parse(text: &str, language: Language) -> Option<Self> {
        crate::number::read(text, language, true)?
            .plain()
            .parse()
            .ok()
            .map(Self)
    }

    pub fn get(self) -> f64 {
        self.0.get()
    }
}

/// Shows the rating as its number, e.g. `1500` or `7.5`.
impl Display for Rating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl IdAble for Person {
    type Id = EntrantId;

    fn id(&self) -> EntrantId {
        self.id
    }
}

/// The people of a competition, by the id of the entrant each of them is.
///
/// It serializes as the list of the people in the order they were added.
///
/// # Examples
/// ```
/// use tournament_app::people::{People, Person};
/// use tournament_engine::{EntrantId, uuid::Uuid};
///
/// let alice = EntrantId::new(Uuid::from_u128(1));
/// let mut people = People::default();
/// people.add(Person::new(alice, "Alice"));
///
/// assert_eq!(people.name(&alice), Some("Alice"));
/// assert_eq!(people.name(&EntrantId::new(Uuid::from_u128(2))), None);
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct People(IdMap<Person>);

impl People {
    /// Adds `person`, replacing whoever was the same entrant before.
    pub fn add(&mut self, person: Person) {
        self.0.insert(person);
    }

    /// The person the entrant `id` is, if this knows who it is.
    pub fn get(&self, id: &EntrantId) -> Option<&Person> {
        self.0.get(id)
    }

    /// The name of the entrant `id`, if this knows who it is.
    pub fn name(&self, id: &EntrantId) -> Option<&str> {
        self.get(id).map(Person::name)
    }

    /// Rates the entrant `id` `rating`, or unrates it if `None`. Returns whether this knows who it
    /// is; if not, nothing changes.
    pub fn set_rating(&mut self, id: &EntrantId, rating: Option<Rating>) -> bool {
        self.0
            .get_mut(id)
            .map(|person| person.rating = rating)
            .is_some()
    }

    /// The rating of the entrant `id`, if this knows one.
    pub fn rating(&self, id: &EntrantId) -> Option<Rating> {
        self.get(id).and_then(Person::rating)
    }

    /// Everyone, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &Person> {
        self.0.values()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tournament_engine::uuid::Uuid;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn people_serialize_as_a_list_in_the_order_they_were_added() {
        let mut people = People::default();
        people.add(Person::new(EntrantId::new(Uuid::from_u128(2)), "Bob"));
        people.add(Person::new(EntrantId::new(Uuid::from_u128(1)), "Alice"));

        let value = serde_json::to_value(&people).unwrap();

        assert_eq!(
            value,
            json!([
                { "id": "00000000-0000-0000-0000-000000000002", "name": "Bob", "rating": null },
                { "id": "00000000-0000-0000-0000-000000000001", "name": "Alice", "rating": null },
            ])
        );
        assert_eq!(serde_json::from_value::<People>(value).unwrap(), people);
    }

    #[test]
    fn a_rating_serializes_as_its_number() {
        let person = Person::new(EntrantId::new(Uuid::from_u128(1)), "Alice")
            .with_rating(Rating::parse("1500", Language::English));

        let value = serde_json::to_value(&person).unwrap();

        assert_eq!(value["rating"], json!(1500.0));
        assert_eq!(serde_json::from_value::<Person>(value).unwrap(), person);
    }

    #[test]
    fn a_rating_is_read_as_its_language_writes_numbers() {
        let parse = |text, language| Rating::parse(text, language).map(Rating::get);
        assert_eq!(parse(" 1500 ", Language::English), Some(1500.0));
        assert_eq!(parse("7,5", Language::English), Some(7.5));
        assert_eq!(parse("7.5", Language::German), Some(7.5));
        assert_eq!(parse("1.800", Language::German), Some(1800.0));
        assert_eq!(parse("1,800", Language::English), Some(1800.0));
        assert_eq!(parse("-3", Language::English), Some(-3.0));
        for not_a_rating in ["", "1e3", "abc", "1 500", "1.2.3", &"9".repeat(400)] {
            assert_eq!(
                Rating::parse(not_a_rating, Language::English),
                None,
                "{not_a_rating:?}"
            );
        }
    }

    #[test]
    fn adding_the_same_entrant_again_renames_it() {
        let id = EntrantId::new(Uuid::from_u128(1));
        let mut people = People::default();
        people.add(Person::new(id, "Alice"));

        people.add(Person::new(id, "Alicia"));

        assert_eq!(people.name(&id), Some("Alicia"));
    }
}
