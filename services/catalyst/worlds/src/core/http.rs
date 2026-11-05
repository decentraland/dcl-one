//! The worlds content server's error envelope (`ApiError`, `envelope`,
//! `clamp_limit`) under the path the worlds routes import it from; it lives
//! in `comms::worlds_envelope`, shared with the world comms mints in comms.
//! The upload-limit answers are re-exported by `upload_limits`. Also the
//! query-string reading the old routes did through `URLSearchParams`.

pub use crate::comms::worlds_envelope::{ApiError, clamp_limit, envelope};

/// A raw query string, every pair in order (`URLSearchParams.get` reads the first).
pub type QueryPairs = Vec<(String, String)>;

pub fn query_param<'a>(q: &'a QueryPairs, key: &str) -> Option<&'a str> {
    q.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// JS `parseInt(s, 10)`, the shared parser: leading JavaScript whitespace,
/// an optional sign, then the leading digits (saturating); `None` where JS
/// gives `NaN`.
pub fn js_parse_int(s: &str) -> Option<i64> {
    crate::shared::query_params::js_parse_int(s)
}

pub const MAX_PAGE_LIMIT: i64 = 100;

/// `getPaginationParams` of @dcl/http-commons: an absent, invalid or
/// out-of-range limit is 100; an invalid or negative offset is 0.
pub fn pagination(q: &QueryPairs) -> (i64, i64) {
    pagination_of(query_param(q, "limit"), query_param(q, "offset"))
}

pub fn pagination_of(limit: Option<&str>, offset: Option<&str>) -> (i64, i64) {
    let limit = limit
        .and_then(js_parse_int)
        .filter(|l| (1..=MAX_PAGE_LIMIT).contains(l))
        .unwrap_or(MAX_PAGE_LIMIT);
    let offset = offset
        .and_then(js_parse_int)
        .filter(|o| *o >= 0)
        .unwrap_or(0);
    (limit, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(raw: &[(&str, &str)]) -> QueryPairs {
        raw.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn pagination_falls_back_like_http_commons() {
        assert_eq!(pagination(&pairs(&[])), (100, 0));
        assert_eq!(
            pagination(&pairs(&[("limit", "5"), ("offset", "7")])),
            (5, 7)
        );
        assert_eq!(pagination(&pairs(&[("limit", "1000")])), (100, 0));
        assert_eq!(pagination(&pairs(&[("limit", "0")])), (100, 0));
        assert_eq!(
            pagination(&pairs(&[("limit", "-1"), ("offset", "-3")])),
            (100, 0)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", "abc"), ("offset", "x")])),
            (100, 0)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", ""), ("offset", "")])),
            (100, 0)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", "7.9"), ("offset", "3px")])),
            (7, 3)
        );
        assert_eq!(
            pagination(&pairs(&[("limit", "2"), ("limit", "50")])),
            (2, 0)
        );
    }
}
