//! Numbers as an operator types them, in the way their language writes them.

use crate::i18n::Language;

/// A number as typed: its sign, the digits of its whole part, and those of its fraction if it has
/// one, see [`read`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Typed {
    pub(crate) negative: bool,
    pub(crate) whole: String,
    pub(crate) fraction: Option<String>,
}

impl Typed {
    /// The number as Rust reads it, e.g. `-1800.5`.
    pub(crate) fn plain(&self) -> String {
        let sign = if self.negative { "-" } else { "" };
        match &self.fraction {
            Some(fraction) => format!("{sign}{}.{fraction}", self.whole),
            None => format!("{sign}{}", self.whole),
        }
    }

    /// The number, which is infinite if it is too large for a float.
    pub(crate) fn get(&self) -> f64 {
        self.plain()
            .parse()
            .expect("digits with a point are a float")
    }
}

/// The number `text` is, as an operator in `language` types it, spaces around it ignored: digits,
/// perhaps grouped by thousands and with a fraction, preceded by a `-` (or `−`) if `signed`.
///
/// The language's decimal separator (a point in English, a comma in German) is always one; its
/// separator of thousands (a comma in English, a point in German) groups the digits where every
/// group after the first has three digits, and is else a decimal separator too, so that `1.000`
/// is a thousand in German and `1.5` one and a half in either language. Both separators in one
/// number have to be in the language's order, e.g. `1.800,5` in German. No exponent, e.g. `1e3`.
pub(crate) fn read(text: &str, language: Language, signed: bool) -> Option<Typed> {
    let text = text.trim();
    let (negative, body) = match text.strip_prefix(['-', '−']) {
        Some(body) if signed => (true, body),
        Some(_) => return None,
        None => (false, text),
    };
    let (grouping, decimal) = match language {
        Language::English => (',', '.'),
        Language::German => ('.', ','),
    };
    let digits = |text: &str| !text.is_empty() && text.chars().all(|char| char.is_ascii_digit());
    // the whole part grouped by thousands, without its separators
    let grouped = |whole: &str| -> Option<String> {
        let mut groups = whole.split(grouping);
        let first = groups.next()?;
        let rest: Vec<_> = groups.collect();
        (digits(first)
            && first.len() <= 3
            && rest.iter().all(|group| group.len() == 3 && digits(group)))
        .then(|| std::iter::once(first).chain(rest).collect())
    };
    let typed = |whole: String, fraction: Option<&str>| {
        (digits(&whole) && fraction.is_none_or(digits)).then(|| Typed {
            negative,
            whole,
            fraction: fraction.map(str::to_owned),
        })
    };
    match (body.contains(grouping), body.split_once(decimal)) {
        (_, Some((whole, fraction))) if whole.contains(grouping) => {
            typed(grouped(whole)?, Some(fraction))
        }
        (_, Some((whole, fraction))) => typed(whole.to_owned(), Some(fraction)),
        (true, None) => match grouped(body) {
            Some(whole) => typed(whole, None),
            None => {
                let (whole, fraction) = body.split_once(grouping)?;
                typed(whole.to_owned(), Some(fraction))
            }
        },
        (false, None) => typed(body.to_owned(), None),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    fn number(text: &str, language: Language) -> Option<f64> {
        read(text, language, true).map(|typed| typed.get())
    }

    #[test]
    fn the_separator_of_the_language_groups_thousands_and_the_other_is_a_decimal_point() {
        use Language::{English, German};
        for (text, english, german) in [
            ("1500", Some(1500.0), Some(1500.0)),
            ("1.000", Some(1.0), Some(1000.0)),
            ("1,000", Some(1000.0), Some(1.0)),
            ("1.5", Some(1.5), Some(1.5)),
            ("1,5", Some(1.5), Some(1.5)),
            ("1.800,5", None, Some(1800.5)),
            ("1,800.5", Some(1800.5), None),
            ("1.000.000", None, Some(1_000_000.0)),
            ("1.0005", Some(1.0005), Some(1.0005)),
            (" -5 ", Some(-5.0), Some(-5.0)),
            ("−7,5", Some(-7.5), Some(-7.5)),
            ("1e3", None, None),
            ("1.2.3", None, None),
            ("", None, None),
            ("-", None, None),
            (".5", None, None),
            ("5.", None, None),
            ("inf", None, None),
        ] {
            assert_eq!(number(text, English), english, "{text:?} in English");
            assert_eq!(number(text, German), german, "{text:?} in German");
        }
    }

    #[test]
    fn a_sign_is_read_only_where_it_is_allowed() {
        assert_eq!(read("-5", Language::English, false), None);
        assert_eq!(
            read("5", Language::English, false),
            Some(Typed {
                negative: false,
                whole: "5".to_owned(),
                fraction: None,
            })
        );
    }
}
