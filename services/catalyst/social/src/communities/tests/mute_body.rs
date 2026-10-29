//! `POST`/`DELETE /v1/mutes` read their body the way upstream's handlers do
//! (`add-user-mute-handler.ts`, `remove-user-mute-handler.ts` at `fb3dcb8`):
//! the signed-fetch gate answers before the body is read
//! (`social/mutes-post-unauth`, `-delete-unauth`), then `request.json()` and
//! the `muted_address` guard.

use super::*;

fn bad_request(message: &str) -> Value {
    json!({ "error": "Bad request", "message": message })
}

#[tokio::test]
async fn the_mute_body_is_read_after_the_signature_and_as_upstream_reads_it() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let app = &rig.app;
    let (user, other) = (wallet(), wallet());

    for method in ["post", "delete"] {
        // the gate first: no chain, whatever the body
        for body in [b"{}".to_vec(), b"not json".to_vec(), Vec::new()] {
            let (status, v) = hit(
                app,
                Case::new(method, "/social-api/v1/mutes").body(body, "text/plain"),
            )
            .await;
            assert_gate_400(status, &v, &format!("{method} unsigned"));
        }

        // then request.json(): not JSON, empty or null is the error handler's 500
        for body in [&b"not json"[..], b"", b"null"] {
            let (status, v) = hit(
                app,
                Case::new(method, "/v1/mutes")
                    .signed(&user)
                    .await
                    .body(body.to_vec(), "application/json"),
            )
            .await;
            assert_eq!(
                (status, v),
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "error": "Internal Server Error" })
                ),
                "{method} {:?}",
                String::from_utf8_lossy(body)
            );
        }

        // any other value without a non-empty string muted_address
        for body in [
            json!({}),
            json!([]),
            json!([{ "muted_address": other.address.as_str() }]),
            json!("0x0000000000000000000000000000000000000001"),
            json!(42),
            json!({ "muted_address": 5 }),
            json!({ "muted_address": null }),
            json!({ "muted_address": "" }),
        ] {
            let (status, v) = hit(
                app,
                Case::new(method, "/v1/mutes")
                    .signed(&user)
                    .await
                    .json(&body),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {body}");
            assert_eq!(
                v,
                bad_request("muted_address is required"),
                "{method} {body}"
            );
        }

        // lowercased, never trimmed
        let padded = format!(" {} ", other.address.as_str());
        for raw in ["not-an-address", "0x1234", padded.as_str()] {
            let (status, v) = hit(
                app,
                Case::new(method, "/v1/mutes")
                    .signed(&user)
                    .await
                    .json(&json!({ "muted_address": raw })),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {raw:?}");
            assert_eq!(v, bad_request("Invalid muted_address"), "{method} {raw:?}");
        }
    }

    // a valid body needs no content type, and any casing names the same wallet
    let upper = format!("0X{}", other.address.as_str()[2..].to_ascii_uppercase());
    let (status, v) = hit(
        app,
        Case::new("post", "/v1/mutes").signed(&user).await.body(
            serde_json::to_vec(&json!({ "muted_address": upper })).unwrap(),
            "text/plain",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");
    let (_, v) = hit(app, Case::new("get", "/v1/mutes").signed(&user).await).await;
    assert_eq!(
        v["data"]["results"][0]["address"],
        other.address.as_str(),
        "{v}"
    );

    let (status, v) = hit(
        app,
        Case::new("post", "/v1/mutes")
            .signed(&user)
            .await
            .json(&json!({ "muted_address": user.address.as_str() })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v, bad_request("Cannot mute yourself"));

    let (status, v) = hit(
        app,
        Case::new("delete", "/v1/mutes")
            .signed(&user)
            .await
            .json(&json!({ "muted_address": other.address.as_str() })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");
    let (_, v) = hit(app, Case::new("get", "/v1/mutes").signed(&user).await).await;
    assert_eq!(v["data"]["total"], 0, "{v}");
    rig.finish();
}
