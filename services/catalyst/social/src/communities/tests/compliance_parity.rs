use super::super::compliance::{self, Content};
use super::*;
use axum::Json;
use axum::response::IntoResponse;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

fn goldens() -> Value {
    serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-ai-compliance.json"))).unwrap()
}
fn content<'a>(case: &'a Value, bytes: Option<&'a [u8]>) -> Content<'a> {
    Content {
        name: case["request"]["name"].as_str(),
        description: case["request"]["description"].as_str(),
        thumbnail: bytes,
    }
}
#[test]
fn provider_requests_and_cache_hashes_match_executed_sdk_source() {
    for case in goldens()["cases"].as_array().unwrap() {
        let bytes = case["request"]["thumbnailBuffer"]
            .as_str()
            .map(|v| STANDARD.decode(v).unwrap());
        let content = content(case, bytes.as_deref());
        let model = case["model"].as_str().unwrap();
        if case.get("providerRequest").is_some() {
            assert_eq!(
                compliance::request_body(model, content),
                case["providerRequest"],
                "{}",
                case["name"]
            );
        }
        if let Some(key) = case["cacheKey"].as_str() {
            assert_eq!(
                format!(
                    "ai-compliance:{model}:{}",
                    hex::encode(compliance::cache_hash(content))
                ),
                key,
                "{}",
                case["name"]
            );
        }
    }
    assert!(!compliance::changed(" \u{feff}NAME ", "name"));
    assert!(compliance::changed("\u{0085}name", "name"));
}
struct Provider {
    url: String,
    task: tokio::task::JoinHandle<()>,
    calls: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<Value>>>,
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn provider(case: &Value, delay: std::time::Duration) -> Provider {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let bodies = Arc::new(Mutex::new(Vec::new()));
    if case["network"].as_bool() == Some(true) {
        let count = calls.clone();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                count.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });
        return Provider {
            url,
            task,
            calls,
            bodies,
        };
    }
    let body = case["providerResponse"].clone();
    let status =
        StatusCode::from_u16(case["providerStatus"].as_u64().unwrap_or(200) as u16).unwrap();
    let count = calls.clone();
    let captured = bodies.clone();
    let app = Router::new().route(
        "/v1/responses",
        axum::routing::post(
            move |headers: axum::http::HeaderMap, Json(request): Json<Value>| {
                let count = count.clone();
                let captured = captured.clone();
                let body = body.clone();
                async move {
                    assert_eq!(headers["authorization"], "Bearer fixture-key");
                    assert_eq!(headers["x-stainless-timeout"], "600");
                    count.fetch_add(1, Ordering::SeqCst);
                    captured.lock().unwrap().push(request);
                    tokio::time::sleep(delay).await;
                    (status, [("retry-after-ms", "1")], Json(body))
                }
            },
        ),
    );
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Provider {
        url,
        task,
        calls,
        bodies,
    }
}
fn enabled_settings(url: &str) -> Settings {
    let mut settings = Settings::for_tests();
    settings.compliance.environment = "prd".into();
    settings.compliance.base_url = url.into();
    settings.compliance.api_key = Some("fixture-key".into());
    settings
        .features
        .overrides
        .insert(super::super::features::AI_COMPLIANCE.into(), "1".into());
    settings
}
#[tokio::test]
async fn provider_classification_retries_environment_and_cached_rejection_match_source() {
    for case in goldens()["cases"].as_array().unwrap() {
        let provider = provider(case, std::time::Duration::ZERO).await;
        let mut settings = enabled_settings(&provider.url);
        settings.compliance.environment = case["environment"].as_str().unwrap().into();
        settings.compliance.model = case["model"].as_str().unwrap().into();
        settings.features.overrides.insert(
            super::super::features::DEV_AI_COMPLIANCE.into(),
            if case["dev"].as_bool().unwrap() {
                "1"
            } else {
                "0"
            }
            .into(),
        );
        let Some(rig) = Rig::new(settings).await else {
            return;
        };
        let bytes = case["request"]["thumbnailBuffer"]
            .as_str()
            .map(|v| STANDARD.decode(v).unwrap());
        let input = content(case, bytes.as_deref());
        let hash = compliance::cache_hash(input);
        if let Some(cached) = case.get("cached") {
            sqlx::query("INSERT INTO social.community_compliance_cache(model,input_hash,result,expires_at) VALUES($1,$2,$3,timezone('UTC',now())+interval '2 hours') ON CONFLICT(model,input_hash) DO UPDATE SET result=EXCLUDED.result,expires_at=EXCLUDED.expires_at").bind(&rig.state.settings.compliance.model).bind(hash.as_slice()).bind(cached).execute(&rig.pool).await.unwrap();
        }
        let result = rig.state.validate_content(input).await;
        if case["httpResult"]["status"] == 200 {
            assert!(result.is_ok(), "{}: {:?}", case["name"], result);
        } else {
            let response = result.unwrap_err().into_response();
            assert_eq!(
                response.status().as_u16(),
                case["httpResult"]["status"].as_u64().unwrap() as u16,
                "{}",
                case["name"]
            );
            let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap(),
                case["httpResult"]["body"],
                "{}",
                case["name"]
            );
        }
        assert_eq!(
            provider.calls.load(Ordering::SeqCst),
            case["attempts"].as_u64().unwrap() as usize,
            "{}",
            case["name"]
        );
        for body in provider.bodies.lock().unwrap().iter() {
            assert_eq!(body, &case["providerRequest"], "{}", case["name"]);
        }
        if let Some(expected) = case.get("cachedPut") {
            let cached:Value=sqlx::query_scalar("SELECT result FROM social.community_compliance_cache WHERE model=$1 AND input_hash=$2").bind(&rig.state.settings.compliance.model).bind(hash.as_slice()).fetch_one(&rig.pool).await.unwrap();
            assert_eq!(&cached, expected);
            rig.state.validate_content(input).await.ok();
            assert_eq!(
                provider.calls.load(Ordering::SeqCst),
                case["attempts"].as_u64().unwrap() as usize,
                "cached full provider result prevents a second request"
            );
            let ttl:f64=sqlx::query_scalar("SELECT extract(epoch FROM expires_at-timezone('UTC',now()))::float8 FROM social.community_compliance_cache WHERE model=$1 AND input_hash=$2").bind(&rig.state.settings.compliance.model).bind(hash.as_slice()).fetch_one(&rig.pool).await.unwrap();
            assert!((7190.0..=7200.0).contains(&ttl));
            sqlx::query("UPDATE social.community_compliance_cache SET expires_at=timezone('UTC',now())-interval '1 second' WHERE model=$1 AND input_hash=$2").bind(&rig.state.settings.compliance.model).bind(hash.as_slice()).execute(&rig.pool).await.unwrap();
            rig.state.validate_content(input).await.ok();
            assert_eq!(
                provider.calls.load(Ordering::SeqCst),
                2,
                "an expired cached decision goes back to the provider"
            );
        }
        sqlx::query(
            "DELETE FROM social.community_compliance_cache WHERE model=$1 AND input_hash=$2",
        )
        .bind(&rig.state.settings.compliance.model)
        .bind(hash.as_slice())
        .execute(&rig.pool)
        .await
        .unwrap();
        rig.finish();
    }
}
#[tokio::test]
async fn valid_slow_provider_outlives_the_shared_readers_twenty_five_second_budget() {
    let case = goldens()["cases"][0].clone();
    let provider = provider(&case, std::time::Duration::from_secs(26)).await;
    let Some(rig) = Rig::new(enabled_settings(&provider.url)).await else {
        return;
    };
    let name = format!("slow-{}", Uuid::new_v4());
    rig.state
        .validate_content(Content {
            name: Some(&name),
            description: None,
            thumbnail: None,
        })
        .await
        .unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    rig.finish();
}
#[tokio::test]
async fn enabled_compliance_rejects_before_community_or_thumbnail_writes_and_skips_unchanged_text()
{
    let case = goldens()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "noncompliant")
        .unwrap()
        .clone();
    let provider = provider(&case, std::time::Duration::ZERO).await;
    let Some(rig) = Rig::new(enabled_settings(&provider.url)).await else {
        return;
    };
    let owner = wallet();
    rig.profile(&owner, "Owner", true).await;
    let name = format!("Reject {}", &Uuid::new_v4().simple().to_string()[..8]);
    let (body, ctype) = multipart(&[
        ("name", name.as_bytes()),
        ("description", b"Rejected description"),
        ("thumbnail", &png(1100)),
    ]);
    let response = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(body, &ctype),
    )
    .await;
    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    assert_eq!(response.1, case["httpResult"]["body"]);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM social.communities WHERE owner_address=$1")
            .bind(&owner.address)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let id = rig.community(&owner, false).await;
    let current: String = sqlx::query_scalar("SELECT name FROM social.communities WHERE id=$1")
        .bind(id)
        .fetch_one(&rig.pool)
        .await
        .unwrap();
    let case_only = format!(" {} ", current.to_uppercase());
    let (body, ctype) = multipart(&[("name", case_only.as_bytes())]);
    let path = format!("/v1/communities/{id}");
    let response = hit(
        &rig.app,
        Case::new("put", &path)
            .signed(&owner)
            .await
            .body(body, &ctype),
    )
    .await;
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        1,
        "case and source-trim only edits skip provider"
    );
    let (body, ctype) = multipart(&[("description", b"Changed description")]);
    let response = hit(
        &rig.app,
        Case::new("put", &path)
            .signed(&owner)
            .await
            .body(body, &ctype),
    )
    .await;
    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    let retained: String =
        sqlx::query_scalar("SELECT description FROM social.communities WHERE id=$1")
            .bind(id)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(retained, "seeded community");
    rig.finish();
}

#[tokio::test]
async fn cache_read_failure_is_generic_but_cache_write_failure_is_ai_unavailable() {
    let case = goldens()["cases"][0].clone();
    let provider = provider(&case, std::time::Duration::ZERO).await;
    let Some(rig) = Rig::new(enabled_settings(&provider.url)).await else {
        return;
    };
    let recorder = crate::comms::metrics::prometheus_builder()
        .unwrap()
        .build_recorder();
    let handle = recorder.handle();
    let _guard = metrics::set_default_local_recorder(&recorder);
    let name = format!("write-fail-{}", Uuid::new_v4());
    let input = Content {
        name: Some(&name),
        description: None,
        thumbnail: None,
    };
    let hash = hex::encode(compliance::cache_hash(input));
    let function = format!("compliance_failure_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.input_hash=decode('{hash}','hex') THEN RAISE EXCEPTION 'injected cache write failure'; END IF; RETURN NEW; END $$"))).execute(&rig.pool).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER {function} BEFORE INSERT OR UPDATE ON social.community_compliance_cache FOR EACH ROW EXECUTE FUNCTION social.{function}()"))).execute(&rig.pool).await.unwrap();
    let response = rig
        .state
        .validate_content(input)
        .await
        .unwrap_err()
        .into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["communityContentValidationUnavailable"], true);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("injected cache write failure")
    );
    assert_source_metric_counts(&handle.render(), "cache-write-failure");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {function} ON social.community_compliance_cache"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP FUNCTION social.{function}()"
    )))
    .execute(&rig.pool)
    .await
    .unwrap();
    rig.state.validate_content(input).await.unwrap();
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        2,
        "a failed cache write is not a cached provider decision"
    );
    rig.pool.close().await;
    let response = rig
        .state
        .validate_content(Content {
            name: Some("closed-pool"),
            description: None,
            thumbnail: None,
        })
        .await
        .unwrap_err()
        .into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        2,
        "cache read fails before provider invocation"
    );
    rig.finish();
}

fn boundary_metrics_source(scenario: &str) -> Value {
    let source: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/provider-boundary-source-cases.json"
    )))
    .unwrap();
    source["ai_metrics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["scenario"] == scenario)
        .unwrap()
        .clone()
}
fn assert_source_metric_counts(rendered: &str, scenario: &str) {
    let source = boundary_metrics_source(scenario);
    let events = source["events"].as_array().unwrap();
    for result in [
        "compliant",
        "non-compliant",
        "compliant_cached",
        "non-compliant_cached",
        "failed",
    ] {
        let expected = events
            .iter()
            .filter(|event| {
                event["operation"] == "increment" && event["labels"]["result"] == result
            })
            .count();
        let prefix = format!("ai_compliance_validation_total{{result=\"{result}\"}} ");
        let actual = rendered
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .map(|value| value.parse::<usize>().unwrap())
            .unwrap_or(0);
        assert_eq!(actual, expected, "{scenario}: {result}\n{rendered}");
    }
    let expected = events
        .iter()
        .filter(|event| event["operation"] == "observe")
        .count();
    let actual = rendered
        .lines()
        .find_map(|line| line.strip_prefix("ai_compliance_validation_duration_seconds_count "))
        .map(|value| value.parse::<usize>().unwrap())
        .unwrap_or(0);
    assert_eq!(actual, expected, "{scenario}: duration count\n{rendered}");
}

#[tokio::test]
async fn compliance_metrics_follow_source_live_cache_failure_and_read_failure_boundaries() {
    for scenario in [
        "live-compliant",
        "live-non-compliant",
        "cached-compliant",
        "cached-non-compliant",
        "provider-invalid",
        "cache-read-failure",
    ] {
        let mut case = goldens()["cases"][0].clone();
        let result = json!({"isCompliant": !scenario.ends_with("non-compliant"), "issues":{"name":[],"description":[],"image":[]},"confidence":0.9});
        case["providerResponse"]["output"][0]["content"][0]["text"] =
            Value::String(if scenario == "provider-invalid" {
                json!({}).to_string()
            } else {
                result.to_string()
            });
        let provider = provider(&case, std::time::Duration::ZERO).await;
        let Some(rig) = Rig::new(enabled_settings(&provider.url)).await else {
            return;
        };
        let name = format!("metrics-{}", Uuid::new_v4());
        let input = Content {
            name: Some(&name),
            description: None,
            thumbnail: None,
        };
        if scenario.starts_with("cached-") {
            sqlx::query("INSERT INTO social.community_compliance_cache(model,input_hash,result,expires_at) VALUES($1,$2,$3,timezone('UTC',now())+interval '2 hours')").bind(&rig.state.settings.compliance.model).bind(compliance::cache_hash(input).as_slice()).bind(&result).execute(&rig.pool).await.unwrap();
        }
        if scenario == "cache-read-failure" {
            rig.pool.close().await;
        }
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _guard = metrics::set_default_local_recorder(&recorder);
        rig.state.validate_content(input).await.ok();
        assert_source_metric_counts(&handle.render(), scenario);
        rig.finish();
    }
}
