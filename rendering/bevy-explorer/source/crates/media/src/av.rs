//! The av backend's interface, the same on every platform: a source is driven with
//! [`AVCommand`]s and reports [`VideoData`]. Native plays the source in the UUAV helper process
//! ([`crate::uuav`]); the web plays it in a page media element ([`crate::html`]). Both write
//! the frames into the target image from the render world.

#[derive(Debug)]
pub enum AVCommand {
    Play,
    Pause,
    Repeat(bool),
    Seek(f64),
    /// Applied where the backend mixes: the page element on the web, the kira sound on native.
    Volume(f32),
    Dispose,
}

pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub rate: f64,
    pub length: f64,
}

pub enum VideoData {
    Info(VideoInfo),
    /// A decoded frame, at this media time. The backend wrote it into the target image from the
    /// render world, so only the arrival is reported.
    Frame(f64),
}
