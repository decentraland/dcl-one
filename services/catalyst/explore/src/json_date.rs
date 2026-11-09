//! Dates as node's `Date#toJSON` writes them: millisecond precision, `Z`
//! (`2031-01-01T00:00:00.000Z`), the form every places and events answer
//! carries upstream.
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serializer;

pub fn wire(at: &DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn serialize<S: Serializer>(at: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&wire(at))
}

pub fn serialize_opt<S: Serializer>(at: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
    match at {
        Some(at) => s.serialize_str(&wire(at)),
        None => s.serialize_none(),
    }
}

pub fn serialize_vec<S: Serializer>(dates: &[DateTime<Utc>], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(dates.iter().map(wire))
}
