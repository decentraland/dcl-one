//! The old services' OpenAPI documents (utoipa's, as the web tier's clients
//! were generated from them: `library/catalyst-api/openapi`), served
//! at each service's `/openapi.json`.

use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};

pub const PLACES: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/library/catalyst-api/openapi/places.openapi.json"
));
pub const EVENTS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/library/catalyst-api/openapi/events.openapi.json"
));
pub const WORLDS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/library/catalyst-api/openapi/worlds.openapi.json"
));
pub const COMMUNITIES: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/library/catalyst-api/openapi/communities.openapi.json"
));

pub fn document(spec: &'static str) -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        spec,
    )
        .into_response()
}
