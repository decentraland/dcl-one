use super::*;
use axum::response::IntoResponse;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn sns_batches_bound_requests_await_every_delivery_and_keep_input_result_order() {
    let release = Arc::new(
        (0..12)
            .map(|_| tokio::sync::Notify::new())
            .collect::<Vec<_>>(),
    );
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let (entered, mut entries) = tokio::sync::mpsc::unbounded_channel();
    let (finished, mut finishes) = tokio::sync::mpsc::unbounded_channel();
    let (gates, current, maximum) = (release.clone(), active.clone(), peak.clone());
    let app=axum::Router::new().fallback(move |body:axum::body::Bytes| {let gates=gates.clone();let active=current.clone();let peak=maximum.clone();let entered=entered.clone();let finished=finished.clone();async move {
        let fields=url::form_urlencoded::parse(&body).into_owned().collect::<std::collections::HashMap<_,_>>();
        assert_eq!(fields["Action"],"PublishBatch");
        let first:Value=serde_json::from_str(&fields["PublishBatchRequestEntries.member.1.Message"]).unwrap();let batch=first["key"].as_u64().unwrap() as usize/10;
        let live=active.fetch_add(1,Ordering::SeqCst)+1;peak.fetch_max(live,Ordering::SeqCst);entered.send(batch).unwrap();
        gates[batch].notified().await;active.fetch_sub(1,Ordering::SeqCst);finished.send(batch).unwrap();
        if batch==2 {return (axum::http::StatusCode::BAD_REQUEST,[("content-type","text/xml")],"<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture batch failed</Message></Error><RequestId>fixture</RequestId></ErrorResponse>".to_owned()).into_response();}
        let failed=if batch==5 {"<member><Id>msg_1</Id><Code>Fixture</Code><Message>rejected</Message><SenderFault>true</SenderFault></member>"}else{""};
        ([("content-type","text/xml")],format!("<PublishBatchResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishBatchResult><Successful><member><Id>msg_0</Id><MessageId>batch-{batch}</MessageId></member></Successful><Failed>{failed}</Failed></PublishBatchResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishBatchResponse>")).into_response()
    }});
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let publisher = LegacyEvents::test_publisher(&endpoint);
    let events = (0..120)
        .map(|key| serde_json::json!({"type":"community","key":key}))
        .collect::<Vec<_>>();
    let task = tokio::spawn(async move { publisher.publish_batch(&events).await });
    let mut initial = Vec::new();
    for _ in 0..8 {
        initial.push(
            tokio::time::timeout(Duration::from_secs(2), entries.recv())
                .await
                .unwrap()
                .unwrap(),
        );
    }
    initial.sort();
    assert_eq!(initial, (0..8).collect::<Vec<_>>());
    assert!(
        tokio::time::timeout(Duration::from_millis(30), entries.recv())
            .await
            .is_err(),
        "no ninth request is dispatched while eight are in flight"
    );
    for batch in 1..8 {
        release[batch].notify_one();
    }
    for _ in 0..7 {
        finishes.recv().await.unwrap();
    }
    assert!(
        !task.is_finished(),
        "an early failed chunk cannot end other deliveries"
    );
    release[0].notify_one();
    for _ in 0..4 {
        let batch = tokio::time::timeout(Duration::from_secs(2), entries.recv())
            .await
            .unwrap()
            .unwrap();
        assert!((8..12).contains(&batch));
    }
    assert!(!task.is_finished());
    for batch in 8..12 {
        release[batch].notify_one();
    }
    let result = tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(peak.load(Ordering::SeqCst), 8);
    assert_eq!(
        result.successful_message_ids,
        (0..12)
            .filter(|batch| *batch != 2)
            .map(|batch| format!("batch-{batch}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        result
            .failed_events
            .iter()
            .map(|event| event["key"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        (20..30).chain(std::iter::once(51)).collect::<Vec<_>>()
    );
    server.abort();
}
