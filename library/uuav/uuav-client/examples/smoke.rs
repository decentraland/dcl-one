//! Headless end-to-end smoke of the out-of-process pipeline, no the host
//! involved: creates a real GPU probe texture (standing in for the host's),
//! walks the C ABI exactly like `UUAVRuntime`/`UUAVPlayer` do, and expects
//! the helper process to take a stream to PLAYING.
//!
//! Run: `bash build.sh`, stage `dist/<target>/` next to the example binary
//! (`tooling/uuav/stage-dist.py <target> .target/debug/examples`), then
//! `cargo run -p uuav-client --example smoke [media-url]`. Set
//! `UUAV_FORCE_SW_DECODE=1` to exercise the software decoder.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use uuav::{AudioOptionsRaw, ResultFFI, UUAVState};

/// The output format this smoke negotiates at init; every audio pull below
/// derives its sizes from here.
const AUDIO: AudioOptionsRaw = AudioOptionsRaw {
    sample_rate: 48_000,
    channels: 2,
};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

extern "C" fn on_error(line: *const c_char) {
    let message = to_str(line);
    if message.starts_with("uuav helper terminated (") {
        RECOVERY_STARTED.fetch_add(1, Ordering::AcqRel);
    }
    eprintln!("[error] {message}");
}

extern "C" fn on_warning(line: *const c_char) {
    let message = to_str(line);
    if message == "uuav helper restarted; players restored" {
        RECOVERY_FINISHED.store(RECOVERY_STARTED.load(Ordering::Acquire), Ordering::Release);
    }
    eprintln!("[warn ] {message}");
}

static RECOVERY_STARTED: AtomicU64 = AtomicU64::new(0);
static RECOVERY_FINISHED: AtomicU64 = AtomicU64::new(0);

fn recovered_since(previous: u64) -> bool {
    let started = RECOVERY_STARTED.load(Ordering::Acquire);
    started > previous && RECOVERY_FINISHED.load(Ordering::Acquire) >= started
}

extern "C" fn on_log(line: *const c_char) {
    eprintln!("[log  ] {}", to_str(line));
}

fn to_str(line: *const c_char) -> String {
    if line.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(line) }
        .to_string_lossy()
        .into_owned()
}

fn check(step: &str, result: ResultFFI) {
    if !result.error_message.is_null() {
        let message = to_str(result.error_message);
        unsafe { uuav::uuav_string_free(result.error_message.cast_mut()) };
        panic!("{step}: {message}");
    }
    println!("{step}: ok");
}

/// Stands in for the host's probe: any live `id<MTLTexture>` works, `uuav_init`
/// only derives the device (and blit queue) from it.
#[cfg(target_os = "macos")]
fn with_probe<T>(init: impl FnOnce(*const std::ffi::c_void) -> T) -> T {
    use objc2_metal::{
        MTLCreateSystemDefaultDevice, MTLDevice as _, MTLPixelFormat, MTLTextureDescriptor,
    };

    let device = MTLCreateSystemDefaultDevice().expect("no Metal device");
    let descriptor = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            MTLPixelFormat::RGBA8Unorm,
            1,
            1,
            false,
        )
    };
    let probe = device
        .newTextureWithDescriptor(&descriptor)
        .expect("no probe texture");
    init(objc2::rc::Retained::as_ptr(&probe).cast())
}

/// Stands in for the host's probe: any live `ID3D11Texture2D*` works,
/// `uuav_init` only derives the device (and its adapter LUID) from it.
/// Nothing is derived from a probe on Linux: frames arrive as shared
/// memory the host uploads itself.
#[cfg(target_os = "linux")]
fn with_probe<T>(init: impl FnOnce(*const std::ffi::c_void) -> T) -> T {
    init(std::ptr::null())
}

#[cfg(target_os = "windows")]
fn with_probe<T>(init: impl FnOnce(*const std::ffi::c_void) -> T) -> T {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
        D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice, ID3D11Device,
        ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::core::Interface as _;

    let mut device: Option<ID3D11Device> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    }
    .expect("D3D11CreateDevice failed");
    let device = device.expect("D3D11CreateDevice returned no device");

    let desc = D3D11_TEXTURE2D_DESC {
        Width: 1,
        Height: 1,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut probe: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut probe)) }
        .expect("probe CreateTexture2D failed");
    let probe = probe.expect("CreateTexture2D returned no texture");
    init(probe.as_raw().cast_const())
}

fn wait_for<T>(timeout: Duration, mut check: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + timeout;
    let mut delay = Duration::from_millis(1);
    loop {
        if let Some(value) = check() {
            return Some(value);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        std::thread::sleep(delay.min(remaining));
        delay = (delay * 2).min(FRAME_INTERVAL);
    }
}

fn media_time(player: uuav::PlayerId) -> Option<f64> {
    let mut time = 0.0;
    let result = unsafe { uuav::uuav_player_current_time(player, &mut time) };
    if result.error_message.is_null() {
        Some(time)
    } else {
        unsafe { uuav::uuav_string_free(result.error_message.cast_mut()) };
        None
    }
}

fn advancing(player: uuav::PlayerId, first: &mut Option<(f64, u64)>) -> Option<(f64, f64)> {
    if !matches!(uuav::uuav_player_state(player), UUAVState::UUAV_PLAYING) {
        return None;
    }
    let (time, frames) = uuav::uuav_player_progress(player)?;
    let (first_time, first_frames) = *first.get_or_insert((time, frames));
    (time >= first_time + 0.25 && frames.saturating_sub(first_frames) >= 8)
        .then_some((first_time, time))
}

fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://media.w3.org/2010/05/sintel/trailer.mp4".to_owned());

    let whitelist = c"https,http,tls,tcp,crypto,data,file";
    check(
        "uuav_init",
        with_probe(|probe| unsafe {
            uuav::uuav_init(
                probe,
                AUDIO,
                Some(on_error),
                Some(on_warning),
                Some(on_log),
                whitelist.as_ptr(),
                24, // AV_LOG_WARNING
            )
        }),
    );

    let created = uuav::uuav_player_new();
    assert!(
        created.error_message.is_null(),
        "uuav_player_new: {}",
        to_str(created.error_message)
    );
    let player = created.player_id;
    println!("uuav_player_new: id {player}");

    check("open_media", unsafe {
        let url = std::ffi::CString::new(url).unwrap();
        uuav::uuav_player_open_media_async(player, url.as_ptr())
    });
    check("play", uuav::uuav_player_play(player));

    let mut last_state = UUAVState::UUAV_UNKNOWN as i32;
    let mut first_time = None;
    let render = uuav::uuav_get_render_callback();
    wait_for(Duration::from_secs(20), || {
        render(player as i32);
        let state = uuav::uuav_player_state(player);
        if state as i32 != last_state {
            last_state = state as i32;
            if let Some(time) = media_time(player) {
                println!("state -> {state:?} (t = {time:.3}s)");
            } else {
                println!("state -> {state:?} (media time unavailable)");
            }
        }
        if let Some((first, time)) = advancing(player, &mut first_time) {
            println!("time advanced {first:.3}s -> {time:.3}s");
            return Some(());
        }
        None
    })
    .expect("player never reached PLAYING with advancing media time");

    // video: drive the render callback like the host's render thread would and
    // expect the shared-texture pipeline to surface presentation textures
    let (y, uv) = wait_for(Duration::from_secs(10), || {
        render(player as i32);
        let mut y: *const std::ffi::c_void = std::ptr::null();
        let mut uv: *const std::ffi::c_void = std::ptr::null();
        let y_result = unsafe { uuav::uuav_player_get_video_texture(player, 0, &mut y) };
        let uv_result = unsafe { uuav::uuav_player_get_video_texture(player, 1, &mut uv) };
        for r in [y_result, uv_result] {
            if !r.error_message.is_null() {
                unsafe { uuav::uuav_string_free(r.error_message.cast_mut()) };
            }
        }
        if !y.is_null() && !uv.is_null() {
            Some((y, uv))
        } else {
            None
        }
    })
    .expect("video planes never became available");
    #[cfg(target_os = "macos")]
    assert_ne!(y, uv, "Y and UV planes must be distinct textures");
    // one NV12 texture covers both planes on D3D11
    #[cfg(target_os = "windows")]
    assert_eq!(y, uv, "both planes must be the same NV12 texture");

    let mut size = uuav::VideoSize {
        width: 0,
        height: 0,
    };
    check("get_video_size", unsafe {
        uuav::uuav_player_get_video_size(player, &mut size)
    });
    println!(
        "video planes ready: {}x{} (y {y:?}, uv {uv:?})",
        size.width, size.height
    );

    // pointer stability: the presentation planes must not churn per frame
    let mut presented_at = None;
    wait_for(Duration::from_secs(10), || {
        render(player as i32);
        advancing(player, &mut presented_at)
    })
    .expect("media did not advance before checking plane stability");
    render(player as i32);
    let mut y_again: *const std::ffi::c_void = std::ptr::null();
    let again = unsafe { uuav::uuav_player_get_video_texture(player, 0, &mut y_again) };
    assert!(
        again.error_message.is_null() && y_again == y,
        "plane pointer churned"
    );
    println!("plane pointers stable across frames");

    // audio: pull like the host's audio thread would (10ms chunks at the
    // format negotiated at init) and expect real, non-silent samples
    let frames_per_pull = AUDIO.sample_rate / 100;
    let mut buffer = vec![0.0f32; (frames_per_pull * AUDIO.channels) as usize];
    let heard_signal = wait_for(Duration::from_secs(10), || {
        let frames =
            unsafe { uuav::uuav_player_read_audio(player, buffer.as_mut_ptr(), frames_per_pull) };
        if frames > 0 && buffer.iter().any(|s| s.abs() > 1e-4) {
            println!(
                "audio flowing: {frames} frames, peak {:.4}",
                buffer.iter().fold(0.0f32, |a, s| a.max(s.abs()))
            );
            Some(())
        } else {
            None
        }
    });
    assert!(
        heard_signal.is_some(),
        "no audible samples arrived within 10s"
    );

    let status = uuav::uuav_status();
    println!(
        "status: initialized={} players={}",
        status.initialized, status.players_count
    );

    // resurrection: kill the helper out from under the client and expect
    // playback to self-heal with zero API help — the recovery worker
    // respawns the helper and rebuilds the player from its desired state
    let previous_recovery = RECOVERY_STARTED.load(Ordering::Acquire);
    let t_before_kill = media_time(player).expect("media time unavailable before helper kill");
    kill_helper();
    println!("helper killed; waiting for automatic recovery");
    let mut first_time = None;
    wait_for(Duration::from_secs(30), || {
        if !recovered_since(previous_recovery) {
            return None;
        }
        render(player as i32);
        if let Some((first, time)) = advancing(player, &mut first_time) {
            println!(
                "recovered: PLAYING again, time advancing {first:.3}s -> {time:.3}s (was at {t_before_kill:.3}s when killed)"
            );
            assert!(
                first > t_before_kill - 2.0,
                "playback restarted far behind the resume point"
            );
            return Some(());
        }
        None
    })
    .expect("player never self-recovered after the helper was killed");

    // the presentation texture must have survived the outage (same pointer)
    let mut y_recovered: *const std::ffi::c_void = std::ptr::null();
    let r = unsafe { uuav::uuav_player_get_video_texture(player, 0, &mut y_recovered) };
    assert!(
        r.error_message.is_null() && y_recovered == y,
        "presentation pointer churned across the recovery"
    );
    println!("presentation pointer survived the recovery");

    uuav::uuav_player_free(player);
    uuav::uuav_deinit();
    println!("deinit: ok (helper should be gone)");
}

/// SIGKILL/TerminateProcess on the helper — a real crash, not a clean exit.
fn kill_helper() {
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "uuav-helper.exe"])
        .status();
    #[cfg(not(target_os = "windows"))]
    let status = std::process::Command::new("pkill")
        .args(["-9", "-x", "uuav-helper"])
        .status();
    assert!(
        status.expect("kill command failed to run").success(),
        "no uuav-helper process was there to kill"
    );
}

#[test]
fn an_old_completion_cannot_complete_the_next_recovery() {
    on_error(c"uuav helper terminated (previous); recovering".as_ptr());
    let previous = RECOVERY_STARTED.load(Ordering::Acquire);
    assert!(!recovered_since(previous));
    on_warning(c"uuav helper restarted; players restored".as_ptr());
    assert!(!recovered_since(previous));
    on_error(c"uuav helper terminated (current); recovering".as_ptr());
    assert!(!recovered_since(previous));
    on_warning(c"uuav helper restarted; players restored".as_ptr());
    assert!(recovered_since(previous));
}
