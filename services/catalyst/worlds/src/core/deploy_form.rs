//! The multipart form of `POST /entities`: fields in memory, file parts
//! spooled to disk ([`super::upload_spool`]), both accounted against the
//! process-wide upload budgets as they are parsed. The caps and texts are
//! upstream's `DEPLOYMENT_FIELD_LIMITS` over busboy
//! (`src/controllers/multipart/{limits,middleware}.ts`).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use axum::extract::Multipart;
use axum::extract::multipart::MultipartError;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::BytesMut;

use super::deploy::{
    FILE_TOO_LARGE_MESSAGE, MAX_UPLOAD_SIZE_BYTES, TOO_MANY_FILES_MESSAGE, err_one,
    payload_too_large,
};
use super::deploy_validate::{MAX_DEPLOY_FILE_BYTES, MAX_DEPLOY_FILES};
use super::metrics::{ENTITIES_ROUTE, upload_rejected};
use super::upload_limits;
use super::upload_spool::{SpooledFile, UploadSpool};

pub const SPOOL_FAILED_MESSAGE: &str =
    "The server could not store the multipart upload, please retry shortly.";

/// Longest auth chain a deployment carries, as on Catalyst.
const MAX_DEPLOYMENT_AUTH_CHAIN_LINKS: usize = 10;
/// Non-file fields: `entityId`, `partial` and `authChain[i][type|payload|signature]`.
pub(super) const MAX_DEPLOYMENT_FIELDS: usize = 2 + 3 * MAX_DEPLOYMENT_AUTH_CHAIN_LINKS;
pub(super) const MAX_DEPLOYMENT_FIELD_SIZE_BYTES: usize = 32 * 1024;
/// busboy's `parts`: answered when the count reaches it, so one part fewer is the most a form has.
const MAX_DEPLOYMENT_PARTS: usize = MAX_DEPLOY_FILES + MAX_DEPLOYMENT_FIELDS;

pub(super) fn too_many_fields_message() -> String {
    format!(
        "{} The maximum allowed is {MAX_DEPLOYMENT_FIELDS}.",
        upload_limits::TOO_MANY_FIELDS_MESSAGE
    )
}

fn field_too_large_message(name: &str) -> String {
    format!(
        "Field '{name}' is too large. The maximum allowed size per field is {MAX_DEPLOYMENT_FIELD_SIZE_BYTES} bytes."
    )
}

pub(super) struct DeployForm {
    pub(super) fields: BTreeMap<String, String>,
    pub(super) files: Vec<SpooledFile>,
    /// Owns the spooled parts: they live until the form is dropped.
    pub(super) spool: Option<UploadSpool>,
}

/// busboy's refusal of the request's `Content-Type`, before any of the body is read.
pub(super) fn content_type_error(headers: &HeaderMap) -> Option<String> {
    let Some(raw) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    else {
        return Some("Missing Content-Type".to_string());
    };
    let mut params = raw.split(';');
    let essence = params.next().unwrap_or_default().trim();
    let Some((kind, subtype)) = essence.split_once('/') else {
        return Some("Malformed content type".to_string());
    };
    let token = |t: &str| !t.is_empty() && !t.contains(char::is_whitespace);
    if !token(kind) || !token(subtype) {
        return Some("Malformed content type".to_string());
    }
    if !(kind.eq_ignore_ascii_case("multipart") && subtype.eq_ignore_ascii_case("form-data")) {
        return Some(format!("Unsupported content type: {raw}"));
    }
    let has_boundary = params.any(|p| {
        p.split_once('=')
            .is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case("boundary") && !v.trim().is_empty())
    });
    (!has_boundary).then(|| "Multipart: Boundary not found".to_string())
}

/// A body the parser could not read: busboy's `Unexpected end of form` for a cut stream, the
/// payload cap for the route's body limit, else the parser's own text.
fn read_failed(error: MultipartError) -> Box<Response> {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        upload_rejected(ENTITIES_ROUTE, "wire_size");
        return Box::new(payload_too_large(upload_limits::PAYLOAD_TOO_LARGE_MESSAGE));
    }
    let cut = std::error::Error::source(&error)
        .and_then(|e| e.downcast_ref::<multer::Error>())
        .is_some_and(|e| {
            matches!(
                e,
                multer::Error::IncompleteStream
                    | multer::Error::IncompleteFieldData { .. }
                    | multer::Error::IncompleteHeaders
            )
        });
    let message = if cut {
        "Unexpected end of form".to_string()
    } else {
        error.body_text()
    };
    Box::new(err_one(message))
}

fn spool_failed(error: std::io::Error) -> Box<Response> {
    tracing::error!(%error, "POST /entities: could not spool an upload part to disk");
    upload_rejected(ENTITIES_ROUTE, "storage");
    Box::new(upload_limits::shed_response(SPOOL_FAILED_MESSAGE))
}

fn account_deploy_bytes(
    total_bytes: &mut usize,
    added: usize,
    bytes_lease: &mut upload_limits::InFlightBytesGuard,
    max_in_flight_bytes: u64,
) -> Result<(), Box<Response>> {
    match upload_limits::account_payload_bytes(
        total_bytes,
        added,
        MAX_UPLOAD_SIZE_BYTES,
        bytes_lease,
        max_in_flight_bytes,
    ) {
        Ok(()) => Ok(()),
        Err(upload_limits::PayloadAccountError::PayloadTooLarge) => {
            upload_rejected(ENTITIES_ROUTE, "payload_size");
            Err(Box::new(
                payload_too_large(upload_limits::PAYLOAD_TOO_LARGE_MESSAGE).into_response(),
            ))
        }
        Err(upload_limits::PayloadAccountError::BudgetExhausted) => {
            upload_rejected(ENTITIES_ROUTE, "bytes");
            tracing::warn!(
                total_bytes,
                in_flight = upload_limits::in_flight_upload_bytes(),
                max = max_in_flight_bytes,
                "POST /entities shed: aggregate in-flight upload budget exceeded"
            );
            Err(Box::new(upload_limits::shed_response(
                upload_limits::BYTES_SHED_MESSAGE,
            )))
        }
    }
}

fn duplicate(name: &str) -> Box<Response> {
    upload_rejected(ENTITIES_ROUTE, "invalid_multipart");
    Box::new(err_one(format!("Duplicate form field '{name}'")))
}

/// Kind a form name was first seen as: `true` for a file part.
type SeenNames = HashMap<String, bool>;

pub(super) async fn read_deploy_form(
    mut multipart: Multipart,
    bytes_lease: &mut upload_limits::InFlightBytesGuard,
    files_lease: &mut upload_limits::InFlightFilesGuard,
    max_in_flight_bytes: u64,
    max_in_flight_files: u64,
    spool_root: &Path,
) -> Result<DeployForm, Box<Response>> {
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut files: Vec<SpooledFile> = Vec::new();
    let mut spool: Option<UploadSpool> = None;
    let mut seen = SeenNames::new();
    let mut total_bytes: usize = 0;
    let mut part_count: usize = 0;
    let mut field_count: usize = 0;

    loop {
        let mut field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return Err(read_failed(e)),
        };
        let name = field.name().unwrap_or("").to_string();
        part_count += 1;
        // busboy takes a part with a file name or an octet-stream type as a file.
        let is_file = field.file_name().is_some()
            || field.content_type().is_some_and(|t| {
                t.split(';')
                    .next()
                    .is_some_and(|e| e.trim().eq_ignore_ascii_case("application/octet-stream"))
            });

        if is_file {
            if files.len() >= MAX_DEPLOY_FILES {
                return Err(Box::new(
                    payload_too_large(TOO_MANY_FILES_MESSAGE).into_response(),
                ));
            }
            if part_count >= MAX_DEPLOYMENT_PARTS {
                return Err(Box::new(
                    payload_too_large(upload_limits::TOO_MANY_PARTS_MESSAGE).into_response(),
                ));
            }
            // Checked on the part's headers, before any of its bytes are written or charged.
            if seen.insert(name.clone(), true).is_some() {
                return Err(duplicate(&name));
            }
            if !files_lease.try_resize(files.len() as u64 + 1, max_in_flight_files) {
                upload_rejected(ENTITIES_ROUTE, "files");
                tracing::warn!(
                    request_files = files.len() + 1,
                    in_flight = upload_limits::in_flight_upload_files(),
                    max = max_in_flight_files,
                    "POST /entities shed: aggregate in-flight upload-file budget exceeded"
                );
                return Err(Box::new(upload_limits::shed_response(
                    upload_limits::FILES_SHED_MESSAGE,
                )));
            }
            if spool.is_none() {
                spool = Some(
                    UploadSpool::create(spool_root)
                        .await
                        .map_err(spool_failed)?,
                );
            }
            let mut part = spool
                .as_mut()
                .expect("created above")
                .part()
                .await
                .map_err(spool_failed)?;
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        if part.len().saturating_add(chunk.len()) > MAX_DEPLOY_FILE_BYTES {
                            return Err(Box::new(
                                payload_too_large(FILE_TOO_LARGE_MESSAGE).into_response(),
                            ));
                        }
                        account_deploy_bytes(
                            &mut total_bytes,
                            chunk.len(),
                            bytes_lease,
                            max_in_flight_bytes,
                        )?;
                        part.write(&chunk).await.map_err(spool_failed)?;
                    }
                    Ok(None) => break,
                    Err(e) => return Err(read_failed(e)),
                }
            }
            files.push(part.finish(name).await.map_err(spool_failed)?);
        } else {
            field_count += 1;
            if field_count > MAX_DEPLOYMENT_FIELDS {
                return Err(Box::new(
                    payload_too_large(too_many_fields_message()).into_response(),
                ));
            }
            if part_count >= MAX_DEPLOYMENT_PARTS {
                return Err(Box::new(
                    payload_too_large(upload_limits::TOO_MANY_PARTS_MESSAGE).into_response(),
                ));
            }
            let mut buf = BytesMut::new();
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        if buf.len().saturating_add(chunk.len()) > MAX_DEPLOYMENT_FIELD_SIZE_BYTES {
                            return Err(Box::new(
                                payload_too_large(field_too_large_message(&name)).into_response(),
                            ));
                        }
                        account_deploy_bytes(
                            &mut total_bytes,
                            chunk.len(),
                            bytes_lease,
                            max_in_flight_bytes,
                        )?;
                        buf.extend_from_slice(&chunk);
                    }
                    Ok(None) => break,
                    Err(e) => return Err(read_failed(e)),
                }
            }
            // A field is checked once read, as busboy emits it.
            if seen.insert(name.clone(), false).is_some() {
                return Err(duplicate(&name));
            }
            fields.insert(name, String::from_utf8_lossy(&buf).into_owned());
        }
    }

    Ok(DeployForm {
        fields,
        files,
        spool,
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use axum::extract::Multipart;

    /// A multipart body from `(name, filename, bytes)` parts and its content type.
    pub(crate) fn multipart_body(parts: &[(&str, Option<&str>, &[u8])]) -> (Vec<u8>, String) {
        let boundary = "xyzgate";
        let mut body = Vec::new();
        for (name, filename, value) in parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            match filename {
                Some(f) => body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n")
                        .as_bytes(),
                ),
                None => body.extend_from_slice(format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes()),
            }
            body.extend_from_slice(value);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        (body, format!("multipart/form-data; boundary={boundary}"))
    }

    /// The `authChain[i][type|payload|signature]` fields of a chain, as clients send it.
    pub(crate) fn chain_fields(chain: &impl serde::Serialize) -> Vec<(String, String)> {
        let chain = serde_json::to_value(chain).unwrap();
        let mut fields = Vec::new();
        for (i, link) in chain.as_array().unwrap().iter().enumerate() {
            for key in ["type", "payload", "signature"] {
                let value = link[key].as_str().unwrap_or_default().to_string();
                fields.push((format!("authChain[{i}][{key}]"), value));
            }
        }
        fields
    }

    pub(crate) async fn multipart_from(parts: &[(&str, Option<&str>, &str)]) -> Multipart {
        use axum::extract::FromRequest;
        let owned: Vec<(&str, Option<&str>, &[u8])> = parts
            .iter()
            .map(|(n, f, v)| (*n, *f, v.as_bytes()))
            .collect();
        let (body, content_type) = multipart_body(&owned);
        let req = axum::http::Request::builder()
            .header("content-type", content_type)
            .body(axum::body::Body::from(body))
            .unwrap();
        Multipart::from_request(req, &()).await.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::multipart_from;
    use super::*;
    use serde_json::{Value, json};

    fn test_root() -> std::path::PathBuf {
        std::env::temp_dir().join("catalyst-deploy-form-tests")
    }

    async fn response_json(resp: Response) -> (StatusCode, Value) {
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn read_deploy_form_grows_the_lease_from_parsed_payload_bytes() {
        let multipart = multipart_from(&[
            ("entityId", None, "bafy123"),
            ("file1", Some("a.txt"), "hello world"),
        ])
        .await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let form = read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            u64::MAX,
            &test_root(),
        )
        .await
        .expect("form parses");
        assert_eq!(form.fields.get("entityId").unwrap(), "bafy123");
        assert_eq!(form.files.len(), 1);
        assert_eq!(bytes_lease.reserved(), 7 + 11);
        assert_eq!(files_lease.reserved(), 1);
    }

    #[tokio::test]
    async fn read_deploy_form_sheds_when_the_byte_budget_is_exhausted() {
        let multipart = multipart_from(&[("entityId", None, "bafy123")]).await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let resp = match read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            0,
            u64::MAX,
            &test_root(),
        )
        .await
        {
            Err(resp) => *resp,
            Ok(_) => panic!("must shed"),
        };
        let (status, body) = response_json(resp).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body,
            json!({ "error": "Service Unavailable", "message": "Server is buffering too many uploads, please retry shortly." })
        );
        assert_eq!(bytes_lease.reserved(), 0);
    }

    #[tokio::test]
    async fn read_deploy_form_sheds_when_the_file_budget_is_exhausted() {
        let multipart = multipart_from(&[
            ("entityId", None, "bafy123"),
            ("file1", Some("a.txt"), "hello world"),
        ])
        .await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let resp = match read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            0,
            &test_root(),
        )
        .await
        {
            Err(resp) => *resp,
            Ok(_) => panic!("must shed"),
        };
        let (status, body) = response_json(resp).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body,
            json!({ "error": "Service Unavailable", "message": "Server is buffering too many upload files, please retry shortly." })
        );
        assert_eq!(files_lease.reserved(), 0);
    }

    async fn rejection(parts: &[(&str, Option<&str>, &str)]) -> (StatusCode, Value) {
        let multipart = multipart_from(parts).await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        match read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            u64::MAX,
            &test_root(),
        )
        .await
        {
            Err(resp) => response_json(*resp).await,
            Ok(_) => panic!("the form must be refused"),
        }
    }

    /// Upstream `test/integration/multipart-error-handling.spec.ts` "a field exceeds the maximum field size".
    #[tokio::test]
    async fn a_field_over_32_kib_is_refused_by_name() {
        let big = "x".repeat(MAX_DEPLOYMENT_FIELD_SIZE_BYTES + 1);
        assert_eq!(
            rejection(&[("entityId", None, big.as_str())]).await,
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({ "error": "Payload Too Large", "message": "Field 'entityId' is too large. The maximum allowed size per field is 32768 bytes." })
            )
        );

        let exact = "a".repeat(MAX_DEPLOYMENT_FIELD_SIZE_BYTES);
        let multipart = multipart_from(&[("authChain[0][signature]", None, exact.as_str())]).await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let form = read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            u64::MAX,
            &test_root(),
        )
        .await
        .expect("a 32 KiB field parses");
        assert_eq!(
            form.fields["authChain[0][signature]"].len(),
            MAX_DEPLOYMENT_FIELD_SIZE_BYTES
        );
    }

    /// Upstream `multipart-error-handling.spec.ts` "more fields than allowed" (200 fields).
    #[tokio::test]
    async fn the_33rd_field_is_refused_with_the_cap() {
        let names: Vec<String> = (0..200).map(|i| format!("field{i}")).collect();
        let parts: Vec<(&str, Option<&str>, &str)> =
            names.iter().map(|n| (n.as_str(), None, "x")).collect();
        assert_eq!(
            rejection(&parts).await,
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                json!({ "error": "Payload Too Large", "message": "The multipart request has too many fields. The maximum allowed is 32." })
            )
        );
    }

    /// Upstream `rejectIfDuplicate` (`multipart/middleware.ts`): a field or file name only once.
    #[tokio::test]
    async fn a_repeated_form_name_is_refused() {
        for parts in [
            vec![("f", None, "a"), ("f", None, "b")],
            vec![("f", Some("f"), "a"), ("f", Some("f"), "a")],
            vec![("f", None, "a"), ("f", Some("f"), "a")],
        ] {
            assert_eq!(
                rejection(&parts).await,
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "Bad request", "message": "Duplicate form field 'f'" })
                )
            );
        }
    }

    #[test]
    fn the_content_type_is_refused_in_busboys_words() {
        let headers = |ct: Option<&str>| {
            let mut h = HeaderMap::new();
            if let Some(ct) = ct {
                h.insert(header::CONTENT_TYPE, ct.parse().unwrap());
            }
            h
        };
        assert_eq!(
            content_type_error(&headers(None)).as_deref(),
            Some("Missing Content-Type")
        );
        assert_eq!(
            content_type_error(&headers(Some("application/json"))).as_deref(),
            Some("Unsupported content type: application/json")
        );
        assert_eq!(
            content_type_error(&headers(Some("garbage"))).as_deref(),
            Some("Malformed content type")
        );
        assert_eq!(
            content_type_error(&headers(Some("multipart/form-data"))).as_deref(),
            Some("Multipart: Boundary not found")
        );
        assert_eq!(
            content_type_error(&headers(Some("Multipart/Form-Data; boundary=x"))),
            None
        );
    }

    async fn read_files(count: usize) -> Result<usize, StatusCode> {
        let root = std::env::temp_dir().join(format!("catalyst-form-{}", uuid::Uuid::new_v4()));
        let names: Vec<String> = (0..count).map(|i| format!("f{i}")).collect();
        let parts: Vec<(&str, Option<&str>, &str)> = names
            .iter()
            .map(|n| (n.as_str(), Some(n.as_str()), n.as_str()))
            .collect();
        let multipart = multipart_from(&parts).await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let read = read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            u64::MAX,
            &root,
        )
        .await;
        let out = read
            .map(|form| form.files.len())
            .map_err(|resp| resp.status());
        let _ = std::fs::remove_dir_all(&root);
        out
    }

    #[tokio::test]
    async fn read_deploy_form_takes_at_most_busboys_file_count() {
        assert_eq!(read_files(MAX_DEPLOY_FILES).await, Ok(MAX_DEPLOY_FILES));
        assert_eq!(
            read_files(MAX_DEPLOY_FILES + 1).await,
            Err(StatusCode::PAYLOAD_TOO_LARGE)
        );
    }

    async fn empty_after_drop(root: &std::path::Path) -> bool {
        for _ in 0..200 {
            if std::fs::read_dir(root).map_or(true, |mut d| d.next().is_none()) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        false
    }

    #[tokio::test]
    async fn spooled_parts_go_when_the_form_or_a_failed_read_is_dropped() {
        let root = std::env::temp_dir().join(format!("catalyst-form-{}", uuid::Uuid::new_v4()));
        let multipart = multipart_from(&[
            ("entityId", None, "bafy123"),
            ("file1", Some("a.txt"), "hello world"),
        ])
        .await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let form = read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            u64::MAX,
            &root,
        )
        .await
        .expect("form parses");
        let file = &form.files[0];
        assert_eq!(file.hash, crate::auth::hash_bytes_v1(b"hello world"));
        assert_eq!(file.read().await.unwrap(), b"hello world");
        assert!(file.path.starts_with(&root));
        drop(form);
        assert!(
            empty_after_drop(&root).await,
            "the spool goes with the form"
        );

        let multipart = multipart_from(&[("file1", Some("a.txt"), "hello world")]).await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let shed = read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            5,
            u64::MAX,
            &root,
        )
        .await;
        assert!(shed.is_err(), "the byte budget sheds mid-part");
        assert!(
            empty_after_drop(&root).await,
            "a failed read leaves nothing"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
