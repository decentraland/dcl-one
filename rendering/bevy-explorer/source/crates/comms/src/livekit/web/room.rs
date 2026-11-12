use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::livekit::web::{
    register_room_events, request, send_command, unregister_room_events, LivekitCommand,
    LocalParticipant, Reply, RoomError, RoomEvent, RoomId, RoomOptions, RoomResult, NEXT_ROOM_ID,
};

/// The engine's handle on a livekit room the page owns.
#[derive(Debug)]
pub struct Room {
    id: RoomId,
    name: String,
    /// The room's metadata as the page last reported it: at connect, then on every change.
    metadata: Arc<Mutex<String>>,
    local_participant: LocalParticipant,
}

struct ConnectingRoom(Option<RoomId>);

impl Drop for ConnectingRoom {
    fn drop(&mut self) {
        if let Some(id) = self.0 {
            unregister_room_events(id);
            send_command(LivekitCommand::DropRoom(id));
        }
    }
}

impl Room {
    pub async fn connect(
        url: &str,
        token: &str,
        room_options: RoomOptions,
    ) -> RoomResult<(Room, mpsc::UnboundedReceiver<RoomEvent>)> {
        let url = url.to_owned();
        let token = token.to_owned();

        let id = NEXT_ROOM_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // registered before the connect so no event is missed
        let (receiver, metadata) = register_room_events(id);
        let mut connecting = ConnectingRoom(Some(id));

        let reply = request(|request| LivekitCommand::Connect {
            request,
            room: id,
            url,
            token,
            options: room_options,
        })
        .await;

        match reply {
            Ok(Reply::Connect(Ok(connected))) => {
                connecting.0 = None;
                Ok((
                    Room {
                        id,
                        name: connected.name,
                        metadata,
                        local_participant: connected.local_participant,
                    },
                    receiver,
                ))
            }
            Ok(Reply::Connect(Err(err))) => {
                unregister_room_events(id);
                Err(err)
            }
            Ok(other) => {
                unregister_room_events(id);
                Err(RoomError::from(format!(
                    "unexpected reply to connect: {other:?}"
                )))
            }
            Err(err) => {
                unregister_room_events(id);
                Err(err)
            }
        }
    }

    pub async fn close(&self) -> RoomResult<()> {
        let room = self.id;
        match request(|request| LivekitCommand::Close { request, room }).await? {
            Reply::Close(result) => result.map_err(RoomError::from),
            other => Err(RoomError::from(format!(
                "unexpected reply to close: {other:?}"
            ))),
        }
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn metadata(&self) -> String {
        self.metadata.lock().unwrap().clone()
    }

    pub fn local_participant(&self) -> LocalParticipant {
        self.local_participant.clone()
    }

    pub(crate) fn dispose(&self) {
        unregister_room_events(self.id);
        send_command(LivekitCommand::DropRoom(self.id));
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        self.dispose();
    }
}
