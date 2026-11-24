use std::sync::Arc;

use async_trait::async_trait;
use dcl_rpc::transports::web_sockets::{Error, Message, WebSocket};
use tokio::sync::{oneshot, Mutex};

pub struct PlatformRpcWebSocket {
    read: Mutex<platform::WebSocketSplitStream>,
    write: Mutex<platform::WebSocketSplitSink>,
    close_signal: Mutex<Option<oneshot::Sender<()>>>,
}

/// Resolves when the underlying websocket has been observed closed (by the
/// peer, by an error on the socket, or by an explicit local close). This is
/// the only reliable signal that dcl-rpc subscription streams have stopped
/// producing data — dcl-rpc's dispatcher does not propagate transport closure
/// to the per-subscription channels, so without this the streams hang.
pub struct WebSocketCloseSignal(oneshot::Receiver<()>);

impl WebSocketCloseSignal {
    pub async fn wait(self) {
        let _ = self.0.await;
    }
}

// SAFETY: the web build is multithreaded (atomics), and this socket holds JS handles, which
// belong to the thread that made them. It never leaves that thread: on wasm it is created and
// used only inside tasks on the `SocialRuntime` local runtime (`spawn_local`), on the thread that
// owns it. Don't hand it to anything that can run on another thread.
#[cfg(target_arch = "wasm32")]
unsafe impl Send for PlatformRpcWebSocket {}
// SAFETY: as for Send.
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for PlatformRpcWebSocket {}

impl PlatformRpcWebSocket {
    pub async fn connect(url: &str) -> Result<(Arc<Self>, WebSocketCloseSignal), Error> {
        let ws = platform::websocket(url)
            .await
            .map_err(|e| Error::Other(e.into()))?;
        let (write, read) = ws.split();
        let (tx, rx) = oneshot::channel();
        let inner = Arc::new(Self {
            read: Mutex::new(read),
            write: Mutex::new(write),
            close_signal: Mutex::new(Some(tx)),
        });
        Ok((inner, WebSocketCloseSignal(rx)))
    }

    async fn signal_closed(&self) {
        if let Some(tx) = self.close_signal.lock().await.take() {
            let _ = tx.send(());
        }
    }
}

fn to_tungstenite(message: Message) -> tungstenite::Message {
    match message {
        Message::Text(data) => tungstenite::Message::Text(data),
        Message::Binary(data) => tungstenite::Message::Binary(data),
        Message::Ping => tungstenite::Message::Ping(vec![]),
        Message::Pong => tungstenite::Message::Pong(vec![]),
        Message::Close => tungstenite::Message::Close(None),
    }
}

fn from_tungstenite(message: tungstenite::Message) -> Option<Message> {
    match message {
        tungstenite::Message::Text(data) => Some(Message::Text(data)),
        tungstenite::Message::Binary(data) => Some(Message::Binary(data)),
        tungstenite::Message::Ping(_) => Some(Message::Ping),
        tungstenite::Message::Pong(_) => Some(Message::Pong),
        tungstenite::Message::Close(_) => Some(Message::Close),
        tungstenite::Message::Frame(_) => None,
    }
}

#[async_trait]
impl WebSocket for PlatformRpcWebSocket {
    async fn send(&self, message: Message) -> Result<(), Error> {
        self.write
            .lock()
            .await
            .send(to_tungstenite(message))
            .await
            .map_err(|e| Error::Other(e.into()))
    }

    async fn receive(&self) -> Option<Result<Message, Error>> {
        loop {
            match self.read.lock().await.next().await {
                Some(Ok(msg)) => match from_tungstenite(msg) {
                    Some(Message::Close) => {
                        self.signal_closed().await;
                        return Some(Ok(Message::Close));
                    }
                    Some(m) => return Some(Ok(m)),
                    None => continue,
                },
                Some(Err(e)) => {
                    self.signal_closed().await;
                    return Some(Err(Error::Other(e.into())));
                }
                None => {
                    self.signal_closed().await;
                    return None;
                }
            }
        }
    }

    async fn close(&self) -> Result<(), Error> {
        self.signal_closed().await;
        self.write
            .lock()
            .await
            .send(tungstenite::Message::Close(None))
            .await
            .map_err(|e| Error::Other(e.into()))
    }
}
