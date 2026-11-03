//! A competition as a file, to keep a copy of it or to move it elsewhere: the JSON of its
//! [`Format`], its name, its log, its people and the competition the log replays into. The CLI
//! saves the same, without the name, so each opens the other's files.
//!
//! A session is exported by [`Session::export`](crate::session::Session::export) and imported by
//! [`Session::import`](crate::session::Session::import); what is stored of a competition is
//! exported by [`Stored::export`] even if it no longer loads.

use std::fmt::{self, Display};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{DeserializeOwned, Error as _, IgnoredAny},
};
use serde_json::Value;

use crate::{error::ImportError, storage::Stored};

/// The version of the format of a file, its `"format"`, raised with every change an older version
/// cannot read. A file without one is of the first.
///
/// Deserializing refuses a format newer than [`Format::CURRENT`] (or 0), so that a file of a newer
/// version is told apart from a broken one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Format(u32);

impl Format {
    /// The first format, that of a file without one.
    pub const FIRST: Self = Self(1);
    /// The format this version writes, and the newest it reads.
    pub const CURRENT: Self = Self(1);
}

impl Default for Format {
    /// [`Format::FIRST`], the format of a file without one.
    fn default() -> Self {
        Self::FIRST
    }
}

impl Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for Format {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let format = Self(u32::deserialize(deserializer)?);
        if format < Self::FIRST || format > Self::CURRENT {
            return Err(if Self::FIRST == Self::CURRENT {
                D::Error::custom(format_args!(
                    "the file is of format {format}, this version reads format {} only",
                    Self::CURRENT
                ))
            } else {
                D::Error::custom(format_args!(
                    "the file is of format {format}, this version reads formats {} to {}",
                    Self::FIRST,
                    Self::CURRENT
                ))
            });
        }
        Ok(format)
    }
}

/// What a file holds: the log `L` and the people `P`, as the CLI saves them, plus the name and the
/// competition `C` the log replays into. Reading skips the competition unless `C` says otherwise.
#[derive(Serialize, Deserialize)]
pub struct File<L, P, C = IgnoredAny> {
    // first, so that a file of a newer format is refused for that before its log is read
    #[serde(default)]
    pub format: Format,
    /// Missing in a file of the CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub log: L,
    pub people: P,
    /// What the log replayed into when the file was written, for a version that can no longer
    /// replay it. Missing in files of older versions, which ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competition: Option<C>,
}

impl<L: DeserializeOwned, P: DeserializeOwned> File<L, P> {
    /// The file whose JSON is `file`.
    ///
    /// # Errors
    /// Returns [`ImportError::NoCompetition`] if it is no JSON object with a log and people and,
    /// if any, a format, [`ImportError::Newer`] if its format is newer than [`Format::CURRENT`],
    /// and [`ImportError::Damaged`] if its log or people do not load.
    pub(crate) fn read(file: &str) -> Result<Self, ImportError> {
        let value: Value = serde_json::from_str(file).map_err(ImportError::NoCompetition)?;
        let no_competition = || {
            ImportError::NoCompetition(serde_json::Error::custom(
                "the file holds no object with a log and people",
            ))
        };
        let object = value.as_object().ok_or_else(no_competition)?;
        if let Some(format) = object.get("format") {
            let format = format
                .as_u64()
                .and_then(|format| u32::try_from(format).ok())
                .map(Format)
                .filter(|format| *format >= Format::FIRST)
                .ok_or_else(no_competition)?;
            if format > Format::CURRENT {
                return Err(ImportError::Newer(format));
            }
        }
        if !object.contains_key("log") || !object.contains_key("people") {
            return Err(no_competition());
        }
        serde_json::from_value(value).map_err(ImportError::Damaged)
    }
}

impl Stored {
    /// The file of the competition stored as `self`, whether or not its log still replays: what is
    /// stored is written out as it is.
    ///
    /// # Panics
    /// Panics if the file does not serialize as JSON, which it always does.
    pub fn export(&self) -> String {
        // a record that is no JSON, which no session writes, is kept as the text it is
        let values = |records: &[String]| -> Vec<Value> {
            records
                .iter()
                .map(|record| {
                    serde_json::from_str(record).unwrap_or_else(|_| Value::String(record.clone()))
                })
                .collect()
        };
        to_json(&File::<_, _, Value> {
            format: Format::CURRENT,
            name: Some(self.name.clone()),
            log: values(&self.entries),
            people: values(&self.people),
            competition: None,
        })
    }
}

/// The JSON of `file`, indented to be readable.
pub(crate) fn to_json<L: Serialize, P: Serialize, C: Serialize>(file: &File<L, P, C>) -> String {
    serde_json::to_string_pretty(file).expect("a competition's file serializes as JSON")
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// While this version reads one format only, it says so rather than "formats 1 to 1".
    #[test]
    fn a_newer_format_is_refused_naming_the_one_read() {
        let error = serde_json::from_str::<Format>("2").unwrap_err();

        assert_eq!(
            error.to_string(),
            "the file is of format 2, this version reads format 1 only"
        );
    }
}
