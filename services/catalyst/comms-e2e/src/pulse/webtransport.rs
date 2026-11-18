//! The WebTransport side of a Pulse bot: one thread, one session. Same contract as the ENet
//! driver: it moves frames and reports what the transport did, with no policy of its own.
//!
//! Reliable messages travel on the framed bidirectional stream, unreliable ones in datagrams
//! with the channel header. The server answers on the stream (framed) and in bare datagrams: a
//! datagram from the server is one message with no header. The reason the server ended the
//! session is the session's close code; a session that ends any other way (idle timeout, lost
//! path, a close without an application code) reports 0.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use catalyst_comms::pulse::transport::webtransport::client::{Client, ClientConfig, ClientEvent};
use catalyst_comms::pulse::transport::webtransport::framing::{
    DatagramSequencer, StreamFrameReader, datagram_frame, stream_frame,
};

use super::{Channel, Command, Shared};

const SERVICE_TICK: Duration = Duration::from_millis(1);
/// The largest message the server sends on the stream; the reader grows past this only on
/// a corrupt length, which it refuses.
const STREAM_MESSAGE_LIMIT: usize = 256 * 1024;
/// Pulse's graceful close code, what the ENet driver also sends when a bot is stopped.
const GRACEFUL: u32 = 1;

/// What to dial: the URL (host and port already replaced if a link is in front) and the
/// SHA-256 of the server certificate, if it is self-signed.
pub(crate) struct Dial {
    pub url: String,
    pub certificate_hash: Option<Vec<u8>>,
    /// The local address to dial from; the server counts its per-address limits by it.
    pub local: Option<std::net::IpAddr>,
    pub deadline: Duration,
}

pub(crate) fn spawn(
    dial: Dial,
    shared: Arc<Shared>,
    commands: Receiver<Command>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name(format!("pulse-bot {}", shared.name))
        .spawn(move || run(dial, shared, commands))
}

struct Wire {
    client: Client,
    sequencer: DatagramSequencer,
}

impl Wire {
    fn send(&mut self, shared: &Shared, channel: Channel, bytes: &[u8]) {
        let sent = match channel {
            Channel::Reliable => self.client.send_stream(&stream_frame(bytes)),
            other => {
                let sequence = self.sequencer.next(other.id());
                self.client
                    .send_datagram(&datagram_frame(other.id(), sequence, bytes))
            }
        };
        if !sent {
            shared.record("send failed", format!("{channel:?}, {} bytes", bytes.len()));
        }
    }
}

fn run(dial: Dial, shared: Arc<Shared>, commands: Receiver<Command>) {
    let client = match Client::connect_within_from(
        ClientConfig {
            url: dial.url,
            server_cert_hash: dial.certificate_hash,
        },
        Some(dial.deadline),
        dial.local,
    ) {
        Ok(client) => client,
        Err(error) => return shared.transport_failed(format!("WebTransport: {error}")),
    };
    shared.connected();
    let mut wire = Wire {
        client,
        sequencer: DatagramSequencer::default(),
    };
    let mut reader = StreamFrameReader::new(STREAM_MESSAGE_LIMIT);

    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Send { channel, bytes }) => wire.send(&shared, channel, &bytes),
                Ok(Command::Disconnect { code }) => wire.client.disconnect(code),
                Ok(Command::Stop) | Err(TryRecvError::Disconnected) => {
                    wire.client.disconnect(GRACEFUL);
                    shared.closed_locally();
                    return;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        // The first wait is the tick; whatever else is queued is drained without waiting.
        let mut wait = SERVICE_TICK;
        while let Some(event) = wire.client.service(wait) {
            wait = Duration::ZERO;
            match event {
                ClientEvent::StreamData { data } => {
                    reader.append(&data);
                    loop {
                        match reader.try_read() {
                            Ok(Some(frame)) => reply(&mut wire, &shared, &frame),
                            Ok(None) => break,
                            Err(overrun) => {
                                wire.client.disconnect(GRACEFUL);
                                return shared.transport_failed(format!(
                                    "the server sent a {}-byte stream frame, over {}",
                                    overrun.length, overrun.cap
                                ));
                            }
                        }
                    }
                }
                ClientEvent::Datagram { data } => reply(&mut wire, &shared, &data),
                ClientEvent::Disconnected { reason } => {
                    shared.disconnected(reason);
                    return;
                }
            }
        }
    }
}

fn reply(wire: &mut Wire, shared: &Shared, message: &[u8]) {
    for (channel, bytes) in shared.received(message) {
        wire.send(shared, channel, &bytes);
    }
}
