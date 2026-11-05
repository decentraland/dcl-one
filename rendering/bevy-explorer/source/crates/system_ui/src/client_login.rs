use std::time::Duration;

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    base_domain::{self, Service},
    client_login::{retry_delay_ms, retryable_status, DurableLogins, PendingClientLogin},
    util::{reqwest_client, TaskCompat, TaskExt},
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use wallet::Wallet;

pub(super) fn register(app: &mut App) {
    app.init_resource::<ClientLoginDelivery>()
        .add_console_command::<ClientLoginDebug, _>(debug)
        .add_debug_section(DebugSection {
            title: "Client login events",
            query: "/client_login_state",
            fields: &[
                ("Delivery active", "active"),
                ("Pending events", "pending"),
                ("Queue saved", "durable"),
                ("Storage in flight", "storageInFlight"),
                ("Request in flight", "inFlight"),
                ("Attempts", "attempts"),
                ("Result", "result"),
                ("Discarded events", "discarded"),
                ("Retry in milliseconds", "retryInMs"),
            ],
            actions: &[],
            inputs: &[],
        });
}

#[derive(Resource, Default)]
pub(super) struct ClientLoginDelivery {
    queue: DurableLogins,
    storage: Option<Task<Result<Stored, String>>>,
    storage_retry: u64,
    active: Option<String>,
    task: Option<Task<AttemptResult>>,
    event_id: Option<String>,
    attempts: u32,
    next_attempt: u64,
    result: &'static str,
    discarded: usize,
}

enum Stored {
    Loaded(Vec<PendingClientLogin>),
    Saved(Vec<String>),
}

#[derive(Debug, PartialEq, Eq)]
enum AttemptResult {
    Accepted,
    Retry,
    Rejected,
}

fn now_ms() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn endpoint() -> String {
    base_domain::url(Service::SocialApi, "/v1/client-login")
}

impl ClientLoginDelivery {
    pub(super) fn pause(&mut self) {
        self.task = None;
        self.event_id = None;
        self.active = None;
        self.attempts = 0;
        self.next_attempt = 0;
        self.result = "paused";
    }

    pub(super) fn completed(&mut self, wallet: &Wallet) {
        self.pause();
        let Some(address) = wallet.address().filter(|_| !wallet.is_guest()) else {
            return;
        };
        let address = format!("{address:#x}");
        self.discarded += self.queue.push(PendingClientLogin {
            wallet: address.clone(),
            endpoint: endpoint(),
            login_id: uuid::Uuid::new_v4().to_string(),
            occurred_at: now_ms(),
        });
        self.active = Some(address);
        self.result = "queued";
    }

    pub(super) fn drive(&mut self, wallet: &Wallet) {
        let now = now_ms();
        if let Some(mut task) = self.storage.take() {
            match task.complete() {
                None => self.storage = Some(task),
                Some(Ok(Stored::Loaded(stored))) => {
                    self.discarded += self.queue.load(stored);
                }
                Some(Ok(Stored::Saved(ids))) => {
                    self.queue.finish_save(true);
                    self.discarded += self.queue.observe_saved_ids(&ids);
                }
                Some(Err(error)) => {
                    if self.queue.loaded {
                        self.queue.finish_save(false);
                    }
                    warn!("login event storage failed: {error}");
                    self.result = "storage failed";
                    self.storage_retry = now.saturating_add(5_000);
                }
            }
        }
        self.discarded += self.queue.expire(now);
        if self.storage.is_none() && now >= self.storage_retry {
            if !self.queue.loaded {
                self.storage = Some(IoTaskPool::get().spawn_compat(async {
                    let json = platform::client_logins_read().await?;
                    serde_json::from_str(&json)
                        .map(Stored::Loaded)
                        .map_err(|e| e.to_string())
                }));
            } else if let Some(events) = self.queue.begin_save() {
                self.storage = Some(IoTaskPool::get().spawn_compat(async move {
                    let json = serde_json::to_string(&events).map_err(|e| e.to_string())?;
                    let receipt = platform::client_logins_write(json).await?;
                    serde_json::from_str(&receipt)
                        .map(Stored::Saved)
                        .map_err(|e| e.to_string())
                }));
            }
        }
        let Some(active) = self.active.as_deref() else {
            return;
        };
        if wallet.is_guest()
            || wallet.address().map(|a| format!("{a:#x}")).as_deref() != Some(active)
        {
            self.pause();
            return;
        }
        if let Some(mut task) = self.task.take() {
            let Some(result) = task.complete() else {
                self.task = Some(task);
                return;
            };
            match result {
                AttemptResult::Retry => {
                    self.result = "retrying";
                    self.next_attempt = now.saturating_add(retry_delay_ms(
                        self.attempts,
                        uuid::Uuid::new_v4().as_u128() as u64,
                    ));
                }
                AttemptResult::Accepted | AttemptResult::Rejected => {
                    if let Some(id) = self.event_id.as_deref() {
                        self.queue.remove(id);
                    }
                    self.result = if result == AttemptResult::Accepted {
                        "accepted"
                    } else {
                        "rejected"
                    };
                    self.event_id = None;
                    self.attempts = 0;
                    self.next_attempt = 0;
                }
            }
        }
        if !self.queue.ready() {
            return;
        }
        let Some(event) = self
            .queue
            .events
            .iter()
            .find(|event| event.eligible(active, &endpoint(), now))
            .cloned()
        else {
            return;
        };
        if self.event_id.as_ref() != Some(&event.login_id) {
            self.event_id = Some(event.login_id.clone());
            self.attempts = 0;
            self.next_attempt = 0;
        }
        if now < self.next_attempt {
            return;
        }
        self.attempts = self.attempts.saturating_add(1);
        let wallet = wallet.snapshot();
        self.task = Some(IoTaskPool::get().spawn_compat(async move {
            // Includes remote signing and HTTP headers; login never waits for this work.
            async_std::future::timeout(Duration::from_secs(10), send(&wallet, &event))
                .await
                .unwrap_or(AttemptResult::Retry)
        }));
    }
}

async fn send(wallet: &Wallet, event: &PendingClientLogin) -> AttemptResult {
    let Ok(uri) = event.endpoint.parse() else {
        return AttemptResult::Rejected;
    };
    let Ok(headers) = wallet::sign_request("post", &uri, wallet, event.metadata()).await else {
        return AttemptResult::Retry;
    };
    let mut request = reqwest_client().post(&event.endpoint);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    match request.send().await {
        Ok(response) if response.status().as_u16() == 204 => AttemptResult::Accepted,
        Ok(response) if retryable_status(response.status().as_u16()) => AttemptResult::Retry,
        Ok(_) => AttemptResult::Rejected,
        Err(_) => AttemptResult::Retry,
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/client_login_state")]
struct ClientLoginDebug;

fn debug(mut cmd: ConsoleCommand<ClientLoginDebug>, state: Res<ClientLoginDelivery>) {
    let Some(Ok(_)) = cmd.take() else { return };
    cmd.reply_ok(
        serde_json::json!({
            "active": state.active.is_some(), "pending": state.queue.events.len(),
            "inFlight": state.task.is_some(), "attempts": state.attempts,
            "result": if state.result.is_empty() { "idle" } else { state.result },
            "discarded": state.discarded, "durable": state.queue.ready(),
        "storageInFlight": state.storage.is_some(), "retryInMs": state.next_attempt.saturating_sub(now_ms())
        })
        .to_string(),
    );
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn every_identity_completion_queues_without_a_social_consumer() {
        let root = wallet::local_guest::new_root_key();
        let (ephemeral, delegates) =
            wallet::local_guest::sign_identity(&root, now_ms() as i64).unwrap();
        let mut wallet = Wallet::default();
        wallet.finalize(root.address(), ephemeral, delegates, false);
        let mut state = ClientLoginDelivery::default();
        state.completed(&wallet);
        let first = state.queue.events[0].clone();
        state.completed(&wallet);
        assert_eq!(state.queue.events.len(), 2);
        assert_eq!(state.queue.events[0], first);
        assert_ne!(state.queue.events[1].login_id, first.login_id);
        assert!(!state.queue.ready());
        state.pause();
        assert!(state.active.is_none());
        assert!(state.task.is_none());
        assert_eq!(state.queue.events.len(), 2);
    }

    #[test]
    fn retry_resigns_same_event_and_keeps_snapshot_account() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!(
            "http://{}/social-api/v1/client-login",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for status in [503, 204] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                requests.push(String::from_utf8(bytes).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            }
            requests
        });
        let mut live = Wallet::default();
        live.finalize_as_guest();
        let address = format!("{:#x}", live.address().unwrap());
        let wallet = live.snapshot();
        live.finalize_as_guest();
        live.disconnect();
        let event = PendingClientLogin {
            wallet: address.clone(),
            endpoint,
            login_id: uuid::Uuid::new_v4().to_string(),
            occurred_at: now_ms(),
        };
        async_std::task::block_on(platform::compat(async {
            assert_eq!(send(&wallet, &event).await, AttemptResult::Retry);
            async_std::task::sleep(Duration::from_millis(2)).await;
            assert_eq!(send(&wallet, &event).await, AttemptResult::Accepted);
        }));
        let requests = server.join().unwrap();
        let headers = requests
            .iter()
            .map(|request| {
                assert!(request.starts_with("POST /social-api/v1/client-login HTTP/1.1\r\n"));
                request
                    .lines()
                    .filter_map(|line| line.split_once(": "))
                    .map(|(key, value)| (key.to_lowercase(), value.to_string()))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(headers[0]["x-identity-metadata"], event.metadata());
        assert_eq!(
            headers[0]["x-identity-metadata"],
            headers[1]["x-identity-metadata"]
        );
        assert_ne!(
            headers[0]["x-identity-timestamp"],
            headers[1]["x-identity-timestamp"]
        );
        for header in &headers {
            let signer: serde_json::Value =
                serde_json::from_str(&header["x-identity-auth-chain-0"]).unwrap();
            assert_eq!(signer["payload"], address);
        }
    }
}
