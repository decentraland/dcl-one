use super::core_http_snapshot;
use super::native::Pressure;
use crate::http::{fetch, fetch_with_permit, HttpAttemptPermit};
use futures_util::future::{select, Either};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn pressure_starts_at_fifteen_and_restarts_only_on_completion() {
    let mut pressure = Pressure::default();
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let mut ids: Vec<_> = (0..14).map(|_| pressure.begin(at(0))).collect();
    assert!(!pressure.snapshot(at(20_000)).suspected_starvation);

    ids.push(pressure.begin(at(20_000)));
    ids.push(pressure.begin(at(23_000)));
    assert!(!pressure.snapshot(at(24_999)).suspected_starvation);
    let stalled = pressure.snapshot(at(25_000));
    assert!(stalled.suspected_starvation);
    assert_eq!(stalled.active, 16);
    assert_eq!(stalled.high_water, 16);
    assert_eq!(stalled.oldest_active_ms, 25_000);

    pressure.end(ids.pop().unwrap(), at(26_000));
    assert!(!pressure.snapshot(at(30_999)).suspected_starvation);
    assert!(pressure.snapshot(at(31_000)).suspected_starvation);
    pressure.end(ids.pop().unwrap(), at(31_000));
    assert!(!pressure.snapshot(at(60_000)).suspected_starvation);
    ids.push(pressure.begin(at(60_000)));
    assert!(!pressure.snapshot(at(63_999)).suspected_starvation);

    // Completions between snapshots count, even when active and high water look unchanged.
    for ms in [64_000, 68_000, 72_000] {
        pressure.end(ids.remove(0), at(ms));
        ids.push(pressure.begin(at(ms)));
    }
    assert!(!pressure.snapshot(at(75_000)).suspected_starvation);
    assert!(pressure.snapshot(at(77_000)).suspected_starvation);
    for id in ids {
        pressure.end(id, at(78_000));
    }
    let idle = pressure.snapshot(at(90_000));
    assert_eq!(idle.active, 0);
    assert_eq!(idle.oldest_active_ms, 0);
    assert_eq!(idle.high_water, 16);
    assert!(!idle.suspected_starvation);
}

struct Admitted;
impl HttpAttemptPermit for Admitted {}

#[test]
fn ordinary_transfers_start_immediately_and_admitted_attempts_stay_out_of_the_detector() {
    async_std::task::block_on(async_compat::Compat::new(async {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (finish, resume) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let mut sockets = Vec::new();
            for _ in 0..17 {
                let (mut socket, _) = listener.accept().unwrap();
                for line in BufReader::new(&socket).lines() {
                    if line.unwrap().is_empty() {
                        break;
                    }
                }
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nb")
                    .unwrap();
                sockets.push(socket);
            }
            if resume.recv().is_ok() {
                let _ = sockets[0].write_all(b"ody");
                // Closing the second socket leaves its body truncated.
            }
        });

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let limit = Duration::from_secs(5);
        let mut ordinary = Vec::new();
        for count in 1..=16 {
            let (headers, arrived) = async_std::channel::bounded(1);
            let request = client.get(&url);
            let send = async move {
                let response = request.send().await?;
                headers.try_send(()).unwrap();
                Ok::<_, reqwest::Error>(response)
            };
            let mut transfer = Box::pin(fetch(send, limit, limit));
            let ready =
                async_std::future::timeout(limit, select(Box::pin(arrived.recv()), &mut transfer))
                    .await
                    .expect("request waited for an earlier response body");
            assert!(matches!(ready, Either::Left((Ok(()), _))));
            assert_eq!(core_http_snapshot().active, count);
            ordinary.push(transfer);
        }

        // The IPFS path uses the same transport after acquiring its own permit.
        let (chunk, received) = async_std::channel::bounded(1);
        let mut admitted = Box::pin(fetch_with_permit(
            Admitted,
            || client.get(&url).send(),
            limit,
            limit,
            |_| {
                let _ = chunk.try_send(());
            },
        ));
        let ready =
            async_std::future::timeout(limit, select(Box::pin(received.recv()), &mut admitted))
                .await
                .expect("admitted attempt waited for ordinary transfers");
        assert!(matches!(ready, Either::Left((Ok(()), _))));
        assert_eq!(core_http_snapshot().active, 16);
        drop(admitted);

        let success = ordinary.remove(0);
        let truncated = ordinary.remove(0);
        drop(ordinary);
        assert_eq!(core_http_snapshot().active, 2);
        finish.send(()).unwrap();
        assert_eq!(success.await.ok().unwrap().body, b"body");
        assert_eq!(core_http_snapshot().active, 1);
        assert!(matches!(truncated.await, Err(crate::FetchError::Body(_))));
        server.join().unwrap();
        assert_eq!(core_http_snapshot().active, 0);

        let pending = std::future::pending::<Result<reqwest::Response, ()>>();
        let mut before_headers = Box::pin(fetch(pending, limit, limit));
        assert!(futures_util::poll!(&mut before_headers).is_pending());
        assert_eq!(core_http_snapshot().active, 1);
        drop(before_headers);
        assert_eq!(core_http_snapshot().active, 0);
        assert!(fetch(client.get(&url).send(), limit, limit).await.is_err());
        assert_eq!(core_http_snapshot().active, 0);
    }));
}
