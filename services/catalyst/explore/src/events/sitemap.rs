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
/// The approved, undeleted events, one page of ids, oldest first (the old
/// order; a malformed `created_at` is a NULL `created_at_ts` and sorts last
/// instead of failing the page).
const SITEMAP_SQL: &str = "SELECT id FROM explore.event WHERE approved IS TRUE AND deleted IS FALSE \
     ORDER BY created_at_ts ASC NULLS LAST, id ASC OFFSET $1 LIMIT $2";

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
    // A page past i64 is past every row: an empty urlset, never a wrapped
    // offset.
    if let Some(offset) = page.and_then(|p| p.checked_mul(SITEMAP_ITEMS_PER_PAGE)) {
        let ids: Vec<String> = sqlx::query_scalar(SITEMAP_SQL)
            .bind(offset)
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

/// Every row of the schedules table, expired ones included, as the old
/// `getSchedulesSitemap` listed `ScheduleModel.find()` (`Sitemap/routes.ts:84-94`;
/// fixture `events.sitemap.schedules`).
pub async fn sitemap_schedules(State(state): State<SharedState>) -> Result<Response, ApiError> {
    let base = &state.settings.events_base_url;
    let ids = state.schedules.all().await?.into_iter().map(|s| s.id);
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

    /// `Sitemap/routes.ts:84-94`: every schedule row, whether or not an event
    /// names it and whether or not it still runs; `page=-1` stays an empty
    /// urlset (the upstream 500 is not reproduced).
    #[tokio::test]
    async fn the_schedules_sitemap_lists_the_schedules_table() {
        use axum::body::Body;
        use axum::http::Request;
        use std::sync::Arc;
        use tower::ServiceExt;
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let expired = uuid::Uuid::new_v4().to_string();
        let unreferenced = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO explore.schedules_local (id, name, active_since, active_until) VALUES \
             ($1, 'Expired', timezone('UTC', now()) - interval '2 days', timezone('UTC', now()) - interval '1 day'), \
             ($2, 'Unreferenced', timezone('UTC', now()), timezone('UTC', now()) + interval '1 day')",
        )
        .bind(&expired)
        .bind(&unreferenced)
        .execute(&pool)
        .await
        .unwrap();
        let app = crate::router(Arc::new(crate::ExploreState::new(
            pool,
            crate::Settings::for_tests(),
            None,
        )));
        async fn get(app: &axum::Router, uri: &str) -> (StatusCode, String) {
            let response = app
                .clone()
                .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap();
            (status, String::from_utf8(bytes.to_vec()).unwrap())
        }
        let (status, body) = get(&app, "/events/sitemap.schedules.xml").await;
        assert_eq!(status, StatusCode::OK);
        for id in [&expired, &unreferenced] {
            assert!(
                body.contains(&format!(
                    "<url><loc>https://events.decentraland.org/schedule/?id={id}</loc></url>"
                )),
                "{body}"
            );
        }
        let (status, body) = get(&app, "/events/sitemap.events.xml?page=-1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\"></urlset>"
        );
    }
}
