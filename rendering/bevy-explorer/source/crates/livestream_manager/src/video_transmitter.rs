use bevy::prelude::*;
use common::util::watch_send_if_modified;
use dcl_component::proto_components::sdk::components::VideoState;

/// Component for the kind of transmitter that the entity is
#[derive(Debug, Clone, Copy, Component)]
#[component(immutable)]
pub enum TransmitterKind {
    Presentation,
    Screenshare,
    VideoCast,
    Stream,
}

/// Extra marker component for [`TransmitterKind::VideoCast`] that
/// are active
#[derive(Component)]
pub struct ActiveVideoCast;

/// Component with the handle to the image that the transmitter
/// should write to
#[derive(Component)]
pub struct ActiveTransmitter {
    pub image: Handle<Image>,
    pub state: tokio::sync::watch::Sender<VideoState>,
}

/// Seconds without a new frame before a transmission that showed one reports buffering: above
/// the 1 s at which WebRTC repeats the frame of a still screen share.
const BUFFER_THRESHOLD: f64 = 3.;

impl ActiveTransmitter {
    pub fn send_if_modified(&self, new_state: VideoState) -> bool {
        watch_send_if_modified(&self.state, new_state)
    }

    /// Reports the state the frames show: playing at a `frame`, buffering once none came for
    /// [`BUFFER_THRESHOLD`] after `last_frame`. Before the first frame the state stays as it is.
    pub fn report_frames(&self, frame: bool, last_frame: &mut Option<f64>, now: f64) {
        if frame {
            *last_frame = Some(now);
            self.send_if_modified(VideoState::VsPlaying);
        } else if last_frame.is_some_and(|last| now > last + BUFFER_THRESHOLD) {
            self.send_if_modified(VideoState::VsBuffering);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_follows_the_frames() {
        let transmitter = ActiveTransmitter {
            image: Handle::default(),
            state: tokio::sync::watch::channel(VideoState::VsLoading).0,
        };
        let mut last_frame = None;
        // no frame yet, the first frame, a short gap, a long gap, the next frame
        for (frame, now, state) in [
            (false, 60., VideoState::VsLoading),
            (true, 60.1, VideoState::VsPlaying),
            (false, 63.1, VideoState::VsPlaying),
            (false, 63.2, VideoState::VsBuffering),
            (true, 70., VideoState::VsPlaying),
        ] {
            transmitter.report_frames(frame, &mut last_frame, now);
            assert_eq!(*transmitter.state.borrow(), state, "at {now}");
        }
    }
}
