//! Optional legacy SQS ingress and SNS egress, inside the existing serve lifecycle.
//! Wire events remain the source's raw JSON Events. Its queue consumer deletes
//! in finally, including malformed/unknown events and failed domain handlers.
use crate::communities;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub sqs_endpoint: Option<String>,
    pub sqs_queue_url: Option<String>,
    pub sns_endpoint: Option<String>,
    pub sns_arn: Option<String>,
}

pub struct LegacyEvents {
    queue: Option<SqsQueue>,
    publisher: Option<(aws_sdk_sns::Client, String)>,
}

impl LegacyEvents {
    /// With neither source transport configured this returns before resolving
    /// the AWS credential chain, regions or IMDS endpoints.
    pub async fn new(mut settings: Settings) -> anyhow::Result<Option<Arc<Self>>> {
        settings.sqs_endpoint = settings.sqs_endpoint.filter(|value| !value.is_empty());
        settings.sns_arn = settings.sns_arn.filter(|value| !value.is_empty());
        let queue_enabled = settings.sqs_endpoint.is_some();
        if !queue_enabled && settings.sns_arn.is_none() {
            return Ok(None);
        }
        if queue_enabled {
            anyhow::ensure!(
                settings.sqs_queue_url.is_some(),
                "AWS_SQS_QUEUE_URL is required when AWS_SQS_ENDPOINT is configured"
            );
        }
        let sdk = tokio::time::timeout(
            Duration::from_secs(10),
            aws_config::defaults(aws_config::BehaviorVersion::latest())
                .timeout_config(
                    aws_config::timeout::TimeoutConfig::builder()
                        .connect_timeout(Duration::from_secs(3))
                        .read_timeout(Duration::from_secs(25))
                        .operation_timeout(Duration::from_secs(30))
                        .build(),
                )
                .retry_config(aws_config::retry::RetryConfig::standard().with_max_attempts(3))
                .load(),
        )
        .await?;
        let queue = if queue_enabled {
            let mut builder = aws_sdk_sqs::config::Builder::from(&sdk);
            if let Some(endpoint) = settings.sqs_endpoint {
                builder = builder.endpoint_url(endpoint);
            }
            Some(SqsQueue {
                client: aws_sdk_sqs::Client::from_conf(builder.build()),
                url: settings.sqs_queue_url.unwrap(),
            })
        } else {
            None
        };
        let publisher = settings.sns_arn.map(|arn| {
            let mut builder = aws_sdk_sns::config::Builder::from(&sdk);
            if let Some(endpoint) = settings.sns_endpoint {
                builder = builder.endpoint_url(endpoint);
            }
            (aws_sdk_sns::Client::from_conf(builder.build()), arn)
        });
        Ok(Some(Arc::new(Self { queue, publisher })))
    }
    #[cfg(test)]
    pub(crate) fn test_publisher(endpoint: &str) -> Arc<Self> {
        let client = aws_sdk_sns::Client::from_conf(
            aws_sdk_sns::config::Builder::new()
                .behavior_version_latest()
                .region(aws_sdk_sns::config::Region::new("us-east-1"))
                .credentials_provider(aws_sdk_sns::config::Credentials::new(
                    "test-access",
                    "test-secret",
                    None,
                    None,
                    "fixture",
                ))
                .retry_config(aws_config::retry::RetryConfig::standard().with_max_attempts(1))
                .endpoint_url(endpoint)
                .build(),
        );
        Arc::new(Self {
            queue: None,
            publisher: Some((client, "arn:aws:sns:us-east-1:123456789012:fixture".into())),
        })
    }

    pub async fn publish(&self, event: &Value) -> anyhow::Result<()> {
        let Some((client, arn)) = &self.publisher else {
            anyhow::bail!("AWS_SNS_ARN is not configured");
        };
        let attribute = |value: &str| {
            aws_sdk_sns::types::MessageAttributeValue::builder()
                .data_type("String")
                .string_value(value)
                .build()
        };
        let mut publish = client
            .publish()
            .topic_arn(arn)
            .message(serde_json::to_string(event)?)
            .message_attributes(
                "type",
                attribute(
                    event["type"]
                        .as_str()
                        .filter(|kind| !kind.is_empty())
                        .unwrap_or("unknown"),
                )?,
            );
        if let Some(subtype) = event["subType"].as_str() {
            publish = publish.message_attributes("subType", attribute(subtype)?);
        }
        tokio::time::timeout(Duration::from_secs(30), publish.send()).await??;
        Ok(())
    }
    pub fn spawn(
        self: &Arc<Self>,
        jobs: &crate::jobs::Jobs,
        state: communities::SharedState,
        writes_enabled: bool,
    ) {
        if self.queue.is_none() || !writes_enabled {
            return;
        }
        let events = self.clone();
        jobs.spawn_leased("social-legacy-events",move|ctx| {
            let events=events.clone();let state=state.clone();async move {
                let queue=events.queue.as_ref().unwrap();
                let mut failures=0_u32;
                loop {
                    if crate::shared::writes::refusal(writes_enabled).is_some() {
                        tokio::select! { _=ctx.cancel.cancelled()=>return Ok(()),_=tokio::time::sleep(Duration::from_secs(1))=>continue }
                    }
                    let batch=tokio::select! { _=ctx.cancel.cancelled()=>return Ok(()),batch=queue.receive()=>batch };
                    match batch {
                        Ok(messages)=>{
                            failures=0;
                            for (index,message) in messages.iter().enumerate() {
                                if ctx.is_cancelled() {
                                    for pending in &messages[index..] {
                                        if let Some(receipt)=pending.receipt.as_deref() {
                                            let _=tokio::time::timeout(Duration::from_secs(5),queue.release(receipt)).await;
                                        }
                                    }
                                    return Ok(());
                                }
                                process_message(queue,&state,message).await;
                            }
                            ctx.pass_done();
                        }
                        Err(error)=>{
                            failures=failures.saturating_add(1);ctx.error(format!("SQS receive failed: {error}"));
                            let ceiling=1000_u64.saturating_mul(1_u64.checked_shl(failures.saturating_sub(1).min(5)).unwrap_or(32)).min(30000);
                            let jitter=uuid::Uuid::new_v4().as_u128() as u64 % (ceiling/2).max(1);
                            tokio::select!{_=ctx.cancel.cancelled()=>return Ok(()),_=tokio::time::sleep(Duration::from_millis(ceiling/2+jitter))=>{}}
                        }
                    }
                }
            }
        });
    }
}

struct QueueMessage {
    body: Option<String>,
    receipt: Option<String>,
}
#[async_trait::async_trait]
trait Queue: Send + Sync {
    async fn receive(&self) -> anyhow::Result<Vec<QueueMessage>>;
    async fn delete(&self, receipt: &str) -> anyhow::Result<()>;
    async fn release(&self, receipt: &str) -> anyhow::Result<()>;
}
struct SqsQueue {
    client: aws_sdk_sqs::Client,
    url: String,
}
#[async_trait::async_trait]
impl Queue for SqsQueue {
    async fn receive(&self) -> anyhow::Result<Vec<QueueMessage>> {
        let result = self
            .client
            .receive_message()
            .queue_url(&self.url)
            .max_number_of_messages(10)
            .visibility_timeout(60)
            .wait_time_seconds(20)
            .send()
            .await?;
        Ok(result
            .messages
            .unwrap_or_default()
            .into_iter()
            .map(|message| QueueMessage {
                body: message.body,
                receipt: message.receipt_handle,
            })
            .collect())
    }
    async fn delete(&self, receipt: &str) -> anyhow::Result<()> {
        self.client
            .delete_message()
            .queue_url(&self.url)
            .receipt_handle(receipt)
            .send()
            .await?;
        Ok(())
    }
    async fn release(&self, receipt: &str) -> anyhow::Result<()> {
        self.client
            .change_message_visibility()
            .queue_url(&self.url)
            .receipt_handle(receipt)
            .visibility_timeout(0)
            .send()
            .await?;
        Ok(())
    }
}
async fn process_message(queue: &dyn Queue, state: &communities::State, message: &QueueMessage) {
    match serde_json::from_str::<Value>(message.body.as_deref().unwrap_or_default()) {
        Ok(event) => {
            if let Err(error) = dispatch(state, &event).await {
                tracing::error!(?error, "Legacy social event handler failed");
            }
        }
        Err(error) => tracing::error!(%error,"Malformed legacy social event"),
    }
    if let Some(receipt) = message.receipt.as_deref() {
        if let Err(error) = queue.delete(receipt).await {
            tracing::error!(%error,"Legacy SQS delete failed");
        }
    }
}
/// Direct decoded-event seam shared by legacy transport and source captures.
pub async fn dispatch(state: &communities::State, event: &Value) -> anyhow::Result<bool> {
    if event["type"] == "client"
        && matches!(
            event["subType"].as_str(),
            Some("logged-in" | "logged-in-cached")
        )
    {
        let Some(address) = event["metadata"]["userAddress"]
            .as_str()
            .filter(|value| !value.is_empty())
        else {
            tracing::error!("Legacy login event has no userAddress");
            return Ok(true);
        };
        state
            .referral
            .finalize_referral(address)
            .await
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        return Ok(true);
    }
    Ok(communities::ranking::apply_external_event(&state.pool, event).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn official_sdk_keeps_source_queue_parameters_and_sns_attributes() {
        use axum::response::IntoResponse;
        let requests = Arc::new(parking_lot::Mutex::new(Vec::<(
            axum::http::HeaderMap,
            String,
        )>::new()));
        let captured = requests.clone();
        let app=axum::Router::new().fallback(move|headers:axum::http::HeaderMap,body:axum::body::Bytes|{let captured=captured.clone();async move{
            let text=String::from_utf8(body.to_vec()).unwrap();let target=headers.get("x-amz-target").and_then(|value|value.to_str().ok()).unwrap_or_default().to_string();captured.lock().push((headers,text));
            if target.ends_with("ReceiveMessage") {(axum::http::StatusCode::OK,[("content-type","application/x-amz-json-1.0")],"{\"Messages\":[{\"Body\":\"{}\",\"ReceiptHandle\":\"receipt\"}]}").into_response()}
            else if target.contains("SQS") {(axum::http::StatusCode::OK,[("content-type","application/x-amz-json-1.0")],"{}").into_response()}
            else {(axum::http::StatusCode::OK,[("content-type","text/xml")],"<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>test-message</MessageId></PublishResult><ResponseMetadata><RequestId>test-request</RequestId></ResponseMetadata></PublishResponse>").into_response()}
        }});
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let credentials = || {
            aws_sdk_sqs::config::Credentials::new(
                "test-access",
                "test-secret",
                None,
                None,
                "test-fixture",
            )
        };
        let sqs = aws_sdk_sqs::Client::from_conf(
            aws_sdk_sqs::config::Builder::new()
                .behavior_version_latest()
                .region(aws_sdk_sqs::config::Region::new("us-east-1"))
                .credentials_provider(credentials())
                .endpoint_url(&base)
                .build(),
        );
        let sns = aws_sdk_sns::Client::from_conf(
            aws_sdk_sns::config::Builder::new()
                .behavior_version_latest()
                .region(aws_sdk_sns::config::Region::new("us-east-1"))
                .credentials_provider(credentials())
                .endpoint_url(&base)
                .build(),
        );
        let events = LegacyEvents {
            queue: Some(SqsQueue {
                client: sqs,
                url: format!("{base}/fixture-queue"),
            }),
            publisher: Some((sns, "arn:aws:sns:us-east-1:123456789012:fixture".into())),
        };
        let queue = events.queue.as_ref().unwrap();
        let messages = queue.receive().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].receipt.as_deref(), Some("receipt"));
        queue.delete("receipt").await.unwrap();
        queue.release("receipt").await.unwrap();
        let event = serde_json::json!({"type":"referral","subType":"referral-invited-users-accepted","key":"fixture","timestamp":1,"metadata":{"address":"fixture"}});
        events.publish(&event).await.unwrap();
        let captured = requests.lock();
        let receive: Value = serde_json::from_str(&captured[0].1).unwrap();
        assert_eq!(receive["MaxNumberOfMessages"], 10);
        assert_eq!(receive["VisibilityTimeout"], 60);
        assert_eq!(receive["WaitTimeSeconds"], 20);
        assert!(captured.iter().all(|(headers, _)| {
            headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("AWS4-HMAC-SHA256 ")
        }));
        let fields = url::form_urlencoded::parse(captured.last().unwrap().1.as_bytes())
            .into_owned()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(fields["Action"], "Publish");
        assert_eq!(
            serde_json::from_str::<Value>(&fields["Message"]).unwrap(),
            event
        );
        assert!(fields.values().any(|value| value == "referral"));
        assert!(
            fields
                .values()
                .any(|value| value == "referral-invited-users-accepted")
        );
        server.abort();
    }

    #[tokio::test]
    async fn raw_queue_processing_matches_source_acknowledgements_even_on_handler_failure() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let fixture: Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/referral-cases.json"
        )))
        .unwrap();
        struct FakeQueue(parking_lot::Mutex<Vec<String>>);
        #[async_trait::async_trait]
        impl Queue for FakeQueue {
            async fn receive(&self) -> anyhow::Result<Vec<QueueMessage>> {
                unreachable!()
            }
            async fn delete(&self, receipt: &str) -> anyhow::Result<()> {
                self.0.lock().push(receipt.into());
                if receipt == "handler-error" {
                    anyhow::bail!("injected delete failure")
                }
                Ok(())
            }
            async fn release(&self, _: &str) -> anyhow::Result<()> {
                unreachable!()
            }
        }
        let dir =
            std::env::temp_dir().join(format!("social-legacy-events-{}", uuid::Uuid::new_v4()));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let mut settings = communities::Settings::for_tests();
        settings.referral.min_login_days = 0;
        let state = communities::State::new(
            pool.clone(),
            storage,
            settings,
            None,
            &crate::http::Outbound::for_tests(),
            communities::State::explore_for_tests(&pool),
        );
        let address = || {
            crate::Address::parse(&format!("0x{}00000000", uuid::Uuid::new_v4().simple())).unwrap()
        };
        let (referrer, invited, wrapped) = (address(), address(), address());
        for user in [&invited, &wrapped] {
            state
                .referral
                .create(&referrer, user, "192.0.2.5")
                .await
                .unwrap();
        }
        let queue = FakeQueue(Default::default());
        for case in fixture["queue_cases"].as_array().unwrap() {
            let body = case["body"].as_str().unwrap().replace(
                "0x0000000000000000000000000000000000000002",
                if case["receipt"] == "sns-envelope" {
                    wrapped.as_str()
                } else {
                    invited.as_str()
                },
            );
            process_message(
                &queue,
                &state,
                &QueueMessage {
                    body: Some(body),
                    receipt: case["receipt"].as_str().map(str::to_string),
                },
            )
            .await;
        }
        let expected = fixture["queue_effects"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|effect| effect["kind"] == "delete")
            .map(|effect| effect["receipt"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(*queue.0.lock(), expected);
        let rows:Vec<(String,String)>=sqlx::query_as("SELECT invited_user,status FROM social.referral_progress WHERE invited_user=ANY($1) ORDER BY invited_user").bind(vec![invited.to_string(),wrapped.to_string()]).fetch_all(&pool).await.unwrap();
        assert!(rows.contains(&(invited.to_string(), "tier_granted".into())));
        assert!(
            rows.contains(&(wrapped.to_string(), "pending".into())),
            "Source queue does not unwrap SNS envelopes"
        );
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE address=$1 AND type='referral_invited_users_accepted'").bind(referrer).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn queue_dispatch_updates_all_three_source_ranking_kinds_before_acknowledgement() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        struct Acks(parking_lot::Mutex<Vec<String>>);
        #[async_trait::async_trait]
        impl Queue for Acks {
            async fn receive(&self) -> anyhow::Result<Vec<QueueMessage>> {
                unreachable!()
            }
            async fn delete(&self, receipt: &str) -> anyhow::Result<()> {
                self.0.lock().push(receipt.into());
                Ok(())
            }
            async fn release(&self, _: &str) -> anyhow::Result<()> {
                unreachable!()
            }
        }
        let dir =
            std::env::temp_dir().join(format!("social-ranking-queue-{}", uuid::Uuid::new_v4()));
        let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
        let state = communities::State::new(
            pool.clone(),
            storage,
            communities::Settings::for_tests(),
            None,
            &crate::http::Outbound::for_tests(),
            communities::State::explore_for_tests(&pool),
        );
        let community = uuid::Uuid::new_v4();
        let place = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO social.communities(id,name,owner_address) VALUES($1,'ranking fixture','fixture-owner')").bind(community).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO social.community_places(community_id,place_id,added_by) VALUES($1,$2,'fixture-owner')")
            .bind(community)
            .bind(&place)
            .execute(&pool)
            .await
            .unwrap();
        let queue = Acks(Default::default());
        for (receipt, event) in [
            (
                "photo",
                serde_json::json!({"type":"camera","subType":"photo-taken","metadata":{"placeId":place}}),
            ),
            (
                "event",
                serde_json::json!({"type":"event","subType":"event-ended","metadata":{"communityId":community,"totalAttendees":4.9}}),
            ),
            (
                "stream",
                serde_json::json!({"type":"streaming","subType":"community-streaming-ended","metadata":{"communityId":community,"totalParticipants":3.8}}),
            ),
            (
                "failure",
                serde_json::json!({"type":"event","subType":"event-ended","metadata":{"communityId":"invalid"}}),
            ),
        ] {
            process_message(
                &queue,
                &state,
                &QueueMessage {
                    body: Some(event.to_string()),
                    receipt: Some(receipt.into()),
                },
            )
            .await;
        }
        assert_eq!(*queue.0.lock(), ["photo", "event", "stream", "failure"]);
        let counts:(i32,i32,i32,i32,i32)=sqlx::query_as("SELECT ranking_events_count,ranking_photos_count,ranking_streams_count,ranking_events_total_attendees,ranking_streams_total_participants FROM social.communities WHERE id=$1").bind(community).fetch_one(&pool).await.unwrap();
        assert_eq!(counts, (1, 1, 1, 4, 3));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn an_unconfigured_consumer_does_not_load_aws_defaults() {
        assert!(
            LegacyEvents::new(Settings {
                sqs_endpoint: Some(String::new()),
                sns_arn: Some(String::new()),
                ..Default::default()
            })
            .await
            .unwrap()
            .is_none()
        );
        assert!(
            tokio::time::timeout(
                Duration::from_millis(100),
                LegacyEvents::new(Settings::default())
            )
            .await
            .unwrap()
            .unwrap()
            .is_none()
        );
    }
}
