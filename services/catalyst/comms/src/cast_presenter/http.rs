//! `POST /presentations`, upstream's
//! `src/controllers/handlers/create-presentation-handler.ts:38-178`: the same
//! request forms, checks, order and `{"error": msg}` answers.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::files::{self, MAX_FILE_SIZE, MAX_JSON_BODY_SIZE};
use super::session::{CreateError, Presenter};
use super::ssrf::{self, DownloadError};
use super::wire::FileType;

/// The router state: no presenter when no bot binary is configured.
pub type PresenterState = Option<Arc<Presenter>>;

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

fn bad_request(message: impl Into<String>) -> Response {
    error(StatusCode::BAD_REQUEST, message)
}

fn internal() -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, "Internal error")
}

fn too_large(max_bytes: usize) -> Response {
    error(
        StatusCode::PAYLOAD_TOO_LARGE,
        format!(
            "Request exceeds maximum size of {}MB",
            max_bytes / (1024 * 1024)
        ),
    )
}

/// The request's file and LiveKit credentials, from either form.
struct Parsed {
    file: Vec<u8>,
    file_name: String,
    livekit_token: String,
    livekit_url: String,
    presenter_identity: Option<String>,
}

/// A multipart body as busboy reads it with `files: 1, fields: 10`: the
/// first part with a file name is the file whatever its field name, later
/// file parts and fields past the tenth are ignored, the last value of a
/// field wins.
async fn parse_multipart(
    content_type: &str,
    body: bytes::Bytes,
) -> Result<
    (
        Option<(Vec<u8>, String)>,
        std::collections::HashMap<String, String>,
    ),
    Response,
> {
    let boundary = multer::parse_boundary(content_type).map_err(|_| internal())?;
    let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(body) });
    let mut multipart = multer::Multipart::new(stream, boundary);
    let mut file = None;
    let mut fields = std::collections::HashMap::new();
    let mut field_count = 0;
    while let Some(part) = multipart.next_field().await.map_err(|_| internal())? {
        if let Some(name) = part.file_name().map(str::to_string) {
            let bytes = part.bytes().await.map_err(|_| internal())?;
            if file.is_none() {
                if bytes.len() > MAX_FILE_SIZE {
                    return Err(error(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "File exceeds maximum size of 100MB",
                    ));
                }
                file = Some((bytes.to_vec(), name));
            }
        } else {
            let name = part.name().unwrap_or_default().to_string();
            let value = part.text().await.map_err(|_| internal())?;
            field_count += 1;
            if field_count <= 10 {
                fields.insert(name, value);
            }
        }
    }
    Ok((file, fields))
}

fn download_failure(e: DownloadError) -> Response {
    match e {
        DownloadError::InvalidUrl(message) => bad_request(message),
        DownloadError::Failed(message) => {
            bad_request(format!("Failed to download file: {message}"))
        }
        DownloadError::TooLarge => error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "File exceeds maximum size of 100MB",
        ),
        DownloadError::Network(message) => {
            tracing::error!("Failed to create presentation: {message}");
            internal()
        }
    }
}

fn credentials(token: Option<&str>, url: Option<&str>) -> Result<(String, String), Response> {
    let (Some(token), Some(url)) = (
        token.filter(|t| !t.is_empty()),
        url.filter(|u| !u.is_empty()),
    ) else {
        return Err(bad_request("Missing livekitToken or livekitUrl"));
    };
    files::check_livekit_url(url).map_err(bad_request)?;
    Ok((token.to_string(), url.to_string()))
}

async fn parse_json(presenter: &Presenter, body: bytes::Bytes) -> Result<Parsed, Response> {
    let value: Value = serde_json::from_slice(&body)
        .map_err(|_| bad_request("Request body must be valid JSON"))?;
    let Value::Object(body) = value else {
        return Err(bad_request("Request body must be a JSON object"));
    };
    let text = |key: &str| body.get(key).and_then(Value::as_str);
    let presenter_identity = files::presenter_identity(text("presenterIdentity"));
    let Some(url) = text("url").filter(|u| !u.is_empty()) else {
        return Err(bad_request("Missing url"));
    };
    let (livekit_token, livekit_url) = credentials(text("livekitToken"), text("livekitUrl"))?;
    // The token first, so a refused one costs no download.
    if let Err(e) = presenter
        .validate_credentials(&livekit_url, &livekit_token)
        .await
    {
        tracing::info!("LiveKit credentials refused: {e}");
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "Invalid LiveKit credentials",
        ));
    }
    let resolved = files::resolve_file_url(url);
    let downloaded = ssrf::download(&resolved.url)
        .await
        .map_err(download_failure)?;
    Ok(Parsed {
        file: downloaded.bytes,
        file_name: resolved.filename.unwrap_or(downloaded.filename),
        livekit_token,
        livekit_url,
        presenter_identity,
    })
}

/// `POST /presentations`.
pub async fn create_presentation(
    State(presenter): State<PresenterState>,
    request: Request,
) -> Response {
    let Some(presenter) = presenter else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Presentation service unavailable",
        );
    };
    let headers = request.headers().clone();
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let parsed = if content_type.contains("application/json") {
        // The body as received: Content-Length can be absent or wrong.
        let Ok(body) = axum::body::to_bytes(request.into_body(), MAX_JSON_BODY_SIZE).await else {
            return too_large(MAX_JSON_BODY_SIZE);
        };
        parse_json(&presenter, body).await
    } else if content_type.contains("multipart/form-data") {
        let declared = headers
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        if declared.is_some_and(|len| len > MAX_FILE_SIZE as u64) {
            return too_large(MAX_FILE_SIZE);
        }
        let Ok(body) = axum::body::to_bytes(request.into_body(), MAX_FILE_SIZE).await else {
            return too_large(MAX_FILE_SIZE);
        };
        match parse_multipart(&content_type, body).await {
            Err(response) => Err(response),
            Ok((file, fields)) => {
                let field = |k: &str| fields.get(k).map(String::as_str);
                let presenter_identity = files::presenter_identity(field("presenterIdentity"));
                match file {
                    None => Err(bad_request("Missing file in multipart upload")),
                    Some((file, file_name)) => {
                        credentials(field("livekitToken"), field("livekitUrl")).map(
                            |(livekit_token, livekit_url)| Parsed {
                                file,
                                file_name,
                                livekit_token,
                                livekit_url,
                                presenter_identity,
                            },
                        )
                    }
                }
            }
        }
    } else {
        Err(bad_request(
            "Content-Type must be multipart/form-data or application/json",
        ))
    };
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(response) => return response,
    };
    // Stored by upstream only for client composition, which this node does not run.
    let _ = parsed.presenter_identity;

    let file_name = files::sanitize_filename(&parsed.file_name);
    let Some(file_type) = files::file_type_from_name(&file_name) else {
        return bad_request("Unsupported file type. Only .pdf and .pptx files are supported.");
    };
    if !files::magic_bytes_match(&parsed.file, file_type) {
        return bad_request(format!(
            "File content does not match expected {} format",
            file_type.as_str().to_uppercase()
        ));
    }
    if file_type == FileType::Pptx {
        // Not rendered by this node yet: answered as a slide upstream cannot render.
        tracing::warn!("PPTX presentations are not rendered by this node");
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Unable to render the presentation",
        );
    }
    tracing::info!(
        file_name,
        size = parsed.file.len(),
        "Creating presentation from pdf file"
    );
    match presenter
        .create(
            parsed.file,
            &parsed.livekit_url,
            &parsed.livekit_token,
            &file_name,
        )
        .await
    {
        Ok(info) => (StatusCode::CREATED, Json(info)).into_response(),
        Err(CreateError::MaxConcurrent(max)) => error(
            StatusCode::TOO_MANY_REQUESTS,
            CreateError::MaxConcurrent(max).to_string(),
        ),
        Err(CreateError::SlideRender(message)) => {
            tracing::warn!("{message}");
            error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Unable to render the presentation",
            )
        }
        Err(CreateError::Internal(message)) => {
            tracing::error!("Failed to create presentation: {message}");
            internal()
        }
    }
}

/// Upstream's response headers on every answer (`src/components.ts:49-64`).
pub async fn security_headers(request: Request, next: axum::middleware::Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        (
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        ),
        ("content-security-policy", "default-src 'none'"),
    ] {
        headers
            .entry(name)
            .or_insert(HeaderValue::from_static(value));
    }
    response
}
