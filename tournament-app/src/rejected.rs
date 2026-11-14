//! What an operator typed into a field of a form that the field cannot take.

use crate::i18n::Text;

/// What was typed into fields of a form and not taken, each by the key `K` of its field. It stays
/// shown with why until the field is entered again, and while there is any, the form is not
/// submitted.
#[derive(Debug, Clone)]
pub(crate) struct Rejections<K> {
    rejected: Vec<(K, Rejected)>,
}

/// What was typed into a field and not taken, with why.
#[derive(Debug, Clone)]
pub(crate) struct Rejected {
    pub(crate) entered: String,
    pub(crate) problem: Text,
}

impl<K> Default for Rejections<K> {
    fn default() -> Self {
        Self {
            rejected: Vec::new(),
        }
    }
}

impl<K: PartialEq> Rejections<K> {
    /// Notes that the field `key` did not take `entered`, for `problem`, in place of what it did
    /// not take before.
    pub(crate) fn reject(&mut self, key: K, entered: &str, problem: Text) {
        self.forget(&key);
        self.rejected.push((
            key,
            Rejected {
                entered: entered.to_owned(),
                problem,
            },
        ));
    }

    /// Forgets what the field `key` did not take, e.g. once it is entered again.
    pub(crate) fn forget(&mut self, key: &K) {
        self.retain(|rejected| rejected != key);
    }

    /// Keeps what fields did not take only for the keys `keep` keeps.
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        self.rejected.retain(|(key, _)| keep(key));
    }

    /// Forgets everything.
    pub(crate) fn clear(&mut self) {
        self.rejected.clear();
    }

    /// What the field `key` did not take, if anything.
    pub(crate) fn get(&self, key: &K) -> Option<&Rejected> {
        self.rejected
            .iter()
            .find(|(rejected, _)| rejected == key)
            .map(|(_, rejected)| rejected)
    }

    /// Whether the field `key` did not take something.
    pub(crate) fn contains(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    /// Whether every field took what was typed into it, so the form may be submitted.
    pub(crate) fn is_empty(&self) -> bool {
        self.rejected.is_empty()
    }

    /// What fields did not take, by their keys, in the order it was typed.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &Rejected)> {
        self.rejected.iter().map(|(key, rejected)| (key, rejected))
    }
}
