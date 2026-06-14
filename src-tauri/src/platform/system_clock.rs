use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClockError {
    #[error("system time is before the Unix epoch")]
    BeforeEpoch(#[source] std::time::SystemTimeError),

    #[error("system time is outside Burnly's supported timestamp range")]
    OutOfRange,
}

pub fn now_epoch_ms() -> Result<i64, ClockError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(ClockError::BeforeEpoch)?;
    i64::try_from(duration.as_millis()).map_err(|_| ClockError::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_time_is_a_positive_epoch_timestamp() {
        assert!(now_epoch_ms().expect("read system time") > 0);
    }
}
