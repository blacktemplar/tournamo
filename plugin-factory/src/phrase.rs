use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

/// Something a plugin says for a frontend to show, e.g. the name of a stage, the text of a metric,
/// or a fact in a message about [compatibility](crate::compatibility). Shows as what it says in
/// English.
///
/// A translation finds it by its [key](Self::key), among the phrases of its plugin of the same
/// kind or among those of the facts, and the numbers it has.
///
/// # Examples
/// ```
/// # use plugin_factory::Phrase;
/// let phrase = Phrase::new("winners-round", "Winners round 2").with("round", 2);
/// assert_eq!(phrase.to_string(), "Winners round 2");
/// assert_eq!(phrase.numbers(), [("round", 2)]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Phrase {
    key: &'static str,
    numbers: Vec<(&'static str, u32)>,
    english: Cow<'static, str>,
}

impl Phrase {
    /// The phrase `key`, which names it among the phrases of its plugin of the same kind, or among
    /// those of the facts, whatever its numbers, e.g. `"final"`, and says `english` in English.
    pub fn new(key: &'static str, english: impl Into<Cow<'static, str>>) -> Self {
        Self {
            key,
            numbers: Vec::new(),
            english: english.into(),
        }
    }

    /// This phrase with the number `name`, which it says, set to `value`.
    #[must_use]
    pub fn with(mut self, name: &'static str, value: u32) -> Self {
        self.numbers.push((name, value));
        self
    }

    /// What names it among the phrases of its plugin of the same kind, or among those of the
    /// facts, whatever its numbers.
    pub fn key(&self) -> &'static str {
        self.key
    }

    /// The numbers it says, each by its name, in the order they were set.
    pub fn numbers(&self) -> &[(&'static str, u32)] {
        &self.numbers
    }
}

impl Display for Phrase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.english)
    }
}
