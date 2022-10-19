use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

#[inline]
pub fn maybe_timestamp_to_utc_datetime(maybe_timestamp: Option<u64>) -> Option<DateTime<Utc>> {
    maybe_timestamp
        // NB: Chrono represents times before epoch as signed ints, which is useless for us.
        .map(|timestamp| i64::try_from(timestamp))
        .transpose()
        .ok()
        .flatten()
        .and_then(|timestamp| NaiveDateTime::from_timestamp_opt(timestamp, 0))
        .map(|naive| Utc.from_utc_datetime(&naive))
}

#[inline]
pub fn utc_datetime_to_timestamp(datetime: DateTime<Utc>) -> u64 {
    // NB: Chrono represents times before epoch as signed ints, which is useless for us.
    datetime.timestamp().unsigned_abs()
}
