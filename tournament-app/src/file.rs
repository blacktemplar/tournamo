//! A competition as a file, to keep a copy of it or to move it elsewhere: the JSON of its
//! [`Format`], its name, its log, its people and the competition the log replays into. The CLI
//! saves the same, without the name, so each opens the other's files. A storage holds it too,
//! without the name, which the listing holds.
//!
//! A session is exported by [`Session::export`](crate::session::Session::export) and imported by
//! [`Session::import`](crate::session::Session::import); what is stored of a competition is
//! exported by [`Stored::export`] even if it no longer loads.

use std::fmt::{self, Display};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeOwned, Error as _, IgnoredAny, MapAccess},
};
use serde_json::Value;

pub use self::convert::upgrade_competition;
use crate::{error::ImportError, storage::Stored};

/// The version of the format of a file, its `"format"`, raised with every change of what is saved
/// that needs a converter or that an older version cannot read. A file without one is of the first.
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
    pub const CURRENT: Self = Self(2);
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
    /// The file whose JSON is `file`, read as of [`Format::CURRENT`]: a file of an older format is
    /// converted first.
    ///
    /// # Errors
    /// Returns [`ImportError::NoCompetition`] if it is no JSON object with a log and people and,
    /// if any, a format, [`ImportError::Newer`] if its format is newer than [`Format::CURRENT`],
    /// and [`ImportError::Damaged`] if its log or people do not load.
    pub fn read(file: &str) -> Result<Self, ImportError> {
        // checked before reading the rest, which may take long, so as to tell a file of a newer
        // version or none at all from a damaged one
        let head: Head = serde_json::from_str(file).map_err(ImportError::NoCompetition)?;
        let no_competition = || {
            ImportError::NoCompetition(serde_json::Error::custom(
                "the file holds no object with a log and people",
            ))
        };
        let format = match head.format {
            Some(format) => format
                .as_u64()
                .and_then(|format| u32::try_from(format).ok())
                .map(Format)
                .filter(|format| *format >= Format::FIRST)
                .ok_or_else(no_competition)?,
            None => Format::FIRST,
        };
        if format > Format::CURRENT {
            return Err(ImportError::Newer(format));
        }
        if !head.log || !head.people {
            return Err(no_competition());
        }
        if format == Format::CURRENT {
            return serde_json::from_str(file).map_err(ImportError::Damaged);
        }
        let mut file = serde_json::from_str(file).map_err(ImportError::Damaged)?;
        convert::upgrade(&mut file, format);
        serde_json::from_value(file).map_err(ImportError::Damaged)
    }
}

/// What [`File::read`] checks of a JSON object before reading it: its format, if any, and whether
/// it has a log and people.
struct Head {
    format: Option<Value>,
    log: bool,
    people: bool,
}

impl<'de> Deserialize<'de> for Head {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Head;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Head, A::Error> {
                let mut head = Head {
                    format: None,
                    log: false,
                    people: false,
                };
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "format" => head.format = Some(map.next_value()?),
                        "log" => {
                            map.next_value::<IgnoredAny>()?;
                            head.log = true;
                        }
                        "people" => {
                            map.next_value::<IgnoredAny>()?;
                            head.people = true;
                        }
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(head)
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

impl Stored {
    /// The file of the competition stored as `self`, whether or not its log still replays: what is
    /// stored is written out as it is, with the name.
    pub fn export(&self) -> String {
        match serde_json::from_str::<File<Value, Value, Value>>(&self.record) {
            Ok(file) => to_json(&File {
                name: Some(self.name.clone()),
                ..file
            }),
            // no file, which no session stores, or one of a newer format, is kept as it is
            Err(_) => self.record.clone(),
        }
    }
}

/// The record `file` is stored as, see [`crate::storage`]: its JSON, without the name, which the
/// listing holds.
pub(crate) fn to_record<L: Serialize, P: Serialize, C: Serialize>(file: File<L, P, C>) -> String {
    serde_json::to_string(&File { name: None, ..file })
        .expect("a competition's file serializes as JSON")
}

/// The JSON of `file`, indented to be readable.
pub(crate) fn to_json<L: Serialize, P: Serialize, C: Serialize>(file: &File<L, P, C>) -> String {
    serde_json::to_string_pretty(file).expect("a competition's file serializes as JSON")
}

mod convert;

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn a_newer_format_is_refused_naming_the_ones_read() {
        let error = serde_json::from_str::<Format>("3").unwrap_err();

        assert_eq!(
            error.to_string(),
            "the file is of format 3, this version reads formats 1 to 2"
        );
    }
}
