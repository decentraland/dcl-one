//! The places social cards: `GET /places/place?id=|position=` and
//! `/places/world?id=|name=` answer a bare HTML document whose title,
//! description, image and canonical URL are the place's or the world's
//! (Open Graph and Twitter tags), for the link-preview crawlers the places
//! site's `/place/` and `/world/` URLs send here. Every value is HTML-escaped
//! and the image goes through the catalogue's URL sanitizer; an unknown id
//! answers the generic card. The canonical site stays upstream's, as the
//! old service had it.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{Html, IntoResponse, Response};

use super::PlaceRow;
use crate::SharedState;
use crate::sanitize::sanitize_image_url;

const SITE_URL: &str = "https://places.decentraland.org";

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render(title: &str, description: &str, image: &str, url: &str) -> String {
    let t = escape_html(title);
    let d = description.trim();
    let d = match d.find("\n\n") {
        Some(i) if i > 0 => d[..i].trim(),
        _ => d,
    };
    let d = escape_html(d).replace('\n', " ");
    let image = escape_html(image);
    let url = escape_html(url);
    format!(
        "<!DOCTYPE html><html><head>\
<title data-react-helmet=\"true\">{t}</title>\
<meta data-react-helmet=\"true\" name=\"description\" content=\"{d}\" />\
<meta data-react-helmet=\"true\" property=\"og:title\" content=\"{t}\" />\
<meta data-react-helmet=\"true\" property=\"og:description\" content=\"{d}\" />\
<meta data-react-helmet=\"true\" property=\"og:image\" content=\"{image}\" />\
<meta data-react-helmet=\"true\" property=\"og:url\" content=\"{url}\" />\
<meta data-react-helmet=\"true\" property=\"og:type\" content=\"website\" />\
<meta data-react-helmet=\"true\" name=\"twitter:title\" content=\"{t}\" />\
<meta data-react-helmet=\"true\" name=\"twitter:description\" content=\"{d}\" />\
<meta data-react-helmet=\"true\" name=\"twitter:image\" content=\"{image}\" />\
<meta data-react-helmet=\"true\" name=\"twitter:url\" content=\"{url}\" />\
<meta data-react-helmet=\"true\" name=\"twitter:card\" content=\"summary_large_image\" />\
<meta data-react-helmet=\"true\" name=\"twitter:site\" content=\"@decentraland\" />\
<link data-react-helmet=\"true\" rel=\"canonical\" href=\"{url}\" />\
</head><body></body></html>"
    )
}

fn social_url(path: &str, key: &str, value: &str) -> String {
    let mut u = reqwest::Url::parse(SITE_URL).expect("SITE_URL is a valid URL");
    u.set_path(path);
    u.query_pairs_mut().append_pair(key, value);
    u.to_string()
}

fn place_url(place: &PlaceRow) -> String {
    social_url("/place/", "position", &place.base_position)
}

fn with_canonical(url: &str, html: String) -> Response {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&format!("<{}>; rel=canonical", url)) {
        headers.insert(header::LINK, v);
    }
    (headers, Html(html)).into_response()
}

/// The place deployed at `position` (the old service looked the position up
/// as an id, which never matched: the `?position=` card was always generic).
async fn place_at(state: &SharedState, position: &str) -> Option<PlaceRow> {
    let filters = super::PlaceListFilters {
        positions: vec![position.to_string()],
        limit: 1,
        only_places: true,
        ..Default::default()
    };
    state
        .places
        .list_page(&filters)
        .await
        .ok()
        .and_then(|(rows, _)| rows.into_iter().next())
}

pub async fn inject_place_metadata(
    State(state): State<SharedState>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let id = q.get("id").cloned().unwrap_or_default();
    let position = q.get("position").cloned().unwrap_or_default();

    let place = if !id.is_empty() {
        state.places.find_by_id_for(&id, None).await.ok().flatten()
    } else if !position.is_empty() {
        place_at(&state, &position).await
    } else {
        None
    };

    if let Some(place) = place {
        let url = place_url(&place);
        let title = format!(
            "{} | Decentraland Place",
            place.title.clone().unwrap_or_default()
        );
        let image = sanitize_image_url(place.image.as_deref(), state.places.content_origin())
            .unwrap_or_default();
        let html = render(
            &title,
            place.description.as_deref().unwrap_or("").trim(),
            &image,
            &url,
        );
        return with_canonical(&url, html);
    }

    let url = format!("{}/place/", SITE_URL);
    Html(render("Decentraland Place", "", "", &url)).into_response()
}

pub async fn inject_world_metadata(
    State(state): State<SharedState>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let world_id = q
        .get("id")
        .or_else(|| q.get("name"))
        .cloned()
        .unwrap_or_default()
        .to_lowercase();

    let world = if !world_id.is_empty() {
        state
            .places
            .find_world_by_id_for(&world_id, None)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    if let Some(world) = world {
        let name = world.world_name.clone().unwrap_or_default();
        let url = social_url("/world/", "name", &name);
        let title = format!(
            "{} | Decentraland Place",
            world.title.clone().unwrap_or_default()
        );
        let image = sanitize_image_url(world.image.as_deref(), state.places.content_origin())
            .unwrap_or_default();
        let html = render(
            &title,
            world.description.as_deref().unwrap_or("").trim(),
            &image,
            &url,
        );
        return with_canonical(&url, html);
    }

    let url = format!("{}/world/", SITE_URL);
    Html(render("Decentraland Place", "", "", &url)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(value: Option<&str>) -> Option<String> {
        sanitize_image_url(value, None)
    }

    const BREAKOUT: &str = "https://a\"><script>alert(1)</script><meta name=\"x";

    #[test]
    fn sanitize_image_url_keeps_valid_http_and_https_urls_normalized() {
        assert_eq!(
            img(Some("https://cdn.decentraland.org/thumb.png")),
            Some("https://cdn.decentraland.org/thumb.png".to_string())
        );
        assert_eq!(
            img(Some("http://cdn.decentraland.org/thumb.png")),
            Some("http://cdn.decentraland.org/thumb.png".to_string())
        );
        assert_eq!(
            img(Some("https://cdn.decentraland.org/t.png?w=100&h=50")),
            Some("https://cdn.decentraland.org/t.png?w=100&h=50".to_string())
        );
    }

    #[test]
    fn sanitize_image_url_rejects_html_breakout_values() {
        assert_eq!(img(Some(BREAKOUT)), None);
    }

    #[test]
    fn sanitize_image_url_rejects_non_http_schemes() {
        assert_eq!(img(Some("javascript:alert(document.domain)")), None);
        assert_eq!(img(Some("data:text/html,<script>alert(1)</script>")), None);
        assert_eq!(img(Some("file:///etc/passwd")), None);
    }

    #[test]
    fn sanitize_image_url_rejects_missing_or_unparseable_values() {
        assert_eq!(img(None), None);
        assert_eq!(img(Some("")), None);
        assert_eq!(img(Some("not a url")), None);
        assert_eq!(img(Some("/relative/path.png")), None);
    }

    #[test]
    fn sanitize_image_url_percent_encodes_breakout_chars_outside_the_host() {
        assert_eq!(
            img(Some("https://example.com/a\"b<c>d")),
            Some("https://example.com/a%22b%3Cc%3Ed".to_string())
        );
    }

    #[test]
    fn render_never_emits_a_sanitized_breakout_image_as_live_markup() {
        let image = img(Some(BREAKOUT)).unwrap_or_default();
        let html = render(
            "Genesis Plaza | Decentraland Place",
            "description",
            &image,
            "https://places.decentraland.org/place/?position=0,0",
        );
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(!html.contains(BREAKOUT));
        assert!(html.contains("property=\"og:image\" content=\"\""));
    }

    #[test]
    fn render_keeps_a_valid_image_url_as_the_og_image_content() {
        let image = img(Some("https://cdn.decentraland.org/thumb.png")).unwrap_or_default();
        let html = render(
            "Genesis Plaza | Decentraland Place",
            "description",
            &image,
            "https://places.decentraland.org/place/?position=0,0",
        );
        assert!(html.contains("content=\"https://cdn.decentraland.org/thumb.png\""));
    }

    #[test]
    fn render_emits_twitter_variants_alongside_og_for_all_metadata() {
        let html = render(
            "Genesis Plaza | Decentraland Place",
            "A plaza",
            "https://cdn.decentraland.org/thumb.png",
            "https://places.decentraland.org/place/?position=0%2C0",
        );
        for tag in [
            "property=\"og:title\" content=\"Genesis Plaza | Decentraland Place\"",
            "name=\"twitter:title\" content=\"Genesis Plaza | Decentraland Place\"",
            "property=\"og:description\" content=\"A plaza\"",
            "name=\"twitter:description\" content=\"A plaza\"",
            "property=\"og:image\" content=\"https://cdn.decentraland.org/thumb.png\"",
            "name=\"twitter:image\" content=\"https://cdn.decentraland.org/thumb.png\"",
            "property=\"og:url\" content=\"https://places.decentraland.org/place/?position=0%2C0\"",
            "name=\"twitter:url\" content=\"https://places.decentraland.org/place/?position=0%2C0\"",
        ] {
            assert!(html.contains(tag), "missing tag: {tag}");
        }
    }

    #[test]
    fn render_truncates_description_at_first_paragraph_and_collapses_newlines() {
        let html = render(
            "t",
            "  line one\nline two\n\nsecond paragraph  ",
            "",
            "https://places.decentraland.org/place/",
        );
        assert!(html.contains("property=\"og:description\" content=\"line one line two\""));
        assert!(html.contains("name=\"twitter:description\" content=\"line one line two\""));
        assert!(html.contains("name=\"description\" content=\"line one line two\""));
        assert!(!html.contains("second paragraph"));
    }

    #[test]
    fn social_url_form_encodes_query_values_like_upstream() {
        assert_eq!(
            social_url("/place/", "position", "0,0"),
            "https://places.decentraland.org/place/?position=0%2C0"
        );
        assert_eq!(
            social_url("/world/", "name", "my world\".dcl.eth"),
            "https://places.decentraland.org/world/?name=my+world%22.dcl.eth"
        );
    }

    /// The cards over the catalogue: by id, by the parcel it is deployed at,
    /// a world by name, and the generic card for an unknown one.
    #[tokio::test]
    async fn the_cards_describe_the_catalogue_row() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let place = uuid::Uuid::new_v4().to_string();
        let world = format!(
            "card{}.dcl.eth",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        let position = format!(
            "{},-{}",
            rand::random::<u16>() % 150,
            rand::random::<u16>() % 150
        );
        sqlx::query("INSERT INTO explore.place (id, base_position, title, description, raw) VALUES ($1, $2, 'Plaza <b>', 'First\n\nsecond', $3)")
            .bind(&place)
            .bind(&position)
            .bind(serde_json::json!({ "positions": [position], "image": "https://cdn.example.org/p.png" }))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '0,0', 'World', $2)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(serde_json::json!({ "world": true, "world_name": world }))
            .execute(&pool)
            .await
            .unwrap();
        let app = crate::router(std::sync::Arc::new(crate::ExploreState::new(
            pool,
            crate::Settings::for_tests(),
            None,
        )));
        let get = |uri: String| {
            let app = app.clone();
            async move {
                let resp = app
                    .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                let link = resp
                    .headers()
                    .get("link")
                    .map(|v| v.to_str().unwrap().to_string());
                let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
                    .await
                    .unwrap();
                (link, String::from_utf8(bytes.to_vec()).unwrap())
            }
        };
        for uri in [
            format!("/places/place?id={place}"),
            format!("/places/place?position={position}"),
        ] {
            let (link, html) = get(uri.clone()).await;
            assert!(
                html.contains(
                    "property=\"og:title\" content=\"Plaza &lt;b&gt; | Decentraland Place\""
                ),
                "{uri}: {html}"
            );
            assert!(
                html.contains("content=\"First\""),
                "the first paragraph only: {html}"
            );
            assert!(html.contains("content=\"https://cdn.example.org/p.png\""));
            assert!(link.unwrap().contains("rel=canonical"));
        }
        let (_, html) = get(format!("/places/world?name={}", world.to_uppercase())).await;
        assert!(html.contains("World | Decentraland Place"), "{html}");
        let (link, html) = get("/places/place?id=nowhere".into()).await;
        assert!(
            link.is_none()
                && html.contains("<title data-react-helmet=\"true\">Decentraland Place</title>")
        );
    }

    #[test]
    fn render_escapes_image_and_url_attribute_values() {
        let html = render(
            "t",
            "d",
            "https://cdn.decentraland.org/t.png?w=100&h=50",
            "https://x\"><script>alert(1)</script>",
        );
        assert!(html.contains("content=\"https://cdn.decentraland.org/t.png?w=100&amp;h=50\""));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(
            html.contains("content=\"https://x&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;\"")
        );
    }
}
