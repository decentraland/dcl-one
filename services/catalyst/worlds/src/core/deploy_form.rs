//! The multipart form of `POST /entities`: fields in memory, file parts
//! spooled to disk ([`super::upload_spool`]), both accounted against the
//! process-wide upload budgets as they are parsed.

use std::collections::BTreeMap;
use std::path::Path;

use axum::extract::Multipart;
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

pub const SPOOL_FAILED_MESSAGE: &str = "Server could not buffer the upload, please retry shortly.";

pub(super) struct DeployForm {
    pub(super) fields: BTreeMap<String, String>,
    pub(super) files: Vec<SpooledFile>,
    /// Owns the spooled parts: they live until the form is dropped.
    pub(super) spool: Option<UploadSpool>,
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
    let mut total_bytes: usize = 0;
    let mut part_count: usize = 0;
    let mut field_count: usize = 0;

    loop {
        let mut field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return Err(Box::new(
                    err_one(format!("Failed to read multipart field: {e}")).into_response(),
                ));
            }
        };
        let name = field.name().unwrap_or("").to_string();

        part_count += 1;
        if part_count > upload_limits::MAX_MULTIPART_PARTS {
            return Err(Box::new(
                payload_too_large(upload_limits::TOO_MANY_PARTS_MESSAGE).into_response(),
            ));
        }

        if field.file_name().is_some() {
            // The entity itself is one more file part than its content.
            if files.len() > MAX_DEPLOY_FILES {
                return Err(Box::new(
                    payload_too_large(TOO_MANY_FILES_MESSAGE).into_response(),
                ));
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
                    Err(e) => {
                        return Err(Box::new(
                            err_one(format!("Failed to read file data: {e}")).into_response(),
                        ));
                    }
                }
            }
            files.push(part.finish().await.map_err(spool_failed)?);
        } else {
            field_count += 1;
            if field_count > upload_limits::MAX_MULTIPART_FIELDS {
                return Err(Box::new(
                    payload_too_large(upload_limits::TOO_MANY_FIELDS_MESSAGE).into_response(),
                ));
            }
            let mut buf = BytesMut::new();
            loop {
                match field.chunk().await {
                    Ok(Some(chunk)) => {
                        if buf.len().saturating_add(chunk.len())
                            > upload_limits::MAX_MULTIPART_FIELD_VALUE_BYTES
                        {
                            return Err(Box::new(
                                payload_too_large(upload_limits::PAYLOAD_TOO_LARGE_MESSAGE)
                                    .into_response(),
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
                    Err(e) => {
                        return Err(Box::new(
                            err_one(format!("Failed to read field value: {e}")).into_response(),
                        ));
                    }
                }
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
    use axum::http::StatusCode;
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

    #[tokio::test]
    async fn read_deploy_form_caps_each_field_value_at_one_megabyte() {
        let big = "a".repeat(upload_limits::MAX_MULTIPART_FIELD_VALUE_BYTES + 1);
        let multipart = multipart_from(&[("authChain", None, big.as_str())]).await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let resp = match read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            u64::MAX,
            &test_root(),
        )
        .await
        {
            Err(resp) => *resp,
            Ok(_) => panic!("oversized field must be rejected"),
        };
        let (status, body) = response_json(resp).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            body,
            json!({ "error": "Payload Too Large", "message": "The multipart request is too large." })
        );
        assert!(bytes_lease.reserved() <= upload_limits::MAX_MULTIPART_FIELD_VALUE_BYTES as u64);

        let exact = "a".repeat(upload_limits::MAX_MULTIPART_FIELD_VALUE_BYTES);
        let multipart = multipart_from(&[("authChain", None, exact.as_str())]).await;
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
        .expect("1 MB field parses");
        assert_eq!(
            form.fields.get("authChain").unwrap().len(),
            upload_limits::MAX_MULTIPART_FIELD_VALUE_BYTES
        );
    }

    #[tokio::test]
    async fn read_deploy_form_rejects_more_than_one_hundred_fields() {
        let names: Vec<String> = (0..=upload_limits::MAX_MULTIPART_FIELDS)
            .map(|i| format!("f{i}"))
            .collect();
        let parts: Vec<(&str, Option<&str>, &str)> =
            names.iter().map(|n| (n.as_str(), None, "v")).collect();
        let multipart = multipart_from(&parts).await;
        let mut bytes_lease = upload_limits::reserve_in_flight();
        let mut files_lease = upload_limits::reserve_in_flight_files();
        let resp = match read_deploy_form(
            multipart,
            &mut bytes_lease,
            &mut files_lease,
            u64::MAX,
            u64::MAX,
            &test_root(),
        )
        .await
        {
            Err(resp) => *resp,
            Ok(_) => panic!("101st field must be rejected"),
        };
        let (status, body) = response_json(resp).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            body,
            json!({ "error": "Payload Too Large", "message": "The multipart request has too many fields." })
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
    async fn read_deploy_form_takes_the_entity_part_beside_the_content_cap() {
        assert_eq!(
            read_files(MAX_DEPLOY_FILES + 1).await,
            Ok(MAX_DEPLOY_FILES + 1)
        );
        assert_eq!(
            read_files(MAX_DEPLOY_FILES + 2).await,
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
