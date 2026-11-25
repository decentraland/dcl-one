//! Thin safe wrappers over the uuav C ABI: every `ResultFFI` becomes a `Result<(), String>`
//! whose error string is freed exactly once, as the contract requires, and the out-param getters
//! become `Option`s.

use std::{
    ffi::{CStr, CString},
    os::raw::c_void,
};

pub use uuav::PlayerId;

pub fn consume(result: uuav::ResultFFI) -> Result<(), String> {
    if result.error_message.is_null() {
        return Ok(());
    }
    let message = unsafe { CStr::from_ptr(result.error_message) }
        .to_string_lossy()
        .into_owned();
    unsafe { uuav::uuav_string_free(result.error_message.cast_mut()) };
    Err(message)
}

pub fn player_new() -> Result<PlayerId, String> {
    let created = uuav::uuav_player_new();
    consume(uuav::ResultFFI {
        error_message: created.error_message,
    })?;
    Ok(created.player_id)
}

pub fn open_media(id: PlayerId, url: &str) -> Result<(), String> {
    let url = CString::new(url).map_err(|_| "url contains a NUL byte".to_owned())?;
    consume(unsafe { uuav::uuav_player_open_media_async(id, url.as_ptr()) })
}

pub fn state(id: PlayerId) -> uuav::UUAVState {
    uuav::uuav_player_state(id)
}

/// `None` for realtime streams and before the media is open.
pub fn duration(id: PlayerId) -> Option<f64> {
    let mut duration = 0.0;
    consume(unsafe { uuav::uuav_player_duration(id, &mut duration) })
        .ok()
        .map(|()| duration)
}

pub fn current_time(id: PlayerId) -> Option<f64> {
    let mut time = 0.0;
    consume(unsafe { uuav::uuav_player_current_time(id, &mut time) })
        .ok()
        .map(|()| time)
}

/// The stream header's size, available once the media is open. The presented planes
/// ([`presented`]) have their own size, which this need not match.
pub fn video_size(id: PlayerId) -> Option<(u32, u32)> {
    let mut size = uuav::VideoSize {
        width: 0,
        height: 0,
    };
    consume(unsafe { uuav::uuav_player_get_video_size(id, &mut size) })
        .ok()
        .map(|()| (size.width, size.height))
}

pub fn framerate(id: PlayerId) -> Option<f64> {
    // plain out-param storage, fully overwritten on success
    let mut info = unsafe { std::mem::zeroed::<uuav::MediaInfo>() };
    consume(unsafe { uuav::uuav_player_get_media_info(id, &mut info) })
        .ok()
        .map(|()| info.framerate)
}

/// The client-owned presentation planes and what they hold.
pub struct Presented {
    /// See `uuav_player_get_video_texture` for what the plane pointers are per platform. Stable
    /// across frames; replaced on resolution change, the previous ones kept for one more poll.
    pub y: *const c_void,
    pub uv: *const c_void,
    /// The size the planes were created for: `width` x `height` bytes behind `y`, half as many
    /// behind `uv`, both even.
    pub width: u32,
    pub height: u32,
    /// Frames presented into the planes so far; unchanged while nothing new was published.
    pub frames: u64,
}

/// `None` before the first presented frame.
pub fn presented(id: PlayerId) -> Option<Presented> {
    let mut video = uuav::PresentedVideo {
        plane0: std::ptr::null(),
        plane1: std::ptr::null(),
        width: 0,
        height: 0,
        frames: 0,
    };
    consume(unsafe { uuav::uuav_player_get_presented_video(id, &mut video) })
        .ok()
        .filter(|()| !video.plane0.is_null() && !video.plane1.is_null())
        .map(|()| Presented {
            y: video.plane0,
            uv: video.plane1,
            width: video.width,
            height: video.height,
            frames: video.frames,
        })
}
