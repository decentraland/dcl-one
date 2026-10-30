//! The events site's sitemaps: `GET /events/sitemap.xml` (the index: the
//! static and schedules maps, then one events page per 100 approved
//! events), `/events/sitemap.static.xml`, `/events/sitemap.events.xml?page=`
//! and `/events/sitemap.schedules.xml`, every location on the events site
//! (`EVENTS_BASE_URL`, the old default `https://events.decentraland.org`).

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};

use crate::SharedState;
use crate::shared::errors::ApiError;

pub const SITEMAP_ITEMS_PER_PAGE: i64 = 100;
/// The approved, undeleted events, one page of ids, oldest first (the
/// old order; a malformed `created_at` sorts last instead of failing the page).
const SITEMAP_SQL: &str = "SELECT id FROM explore.event WHERE approved IS TRUE \
     AND (raw->>'deleted_by_user') IS DISTINCT FROM 'true' AND (raw->>'deleted_by_admin') IS DISTINCT FROM 'true' \
     ORDER BY CASE WHEN jsonb_typeof(raw->'created_at') = 'string' AND raw->>'created_at' ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+(Z|[+-][0-9]{2}:?[0-9]{2})$' \
          THEN (raw->>'created_at')::timestamptz END ASC NULLS LAST, id ASC OFFSET $1 LIMIT $2";
/// Every schedule an event names, capped (a site map of schedules, not of events).
const SCHEDULE_IDS_SQL: &str = "SELECT DISTINCT s AS id FROM explore.event, jsonb_array_elements_text(raw->'schedules') AS s \
     WHERE jsonb_typeof(raw->'schedules') = 'array' ORDER BY id ASC LIMIT 50000";

fn site_url(base: &str, pathname: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if pathname.is_empty() {
        format!("{trimmed}/")
    } else {
        format!("{trimmed}{pathname}")
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml(body: String) -> Response {
    (StatusCode::OK, [(CONTENT_TYPE, "application/xml")], body).into_response()
}

fn open(tag: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><{tag} xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"
    )
}

pub async fn sitemap_index(State(state): State<SharedState>) -> Result<Response, ApiError> {
    let base = &state.settings.events_base_url;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM explore.event WHERE approved IS TRUE")
            .fetch_one(&state.pool)
            .await?;
    let pages = if count <= 0 {
        0
    } else {
        (count + SITEMAP_ITEMS_PER_PAGE - 1) / SITEMAP_ITEMS_PER_PAGE
    };
    let mut out = open("sitemapindex");
    for loc in [
        site_url(base, "/sitemap.static.xml"),
        site_url(base, "/sitemap.schedules.xml"),
    ] {
        out.push_str(&format!(
            "<sitemap><loc>{}</loc></sitemap>",
            xml_escape(&loc)
        ));
    }
    for i in 0..pages {
        let loc = format!("{}?page={i}", site_url(base, "/sitemap.events.xml"));
        out.push_str(&format!(
            "<sitemap><loc>{}</loc></sitemap>",
            xml_escape(&loc)
        ));
    }
    out.push_str("</sitemapindex>");
    Ok(xml(out))
}

pub async fn sitemap_static(State(state): State<SharedState>) -> Response {
    let base = &state.settings.events_base_url;
    let mut out = open("urlset");
    for loc in [site_url(base, ""), site_url(base, "/submit/")] {
        out.push_str(&format!("<url><loc>{}</loc></url>", xml_escape(&loc)));
    }
    out.push_str("</urlset>");
    xml(out)
}

pub async fn sitemap_events(
    State(state): State<SharedState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let base = &state.settings.events_base_url;
    let raw_page = params.get("page");
    let page = raw_page
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|p| *p >= 0 && raw_page.map(String::as_str) == Some(p.to_string().as_str()));
    let mut out = open("urlset");
    if let Some(page) = page {
        let ids: Vec<String> = sqlx::query_scalar(SITEMAP_SQL)
            .bind(page * SITEMAP_ITEMS_PER_PAGE)
            .bind(SITEMAP_ITEMS_PER_PAGE)
            .fetch_all(&state.pool)
            .await?;
        for id in ids {
            let loc = format!("{}?id={id}", site_url(base, "/event/"));
            out.push_str(&format!("<url><loc>{}</loc></url>", xml_escape(&loc)));
        }
    }
    out.push_str("</urlset>");
    Ok(xml(out))
}

pub async fn sitemap_schedules(State(state): State<SharedState>) -> Result<Response, ApiError> {
    let base = &state.settings.events_base_url;
    let ids: Vec<String> = sqlx::query_scalar(SCHEDULE_IDS_SQL)
        .fetch_all(&state.pool)
        .await?;
    let mut out = open("urlset");
    for id in ids {
        let loc = format!("{}?id={id}", site_url(base, "/schedule/"));
        out.push_str(&format!("<url><loc>{}</loc></url>", xml_escape(&loc)));
    }
    out.push_str("</urlset>");
    Ok(xml(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_join_the_base_and_escape_the_query() {
        assert_eq!(
            site_url("https://events.example/", ""),
            "https://events.example/"
        );
        assert_eq!(
            site_url("https://events.example", "/submit/"),
            "https://events.example/submit/"
        );
        assert_eq!(xml_escape("a?x=1&y=<2>"), "a?x=1&amp;y=&lt;2&gt;");
    }
}
