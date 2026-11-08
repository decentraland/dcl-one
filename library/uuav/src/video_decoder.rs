//! Video decoding, hardware first and software otherwise.
//!
//! The hardware path is the platform's `libavcodec` hwaccel: VideoToolbox
//! on macOS, D3D11VA on Windows, VAAPI on Linux. The software path is the
//! plain `libavcodec` decoder with frame threading. Whatever decoded a
//! frame, it leaves here as NV12: either a GPU surface the platform output
//! copies without touching the CPU (macOS, Windows), or a CPU frame the
//! output uploads (every platform; the only kind on Linux, where VAAPI
//! surfaces are downloaded because nothing shares them with the host).
//!
//! Selection happens at open: hardware when the codec has a matching
//! hwaccel and the device context exists, software otherwise or when
//! `UUAV_FORCE_SW_DECODE` is set. A hardware decoder that dies before its
//! first frame (a profile the hardware rejects only surfaces from
//! `get_format` on the first packets) is replaced by a software one by the
//! owning [`super::playback`] unit, which replays the packets it was fed.

use anyhow::{Context, Result, anyhow, ensure};
use ffmpeg_sys_next as ff;
use std::ffi::CStr;
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::ErrorCallback;
use crate::ffutil::{
    AVERROR_EAGAIN, Decoded, OwnedDecoder, OwnedFrame, OwnedSws, Stream, av_err, check, q2d,
};
use crate::hw_device::HwDeviceContext;

/// The platform hwaccel: device type, the pixel format its frames carry,
/// and the name reported through `MediaInfo.decoder`.
mod hw {
    use ffmpeg_sys_next as ff;

    #[cfg(target_os = "macos")]
    pub(super) const DEVICE_TYPE: ff::AVHWDeviceType =
        ff::AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX;
    #[cfg(target_os = "macos")]
    pub(super) const PIX_FMT: ff::AVPixelFormat = ff::AVPixelFormat::AV_PIX_FMT_VIDEOTOOLBOX;
    #[cfg(target_os = "macos")]
    pub(super) const NAME: &str = "videotoolbox";

    #[cfg(target_os = "windows")]
    pub(super) const DEVICE_TYPE: ff::AVHWDeviceType = ff::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA;
    #[cfg(target_os = "windows")]
    pub(super) const PIX_FMT: ff::AVPixelFormat = ff::AVPixelFormat::AV_PIX_FMT_D3D11;
    #[cfg(target_os = "windows")]
    pub(super) const NAME: &str = "d3d11va";

    #[cfg(target_os = "linux")]
    pub(super) const DEVICE_TYPE: ff::AVHWDeviceType = ff::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI;
    #[cfg(target_os = "linux")]
    pub(super) const PIX_FMT: ff::AVPixelFormat = ff::AVPixelFormat::AV_PIX_FMT_VAAPI;
    #[cfg(target_os = "linux")]
    pub(super) const NAME: &str = "vaapi";
}

/// The platform hwaccel's device type, for the device context constructor.
#[cfg(target_os = "linux")]
pub(crate) const HW_DEVICE_TYPE: ff::AVHWDeviceType = hw::DEVICE_TYPE;

/// Name of the platform hardware decoder as reported in `MediaInfo`.
pub(crate) const HW_DECODER_NAME: &str = hw::NAME;

/// Name of the software decoder as reported in `MediaInfo`.
pub(crate) const SW_DECODER_NAME: &str = "software";

/// `UUAV_FORCE_SW_DECODE` set (to anything but empty or `0`) skips the
/// hardware path entirely: for tests and CI runners whose GPU decodes
/// nothing, and to compare the two paths on one machine.
pub(crate) fn software_forced() -> bool {
    static FORCED: OnceLock<bool> = OnceLock::new();
    *FORCED.get_or_init(|| {
        std::env::var_os("UUAV_FORCE_SW_DECODE")
            .is_some_and(|value| !value.is_empty() && value != "0")
    })
}

/// Which decoder a playback runs on, readable from any thread for
/// `MediaInfo.decoder`; moves from hardware to software on a fallback.
pub(crate) struct DecoderModeCell(AtomicU8);

const MODE_NONE: u8 = 0;
const MODE_HARDWARE: u8 = 1;
const MODE_SOFTWARE: u8 = 2;

impl DecoderModeCell {
    pub(crate) const fn new() -> Self {
        Self(AtomicU8::new(MODE_NONE))
    }

    /// A hardware decoder died before its first frame and a software one
    /// took over (see the playback unit's replay).
    pub(crate) fn mark_software(&self) {
        self.set(false);
    }

    fn set(&self, hardware: bool) {
        self.0.store(
            if hardware {
                MODE_HARDWARE
            } else {
                MODE_SOFTWARE
            },
            Ordering::Release,
        );
    }

    /// Empty while no video decoder exists (audio-only media, still opening).
    pub(crate) fn name(&self) -> &'static str {
        match self.0.load(Ordering::Acquire) {
            MODE_HARDWARE => HW_DECODER_NAME,
            MODE_SOFTWARE => SW_DECODER_NAME,
            _ => "",
        }
    }
}

/// The pixel storage of a decoded frame.
enum FrameData {
    /// A platform hardware surface (NV12), presented without a CPU copy.
    #[cfg(not(target_os = "linux"))]
    Hw(OwnedFrame),
    /// An NV12 frame in system memory, uploaded by the platform output.
    Cpu(OwnedFrame),
}

/// Plane pointers of a CPU NV12 frame; valid while the frame lives.
#[derive(Clone, Copy)]
pub(crate) struct CpuPlanes {
    pub(crate) y: *const u8,
    pub(crate) y_stride: usize,
    pub(crate) uv: *const u8,
    pub(crate) uv_stride: usize,
}

/// A decoded frame. The surface or buffer behind it stays alive (and thus
/// valid for GPU reads or CPU uploads) for as long as the frame is held:
/// AVFrame refcounting owns it, so handing frames from the playback thread
/// to the render thread needs no extra bookkeeping.
pub(crate) struct VideoFrame {
    data: FrameData,
    pts: Option<f64>,
    width: i32,
    height: i32,
}

impl VideoFrame {
    /// `CVPixelBufferRef` of a VideoToolbox frame (IOSurface-backed NV12);
    /// `None` for a CPU frame.
    #[cfg(target_os = "macos")]
    pub(crate) fn pixel_buffer(&self) -> Option<*mut c_void> {
        match &self.data {
            FrameData::Hw(frame) => Some(frame.data(3).cast::<c_void>()),
            FrameData::Cpu(_) => None,
        }
    }

    /// `ID3D11Texture2D*` of the decoder surface pool plus the array slice
    /// the frame lives in; `None` for a CPU frame.
    #[cfg(target_os = "windows")]
    pub(crate) fn d3d11_surface(&self) -> Option<(*mut c_void, u32)> {
        match &self.data {
            FrameData::Hw(frame) => {
                let index = frame.data(1) as usize;
                Some((
                    frame.data(0).cast::<c_void>(),
                    u32::try_from(index).unwrap_or(0),
                ))
            }
            FrameData::Cpu(_) => None,
        }
    }

    /// The NV12 planes of a CPU frame; `None` for a hardware surface.
    #[cfg_attr(target_os = "linux", allow(clippy::unnecessary_wraps))] // no surfaces there
    pub(crate) fn cpu_planes(&self) -> Option<CpuPlanes> {
        match &self.data {
            #[cfg(not(target_os = "linux"))]
            FrameData::Hw(_) => None,
            FrameData::Cpu(frame) => Some(CpuPlanes {
                y: frame.data(0).cast_const(),
                y_stride: usize::try_from(frame.linesize(0)).unwrap_or(0),
                uv: frame.data(1).cast_const(),
                uv_stride: usize::try_from(frame.linesize(1)).unwrap_or(0),
            }),
        }
    }

    /// Presentation time in seconds of the media timeline, `None` when the
    /// container provides no usable timestamp.
    pub(crate) const fn pts(&self) -> Option<f64> {
        self.pts
    }

    /// Shifts the presentation time so the media timeline starts at zero.
    pub(crate) fn shift_pts(&mut self, offset: f64) {
        self.pts = self.pts.map(|pts| pts - offset);
    }

    pub(crate) const fn width(&self) -> i32 {
        self.width
    }

    pub(crate) const fn height(&self) -> i32 {
        self.height
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecoderKind {
    Hardware,
    Software,
}

/// One `libavcodec` decoder context in either mode.
pub(crate) struct VideoDecoder {
    ctx: OwnedDecoder,
    time_base: ff::AVRational,
    kind: DecoderKind,
    /// Converts CPU frames that are not NV12 already (most software
    /// decoders emit planar YUV); created on the first such frame.
    sws: Option<OwnedSws>,
    /// Frames returned so far; the owner replaces a hardware decoder that
    /// fails while this is still zero.
    produced: u64,
}

impl VideoDecoder {
    /// Extra surfaces a hardware decoder allocates on top of its own needs;
    /// must cover every frame the player can hold queued at once.
    pub(crate) const EXTRA_HW_FRAMES: c_int = 8;

    /// Opens the hardware decoder when `hw` exists and the codec has a
    /// matching hwaccel, the software decoder otherwise. A hardware open
    /// that fails is reported through `warnings` and falls through to
    /// software rather than failing the playback.
    pub(crate) fn open(
        stream: Stream,
        hw: Option<&HwDeviceContext>,
        mode: &DecoderModeCell,
        warnings: &ErrorCallback,
    ) -> Result<Self> {
        if let Some(hw) = hw {
            match Self::open_hardware(stream, hw) {
                Ok(decoder) => {
                    mode.set(true);
                    return Ok(decoder);
                }
                Err(e) => warnings.report(format!(
                    "{} decoding unavailable for this stream ({e:#}); decoding in software",
                    hw::NAME
                )),
            }
        }
        let decoder = Self::open_software(stream)?;
        mode.set(false);
        Ok(decoder)
    }

    fn open_hardware(stream: Stream, hw: &HwDeviceContext) -> Result<Self> {
        let codec = find_hw_decoder(stream.codec_id())?;
        let mut ctx = OwnedDecoder::new(codec)?;

        unsafe {
            let raw = ctx.as_mut_ptr();
            check(
                "avcodec_parameters_to_context(video)",
                ff::avcodec_parameters_to_context(raw, stream.codecpar()),
            )?;
            (*raw).pkt_timebase = stream.time_base();
            (*raw).hw_device_ctx = ff::av_buffer_ref(hw.as_buffer_ptr());
            if (*raw).hw_device_ctx.is_null() {
                return Err(anyhow!(
                    "failed to reference the {} device context",
                    hw::NAME
                ));
            }
            (*raw).get_format = Some(require_hw_format);
            (*raw).extra_hw_frames = Self::EXTRA_HW_FRAMES;

            check(
                "avcodec_open2(video)",
                ff::avcodec_open2(raw, codec, ptr::null_mut()),
            )?;
        }

        Ok(Self {
            ctx,
            time_base: stream.time_base(),
            kind: DecoderKind::Hardware,
            sws: None,
            produced: 0,
        })
    }

    /// The plain `libavcodec` decoder with frame threading; its output
    /// format is whatever the codec emits and is converted on receive.
    pub(crate) fn open_software(stream: Stream) -> Result<Self> {
        let codec = stream.find_decoder().context("video stream")?;
        let mut ctx = OwnedDecoder::new(codec)?;

        unsafe {
            let raw = ctx.as_mut_ptr();
            check(
                "avcodec_parameters_to_context(video)",
                ff::avcodec_parameters_to_context(raw, stream.codecpar()),
            )?;
            (*raw).pkt_timebase = stream.time_base();
            // 0 lets libavcodec size the pool from the core count
            (*raw).thread_count = 0;
            (*raw).thread_type = (ff::FF_THREAD_FRAME | ff::FF_THREAD_SLICE) as c_int;

            check(
                "avcodec_open2(video, software)",
                ff::avcodec_open2(raw, codec, ptr::null_mut()),
            )?;
        }

        Ok(Self {
            ctx,
            time_base: stream.time_base(),
            kind: DecoderKind::Software,
            sws: None,
            produced: 0,
        })
    }

    pub(crate) const fn is_hardware(&self) -> bool {
        matches!(self.kind, DecoderKind::Hardware)
    }

    /// Frames returned by [`Self::receive`] so far.
    pub(crate) const fn produced_frames(&self) -> u64 {
        self.produced
    }

    /// Sends a packet; pass `null` to signal end of stream.
    pub(crate) fn send(&mut self, packet: *const ff::AVPacket) -> Result<()> {
        let ret = unsafe { ff::avcodec_send_packet(self.ctx.as_mut_ptr(), packet) };
        // EOF from send just means the drain was already signalled
        if ret < 0 && ret != ff::AVERROR_EOF {
            return Err(av_err("avcodec_send_packet(video)", ret));
        }
        Ok(())
    }

    pub(crate) fn receive(&mut self) -> Result<Decoded<VideoFrame>> {
        let mut frame = OwnedFrame::new()?;
        let ret = unsafe { ff::avcodec_receive_frame(self.ctx.as_mut_ptr(), frame.as_mut_ptr()) };
        if ret == AVERROR_EAGAIN {
            return Ok(Decoded::Again);
        }
        if ret == ff::AVERROR_EOF {
            return Ok(Decoded::Eof);
        }
        check("avcodec_receive_frame(video)", ret)?;

        let ts = frame.best_effort_timestamp();
        let pts = if ts == ff::AV_NOPTS_VALUE {
            None
        } else {
            Some(ts as f64 * q2d(self.time_base))
        };
        let width = frame.width();
        let height = frame.height();

        let data = match self.kind {
            DecoderKind::Hardware => self.hardware_frame(frame)?,
            DecoderKind::Software => FrameData::Cpu(self.ensure_nv12(frame)?),
        };
        self.produced = self.produced.saturating_add(1);

        Ok(Decoded::Frame(VideoFrame {
            data,
            pts,
            width,
            height,
        }))
    }

    /// Validates a frame the hardware decoder returned and shapes it for
    /// the platform output.
    #[cfg_attr(target_os = "linux", allow(clippy::needless_pass_by_value))] // downloaded, not kept
    #[cfg_attr(
        not(target_os = "linux"),
        allow(clippy::unused_self, clippy::needless_pass_by_ref_mut) // only Linux converts here
    )]
    fn hardware_frame(&mut self, frame: OwnedFrame) -> Result<FrameData> {
        ensure!(
            frame.format() == hw::PIX_FMT as c_int,
            "decoder produced a non-{} frame (format {}) in hardware mode",
            hw::NAME,
            frame.format()
        );

        #[cfg(not(target_os = "linux"))]
        {
            // sw_format NV12 covers both the video- and full-range surface
            // formats; anything else (P010/10-bit, ...) is rejected, which
            // before the first frame falls back to software decoding
            let sw_format = frame.hw_frames_ctx().context(hw::NAME)?.sw_format;
            ensure!(
                sw_format == ff::AVPixelFormat::AV_PIX_FMT_NV12,
                "unsupported {} surface format {:?}; only NV12 is supported",
                hw::NAME,
                sw_format
            );
            Ok(FrameData::Hw(frame))
        }

        #[cfg(target_os = "linux")]
        {
            // VAAPI surfaces are not shared with the host process: download
            // into system memory (the hardware context's sw_format, NV12
            // for 8-bit streams) and present through the CPU path
            let cpu = OwnedFrame::transfer_from_hw(&frame)?;
            Ok(FrameData::Cpu(self.ensure_nv12(cpu)?))
        }
    }

    /// NV12 frames pass through; anything else is converted.
    fn ensure_nv12(&mut self, frame: OwnedFrame) -> Result<OwnedFrame> {
        if frame.format() == ff::AVPixelFormat::AV_PIX_FMT_NV12 as c_int {
            return Ok(frame);
        }
        self.sws
            .get_or_insert_with(OwnedSws::new)
            .scale_to_nv12(&frame)
    }

    pub(crate) fn flush(&mut self) {
        unsafe { ff::avcodec_flush_buffers(self.ctx.as_mut_ptr()) };
    }
}

/// The decoder for `id` whose hwaccel matches the platform device. The
/// registry's first decoder for a codec can be an external software
/// library (libdav1d for AV1) while the hwaccel hooks sit on the native
/// one, so the choice goes by configuration, not by name. Software-only
/// decoders never invoke `get_format`, so without this probe a missing
/// hwaccel would only surface after a software frame was decoded.
fn find_hw_decoder(id: ff::AVCodecID) -> Result<*const ff::AVCodec> {
    let mut opaque: *mut c_void = ptr::null_mut();
    loop {
        let codec = unsafe { ff::av_codec_iterate(&mut opaque) };
        if codec.is_null() {
            break;
        }
        if unsafe { (*codec).id } == id
            && unsafe { ff::av_codec_is_decoder(codec) } != 0
            && has_hw_config(codec)
        {
            return Ok(codec);
        }
    }

    let name = unsafe { CStr::from_ptr(ff::avcodec_get_name(id)) };
    Err(anyhow!(
        "codec {} has no {} hardware decoder",
        name.to_string_lossy(),
        hw::NAME
    ))
}

fn has_hw_config(codec: *const ff::AVCodec) -> bool {
    for index in 0.. {
        let config = unsafe { ff::avcodec_get_hw_config(codec, index) };
        if config.is_null() {
            break;
        }
        let config = unsafe { &*config };
        let by_device_ctx =
            config.methods & ff::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as c_int != 0;
        if by_device_ctx && config.device_type == hw::DEVICE_TYPE && config.pix_fmt == hw::PIX_FMT {
            return true;
        }
    }
    false
}

/// Rejects everything but the hardware pixel format: returning NONE makes
/// `avcodec` fail decoding instead of silently continuing with software
/// frames inside a context configured for hardware surfaces. The owner
/// turns that failure into a software decoder.
unsafe extern "C" fn require_hw_format(
    _ctx: *mut ff::AVCodecContext,
    list: *const ff::AVPixelFormat,
) -> ff::AVPixelFormat {
    unsafe {
        let mut p = list;
        while !p.is_null() && *p != ff::AVPixelFormat::AV_PIX_FMT_NONE {
            if *p == hw::PIX_FMT {
                return hw::PIX_FMT;
            }
            p = p.add(1);
        }
    }
    ff::AVPixelFormat::AV_PIX_FMT_NONE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(codec: *const ff::AVCodec) -> String {
        unsafe { CStr::from_ptr((*codec).name) }
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn hardware_lookup_goes_by_hwaccel_not_by_registry_order() -> Result<()> {
        // AV1: libdav1d decodes in software and comes first in the registry;
        // the hwaccel hooks sit on the native decoder
        let software = unsafe { ff::avcodec_find_decoder(ff::AVCodecID::AV_CODEC_ID_AV1) };
        ensure!(!software.is_null(), "no AV1 decoder");
        assert_eq!(name(software), "libdav1d");
        if let Ok(hardware) = find_hw_decoder(ff::AVCodecID::AV_CODEC_ID_AV1) {
            assert_eq!(name(hardware), "av1");
        }
        assert_eq!(
            name(find_hw_decoder(ff::AVCodecID::AV_CODEC_ID_H264)?),
            "h264"
        );
        assert!(find_hw_decoder(ff::AVCodecID::AV_CODEC_ID_THEORA).is_err());
        Ok(())
    }
}
