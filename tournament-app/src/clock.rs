//! The clock of the device, which dates every entry a session appends to the log.

use tournament_engine::Timestamp;

/// The time now, by the clock of the device, whatever it is set to.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn now() -> Timestamp {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let millis = |duration: Duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX);
    Timestamp::from_unix_millis(match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(after) => millis(after),
        Err(before) => -millis(before.duration()),
    })
}

/// The time now, by the clock of the device, whatever it is set to.
#[cfg(target_arch = "wasm32")]
pub(crate) fn now() -> Timestamp {
    // `Date.now()` is a whole number of milliseconds, far within what an `f64` holds exactly
    #[expect(clippy::cast_possible_truncation, reason = "a whole number in range")]
    Timestamp::from_unix_millis(js_sys::Date::now() as i64)
}
