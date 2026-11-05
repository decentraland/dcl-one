//! The upstream content server parses query strings with `qs`: repeated keys
//! and `key[0]=` indexes both build arrays, so axum's `Query` extractor is not
//! used on the history routes.

use std::collections::HashMap;

pub const MAX_PAGE_SIZE: u32 = 1000;

pub const MAX_QUERY_PARAMS: usize = 2000;

pub type QueryParams = HashMap<String, Vec<String>>;

/// Every entity type a content server stores, in the upstream order.
pub const ENTITY_TYPES: &[&str] = &["scene", "profile", "wearable", "store", "emote", "outfits"];

/// The types the content read routes accept in a path (`/entities/{type}`,
/// `/audit/{type}/..`); outfits live under lambdas and are rejected here.
pub const DEPLOYABLE_ENTITY_TYPES: &[&str] = &["scene", "profile", "wearable", "store", "emote"];

pub fn parse_query_string(raw: &str) -> QueryParams {
    let mut map: QueryParams = HashMap::new();
    if raw.is_empty() {
        return map;
    }
    for pair in raw.split('&').take(MAX_QUERY_PARAMS) {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");

        let decoded_key = urlencoding_decode(key);
        let decoded_value = urlencoding_decode(value);

        let normalized_key = strip_array_index(&decoded_key);

        map.entry(normalized_key).or_default().push(decoded_value);
    }
    map
}

pub fn qs_get_array(params: &QueryParams, key: &str) -> Vec<String> {
    params.get(key).cloned().unwrap_or_default()
}

pub fn qs_get_string(params: &QueryParams, key: &str) -> Option<String> {
    params.get(key).and_then(|v| v.first()).cloned()
}

pub fn qs_get_number(params: &QueryParams, key: &str) -> Option<i64> {
    qs_get_string(params, key).and_then(|s| s.parse::<i64>().ok())
}

pub fn qs_get_bool(params: &QueryParams, key: &str) -> Option<bool> {
    qs_get_string(params, key).map(|s| s == "true")
}

/// Case-insensitive, an exact name first and then with one trailing `s`
/// stripped, so `scenes` is `scene` and `outfits` stays `outfits`.
pub fn parse_entity_type(raw: &str) -> Option<&'static str> {
    let lowered = raw.trim().to_lowercase();
    let exact = |s: &str| ENTITY_TYPES.iter().copied().find(|t| *t == s);
    exact(&lowered).or_else(|| lowered.strip_suffix('s').and_then(exact))
}

/// The `/entities/{type}` and `/audit/{type}` rule: lower-case, strip one
/// trailing `s`, then the deployable set only.
pub fn parse_deployable_entity_type(raw: &str) -> Option<&'static str> {
    let mut s = raw.trim().to_lowercase();
    if s.ends_with('s') {
        s.pop();
    }
    DEPLOYABLE_ENTITY_TYPES.iter().copied().find(|t| *t == s)
}

pub fn to_query_string(filters: &HashMap<String, Vec<String>>) -> String {
    let mut pairs: Vec<String> = Vec::new();
    let mut keys: Vec<&String> = filters.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(values) = filters.get(key) {
            for val in values {
                if !val.is_empty() {
                    pairs.push(format!(
                        "{}={}",
                        urlencoding_encode(key),
                        urlencoding_encode(val)
                    ));
                }
            }
        }
    }
    pairs.join("&")
}

pub fn camel_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(ch.to_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

#[derive(Debug, Clone)]
pub struct Pagination {
    pub page_size: i64,
    pub page_num: i64,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OversizePolicy {
    Reject,
    Clamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonPositivePolicy {
    Reject,
    ClampToOne,
    PassThrough,
}

pub fn parse_pagination_with(
    params: &QueryParams,
    max_page_size: i64,
    oversize: OversizePolicy,
    non_positive: NonPositivePolicy,
) -> Result<Pagination, String> {
    let mut page_size = qs_get_number(params, "pageSize").unwrap_or(100);
    let mut page_num = qs_get_number(params, "pageNum").unwrap_or(1);

    match non_positive {
        NonPositivePolicy::Reject => {
            if page_size < 1 {
                return Err("pageSize must be a positive integer".to_string());
            }
            if page_num < 1 {
                return Err("pageNum must be a positive integer".to_string());
            }
        }
        NonPositivePolicy::ClampToOne => {
            if page_size < 1 {
                page_size = 1;
            }
            if page_num < 1 {
                page_num = 1;
            }
        }
        NonPositivePolicy::PassThrough => {}
    }

    if page_size > max_page_size {
        match oversize {
            OversizePolicy::Reject => {
                return Err(format!("max allowed pageSize is {}", max_page_size));
            }
            OversizePolicy::Clamp => page_size = max_page_size,
        }
    }

    let offset = page_num.saturating_sub(1).saturating_mul(page_size);
    let limit = page_size;
    Ok(Pagination {
        page_size,
        page_num,
        offset,
        limit,
    })
}

pub fn parse_pagination(params: &QueryParams, max_page_size: i64) -> Result<Pagination, String> {
    parse_pagination_with(
        params,
        max_page_size,
        OversizePolicy::Reject,
        NonPositivePolicy::Reject,
    )
}

/// JavaScript's `parseInt`: leading white space, an optional sign, then the
/// longest run of digits; `None` where JavaScript answers `NaN`. With
/// `hex_prefix` (`parseInt(s)` with no radix) a `0x`/`0X` reads hex digits.
/// A value past `i64` saturates.
pub fn js_parse_int(s: &str, hex_prefix: bool) -> Option<i64> {
    let s = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let (radix, s) = match s.get(..2) {
        Some("0x" | "0X") if hex_prefix => (16, &s[2..]),
        _ => (10, s),
    };
    let digits: Vec<u32> = s.chars().map_while(|c| c.to_digit(radix)).collect();
    if digits.is_empty() {
        return None;
    }
    let magnitude = digits.iter().fold(0i64, |acc, &d| {
        acc.saturating_mul(i64::from(radix))
            .saturating_add(i64::from(d))
    });
    Some(if negative { -magnitude } else { magnitude })
}

/// lamb2's `paginationObject` with `fetchAndPaginate`'s slicing: each of
/// `pageSize` (default 100) and `pageNum` (default 1) is `parseInt(v, 10)`,
/// `None` for `NaN` (answered as `null`); a `pageSize` over the maximum is
/// the 400; the page is `slice(offset, offset + pageSize)` with
/// `offset = (pageNum - 1) * pageSize`, so a zero, negative or `NaN` value
/// is not clamped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsPagination {
    pub page_size: Option<i64>,
    pub page_num: Option<i64>,
}

impl JsPagination {
    pub fn parse(params: &QueryParams, max_page_size: i64) -> Result<Self, String> {
        let read = |key: &str, default: i64| match qs_get_string(params, key) {
            Some(v) => js_parse_int(&v, false),
            None => Some(default),
        };
        let page_size = read("pageSize", 100);
        if page_size.is_some_and(|s| s > max_page_size) {
            return Err(format!("max allowed pageSize is {max_page_size}"));
        }
        Ok(Self {
            page_size,
            page_num: read("pageNum", 1),
        })
    }

    /// `slice`'s arguments, `None` when one is `NaN` (an empty page).
    fn bounds(&self) -> Option<(i64, i64)> {
        let size = self.page_size?;
        let start = self.page_num?.saturating_sub(1).saturating_mul(size);
        Some((start, start.saturating_add(size)))
    }

    /// The page of a list of `len` elements, as `Array.prototype.slice`
    /// takes it: a negative bound counts from the end.
    pub fn range(&self, len: usize) -> std::ops::Range<usize> {
        let Some((start, end)) = self.bounds() else {
            return 0..0;
        };
        let len = len as i64;
        let at = |i: i64| if i < 0 { (len + i).max(0) } else { i.min(len) } as usize;
        let (start, end) = (at(start), at(end));
        if start < end { start..end } else { 0..0 }
    }

    /// `(offset, limit)` for a query that pages in SQL, when the page does
    /// not depend on the list's length (no negative bound); otherwise
    /// `None`, and the caller counts first and takes [`Self::range`].
    pub fn sql_window(&self) -> Option<(i64, i64)> {
        match self.bounds() {
            None => Some((0, 0)),
            Some((start, end)) if start >= 0 && end >= 0 => Some((start, (end - start).max(0))),
            Some(_) => None,
        }
    }
}

fn urlencoding_decode(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::with_capacity(s.len());
    let mut buf = [0u8; 4];
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hi = chars.next().unwrap_or('0');
            let lo = chars.next().unwrap_or('0');
            let hex = format!("{}{}", hi, lo);
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                bytes.push(byte);
            } else {
                bytes.push(b'%');
                bytes.extend_from_slice(hi.encode_utf8(&mut buf).as_bytes());
                bytes.extend_from_slice(lo.encode_utf8(&mut buf).as_bytes());
            }
        } else if ch == '+' {
            bytes.push(b' ');
        } else {
            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn urlencoding_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

fn strip_array_index(key: &str) -> String {
    if let Some(bracket_pos) = key.find('[')
        && key.ends_with(']')
    {
        let inside = &key[bracket_pos + 1..key.len() - 1];
        if inside.chars().all(|c| c.is_ascii_digit()) {
            return key[..bracket_pos].to_string();
        }
    }
    key.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repeated_keys() {
        let params = parse_query_string("cid=abc&cid=def&cid=ghi");
        assert_eq!(qs_get_array(&params, "cid"), vec!["abc", "def", "ghi"]);
    }

    #[test]
    fn parse_indexed_array() {
        let params = parse_query_string("entityType[0]=scene&entityType[1]=profile");
        assert_eq!(
            qs_get_array(&params, "entityType"),
            vec!["scene", "profile"]
        );
    }

    #[test]
    fn parse_just_over_endpoint_cap_is_visible_to_handlers() {
        let query: String = (0..1001)
            .map(|i| format!("cid={}", i))
            .collect::<Vec<_>>()
            .join("&");
        let params = parse_query_string(&query);
        assert_eq!(qs_get_array(&params, "cid").len(), 1001);
    }

    #[test]
    fn parse_bounds_pathological_flood_to_param_limit() {
        let query: String = (0..3000)
            .map(|i| format!("cid={}", i))
            .collect::<Vec<_>>()
            .join("&");
        let params = parse_query_string(&query);
        assert!(qs_get_array(&params, "cid").len() <= MAX_QUERY_PARAMS);
    }

    #[test]
    fn parse_pagination_defaults() {
        let params = parse_query_string("");
        let p = parse_pagination(&params, 1000).unwrap();
        assert_eq!(p.page_size, 100);
        assert_eq!(p.page_num, 1);
        assert_eq!(p.offset, 0);
        assert_eq!(p.limit, 100);
    }

    #[test]
    fn camel_to_snake_works() {
        assert_eq!(camel_to_snake("localTimestamp"), "local_timestamp");
        assert_eq!(camel_to_snake("entityTimestamp"), "entity_timestamp");
    }

    #[test]
    fn entity_types_parse_singular_plural_and_case() {
        assert_eq!(parse_entity_type("scenes"), Some("scene"));
        assert_eq!(parse_entity_type("Profile"), Some("profile"));
        assert_eq!(parse_entity_type("outfits"), Some("outfits"));
        assert_eq!(parse_entity_type("outfit"), None);
        assert_eq!(parse_entity_type("totallybogus"), None);
        assert_eq!(parse_deployable_entity_type("scenes"), Some("scene"));
        assert_eq!(parse_deployable_entity_type("outfits"), None);
        assert_eq!(parse_deployable_entity_type("outfit"), None);
        assert_eq!(parse_deployable_entity_type("banana"), None);
    }

    #[test]
    fn pagination_oversize_clamp() {
        let params = parse_query_string("pageSize=99999");
        let p = parse_pagination_with(
            &params,
            1000,
            OversizePolicy::Clamp,
            NonPositivePolicy::ClampToOne,
        )
        .unwrap();
        assert_eq!(p.page_size, 1000);
        assert_eq!(p.limit, 1000);
    }

    #[test]
    fn pagination_oversize_reject() {
        let params = parse_query_string("pageSize=99999");
        let err = parse_pagination_with(
            &params,
            1000,
            OversizePolicy::Reject,
            NonPositivePolicy::PassThrough,
        )
        .unwrap_err();
        assert_eq!(err, "max allowed pageSize is 1000");
    }

    #[test]
    fn pagination_non_positive_clamp_to_one() {
        let params = parse_query_string("pageSize=0&pageNum=-3");
        let p = parse_pagination_with(
            &params,
            1000,
            OversizePolicy::Clamp,
            NonPositivePolicy::ClampToOne,
        )
        .unwrap();
        assert_eq!(p.page_size, 1);
        assert_eq!(p.page_num, 1);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn pagination_strict_rejects_non_positive() {
        let zero = parse_query_string("pageSize=0");
        assert_eq!(
            parse_pagination(&zero, 1000).unwrap_err(),
            "pageSize must be a positive integer"
        );
        let neg = parse_query_string("pageNum=-1");
        assert_eq!(
            parse_pagination(&neg, 1000).unwrap_err(),
            "pageNum must be a positive integer"
        );
    }

    #[test]
    fn pagination_passthrough_keeps_values() {
        let params = parse_query_string("pageSize=100&pageNum=0");
        let p = parse_pagination_with(
            &params,
            i64::MAX,
            OversizePolicy::Reject,
            NonPositivePolicy::PassThrough,
        )
        .unwrap();
        assert_eq!(p.page_num, 0);
        assert_eq!(p.offset, -100);
    }

    #[test]
    fn pagination_huge_pagenum_saturates() {
        let params = parse_query_string(&format!("pageNum={}&pageSize=100", i64::MAX));
        let p = parse_pagination_with(
            &params,
            1000,
            OversizePolicy::Reject,
            NonPositivePolicy::PassThrough,
        )
        .unwrap();
        assert_eq!(p.offset, i64::MAX);
    }

    #[test]
    fn decode_reassembles_multibyte_utf8() {
        let params = parse_query_string("textSearch=caf%C3%A9&name=%E4%B8%AD%E6%96%87");
        assert_eq!(
            qs_get_string(&params, "textSearch").as_deref(),
            Some("caf\u{E9}")
        );
        assert_eq!(
            qs_get_string(&params, "name").as_deref(),
            Some("\u{4E2D}\u{6587}")
        );
    }

    #[test]
    fn js_parse_int_reads_as_javascript_does() {
        for (s, hex, want) in [
            ("12", false, Some(12)),
            (" \t-7", false, Some(-7)),
            ("+3", false, Some(3)),
            ("1.5", false, Some(1)),
            ("12abc", false, Some(12)),
            ("0x10", false, Some(0)),
            ("0x10", true, Some(16)),
            ("-0X1f", true, Some(-31)),
            ("0x", true, None),
            ("", false, None),
            ("abc", false, None),
            ("-", false, None),
            ("99999999999999999999999", false, Some(i64::MAX)),
        ] {
            assert_eq!(js_parse_int(s, hex), want, "{s:?} hex={hex}");
        }
    }

    /// `paginationObject` plus `elements.slice(offset, offset + limit)`.
    #[test]
    fn js_pagination_slices_as_javascript_does() {
        let page = |q: &str| JsPagination::parse(&parse_query_string(q), i64::MAX).unwrap();
        let p = page("");
        assert_eq!((p.page_size, p.page_num), (Some(100), Some(1)));
        assert_eq!(p.range(150), 0..100);
        assert_eq!(p.sql_window(), Some((0, 100)));
        assert_eq!(page("pageSize=2&pageNum=2").range(5), 2..4);
        assert_eq!(page("pageSize=0").range(5), 0..0);
        // pageNum 0: slice(-2, 0), empty; pageNum -1: slice(-4, -2).
        assert_eq!(page("pageSize=2&pageNum=0").range(5), 0..0);
        assert_eq!(page("pageSize=2&pageNum=-1").range(5), 1..3);
        assert_eq!(page("pageSize=2&pageNum=-1").sql_window(), None);
        // A negative size: slice(-4, -6) is empty; slice(0, -1) all but the last.
        assert_eq!(page("pageSize=-2&pageNum=3").range(5), 0..0);
        assert_eq!(page("pageSize=-1&pageNum=1").range(5), 0..4);
        let nan = page("pageSize=abc&pageNum=1");
        assert_eq!((nan.page_size, nan.range(5)), (None, 0..0));
        assert_eq!(nan.sql_window(), Some((0, 0)));
        assert_eq!(page("pageSize=2.9&pageNum=2x").range(5), 2..4);
        assert_eq!(
            JsPagination::parse(&parse_query_string("pageSize=1001"), 1000).unwrap_err(),
            "max allowed pageSize is 1000"
        );
        assert!(JsPagination::parse(&parse_query_string("pageSize=x"), 1000).is_ok());
    }

    #[test]
    fn decode_plus_is_space_and_malformed_percent_preserved() {
        let params = parse_query_string("q=a+b&bad=%ZZ");
        assert_eq!(qs_get_string(&params, "q").as_deref(), Some("a b"));
        assert_eq!(qs_get_string(&params, "bad").as_deref(), Some("%ZZ"));
    }

    #[test]
    fn next_page_query_string_is_sorted_and_encoded() {
        let mut filters: HashMap<String, Vec<String>> = HashMap::new();
        filters.insert("to".into(), vec!["5".into()]);
        filters.insert("entityType".into(), vec!["scene".into(), "".into()]);
        filters.insert("lastId".into(), vec!["a b".into()]);
        assert_eq!(
            to_query_string(&filters),
            "entityType=scene&lastId=a%20b&to=5"
        );
    }
}
