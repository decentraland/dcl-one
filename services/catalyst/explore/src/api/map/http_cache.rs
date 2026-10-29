//! The atlas API's conditional-request headers: `Last-Modified` from the
//! grid stamp, a weak ETag from stamp + cache key, and the
//! `max-age`/`stale-while-revalidate` policy upstream serves.

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use chrono::{TimeZone, Utc};

pub const DEFAULT_MAX_AGE: u32 = 120;
pub const DEFAULT_SWR: u32 = 180;
pub const MINIMAP_MAX_AGE: u32 = 600;
pub const MINIMAP_SWR: u32 = 600;

const HTTP_DATE_FMT: &str = "%a, %d %b %Y %H:%M:%S GMT";

pub fn format_http_date(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(|| Utc.timestamp_millis_opt(0).single().unwrap())
        .format(HTTP_DATE_FMT)
        .to_string()
}

fn parse_http_date(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return Some(dt.timestamp_millis());
    }
    chrono::NaiveDateTime::parse_from_str(s, HTTP_DATE_FMT)
        .ok()
        .map(|dt| Utc.from_utc_datetime(&dt).timestamp_millis())
}

fn cache_control_value(max_age: u32, swr: u32) -> String {
    format!("max-age={max_age}, s-maxage={max_age}, stale-while-revalidate={swr}, public")
}

pub fn not_modified(
    req_headers: &HeaderMap,
    last_modified_ms: i64,
    max_age: u32,
    swr: u32,
) -> Option<Response> {
    let ims = req_headers
        .get(header::IF_MODIFIED_SINCE)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_http_date)?;
    if last_modified_ms <= ims {
        let mut resp = Response::new(axum::body::Body::empty());
        *resp.status_mut() = StatusCode::NOT_MODIFIED;
        apply(&mut resp, last_modified_ms, max_age, swr);
        Some(resp)
    } else {
        None
    }
}

pub fn etag_for(last_modified_ms: i64, key: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("W/\"{last_modified_ms:x}-{h:x}\"")
}

pub fn not_modified_etag(
    req_headers: &HeaderMap,
    last_modified_ms: i64,
    etag: &str,
    max_age: u32,
    swr: u32,
) -> Option<Response> {
    if let Some(inm) = req_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        && inm.split(',').any(|t| {
            let t = t.trim();
            t == "*" || t == etag || t.trim_start_matches("W/") == etag.trim_start_matches("W/")
        })
    {
        let mut resp = Response::new(axum::body::Body::empty());
        *resp.status_mut() = StatusCode::NOT_MODIFIED;
        apply_etag(&mut resp, last_modified_ms, etag, max_age, swr);
        return Some(resp);
    }
    not_modified(req_headers, last_modified_ms, max_age, swr)
}

pub fn apply_etag(resp: &mut Response, last_modified_ms: i64, etag: &str, max_age: u32, swr: u32) {
    apply(resp, last_modified_ms, max_age, swr);
    if let Ok(v) = HeaderValue::from_str(etag) {
        resp.headers_mut().insert(header::ETAG, v);
    }
}

pub fn apply(resp: &mut Response, last_modified_ms: i64, max_age: u32, swr: u32) {
    let headers = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&format_http_date(last_modified_ms)) {
        headers.insert(header::LAST_MODIFIED, v);
    }
    if let Ok(v) = HeaderValue::from_str(&cache_control_value(max_age, swr)) {
        headers.insert(header::CACHE_CONTROL, v);
    }
}

/// A LAND token's metadata never changes (the coordinates are the id).
pub const LAND_IMMUTABLE_CACHE_CONTROL: &str = "public, max-age=3600,s-maxage=3600, immutable";

pub fn apply_cache_control(resp: &mut Response, last_modified_ms: i64, cache_control: &str) {
    let headers = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&format_http_date(last_modified_ms)) {
        headers.insert(header::LAST_MODIFIED, v);
    }
    if let Ok(v) = HeaderValue::from_str(cache_control) {
        headers.insert(header::CACHE_CONTROL, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_dates_round_trip() {
        let ms = 1_790_279_207_000;
        let s = format_http_date(ms);
        assert_eq!(s, "Thu, 24 Sep 2026 19:46:47 GMT");
        assert_eq!(parse_http_date(&s), Some(ms));
    }

    #[test]
    fn if_none_match_answers_304_weak_or_strong() {
        let etag = etag_for(5, "k");
        let mut h = HeaderMap::new();
        h.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(etag.trim_start_matches("W/")).unwrap(),
        );
        let resp = not_modified_etag(&h, 5, &etag, 1, 2).expect("304");
        assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(resp.headers()[header::ETAG], etag);
        let mut h = HeaderMap::new();
        h.insert(
            header::IF_MODIFIED_SINCE,
            HeaderValue::from_str(&format_http_date(6_000)).unwrap(),
        );
        assert!(not_modified_etag(&h, 5_000, &etag, 1, 2).is_some());
        assert!(not_modified_etag(&h, 7_000, &etag, 1, 2).is_none());
    }
}
