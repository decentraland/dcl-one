use crate::livekit::web::{
    request, AudioCaptureOptions, LivekitCommand, ObjectId, Reply, RoomError, RoomResult, TrackSid,
};

#[derive(Debug, Clone)]
pub struct LocalAudioTrack {
    pub(super) id: ObjectId,
    pub(super) sid: TrackSid,
}

impl LocalAudioTrack {
    /// Commands are ordered with publish requests on the page's LiveKit channel.
    pub fn set_enabled(&self, enabled: bool) {
        super::send_command(LivekitCommand::SetLocalAudioEnabled {
            track: self.id,
            enabled,
        });
    }

    pub async fn stop(&self, release: bool) -> RoomResult<()> {
        match request(|request| LivekitCommand::StopLocalAudio {
            request,
            track: self.id,
            release,
        })
        .await?
        {
            Reply::LocalAudioStopped(result) => result.map_err(RoomError::from),
            _ => Err(RoomError::from("unexpected stop audio reply".to_owned())),
        }
    }

    /// The inner result is the page's explicit creation reply. An outer error leaves the
    /// caller without proof that the page did not create a capture source.
    pub(crate) async fn new(options: AudioCaptureOptions) -> RoomResult<Result<Self, String>> {
        match request(|request| LivekitCommand::CreateLocalAudioTrack { request, options }).await? {
            Reply::LocalAudioTrack(result) => Ok(result),
            other => Err(RoomError::from(format!(
                "unexpected reply to local audio track: {other:?}"
            ))),
        }
    }

    pub fn sid(&self) -> TrackSid {
        self.sid.clone()
    }
}
