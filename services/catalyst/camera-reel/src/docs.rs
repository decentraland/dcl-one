//! Camera Reel API documentation and locally served Swagger UI assets.

use axum::Extension;
use axum::extract::NestedPath;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

pub async fn openapi_json() -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/camera-reel/src/openapi.json"
        )),
    )
        .into_response()
}

pub async fn swagger_ui(uri: axum::http::Uri, nested: Option<Extension<NestedPath>>) -> Response {
    let path = uri.path();
    let Some((local_prefix, tail)) = path.split_once("/docs/ui/") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let prefix = format!(
        "{}{local_prefix}",
        nested
            .as_ref()
            .map(|Extension(path)| path.as_str())
            .unwrap_or("")
    );
    let Some(tail) = crate::http::decoded_path_component(tail) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let name = if tail.is_empty() || tail == "/" {
        "index.html"
    } else {
        tail.as_str()
    };
    if name == "swagger-initializer.js" {
        let config = serde_json::json!({
            "dom_id": "#swagger-ui",
            "url": format!("{prefix}/docs/openapi.json"),
            "deepLinking": true,
            "layout": "StandaloneLayout"
        });
        let config = serde_json::to_string_pretty(&config).expect("Swagger configuration");
        let source = include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/camera-reel/swagger-ui/swagger-initializer.js"
        ));
        let start = source.find("    url:").expect("Swagger URL");
        let end = source.find("    presets:").expect("Swagger presets");
        let source = format!(
            "{}{},\n{}",
            &source[..start],
            &config[2..config.len() - 2],
            &source[end..]
        );
        return (
            [(header::CONTENT_TYPE, "application/javascript")],
            source.replace("    layout: \"StandaloneLayout\"", ""),
        )
            .into_response();
    }
    let (content_type, bytes): (&str, &[u8]) = match name {
        "favicon-16x16.png" => (
            "image/png",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/favicon-16x16.png"
            ))
            .as_slice(),
        ),
        "favicon-32x32.png" => (
            "image/png",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/favicon-32x32.png"
            ))
            .as_slice(),
        ),
        "index.css" => (
            "text/css",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/index.css"
            ))
            .as_slice(),
        ),
        "index.html" => (
            "text/html",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/index.html"
            ))
            .as_slice(),
        ),
        "oauth2-redirect.html" => (
            "text/html",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/oauth2-redirect.html"
            ))
            .as_slice(),
        ),
        "swagger-ui-bundle.js" => (
            "application/javascript",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-bundle.js"
            ))
            .as_slice(),
        ),
        "swagger-ui-bundle.js.map" => (
            "text/plain",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-bundle.js.map"
            ))
            .as_slice(),
        ),
        "swagger-ui-es-bundle-core.js" => (
            "application/javascript",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-es-bundle-core.js"
            ))
            .as_slice(),
        ),
        "swagger-ui-es-bundle-core.js.map" => (
            "text/plain",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-es-bundle-core.js.map"
            ))
            .as_slice(),
        ),
        "swagger-ui-es-bundle.js" => (
            "application/javascript",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-es-bundle.js"
            ))
            .as_slice(),
        ),
        "swagger-ui-es-bundle.js.map" => (
            "text/plain",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-es-bundle.js.map"
            ))
            .as_slice(),
        ),
        "swagger-ui-standalone-preset.js" => (
            "application/javascript",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-standalone-preset.js"
            ))
            .as_slice(),
        ),
        "swagger-ui-standalone-preset.js.map" => (
            "text/plain",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui-standalone-preset.js.map"
            ))
            .as_slice(),
        ),
        "swagger-ui.css" => (
            "text/css",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui.css"
            ))
            .as_slice(),
        ),
        "swagger-ui.css.map" => (
            "text/plain",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui.css.map"
            ))
            .as_slice(),
        ),
        "swagger-ui.js" => (
            "application/javascript",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui.js"
            ))
            .as_slice(),
        ),
        "swagger-ui.js.map" => (
            "text/plain",
            include_bytes!(concat!(
                env!("DCL_ONE_ROOT"),
                "/services/catalyst/camera-reel/swagger-ui/swagger-ui.js.map"
            ))
            .as_slice(),
        ),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
}
