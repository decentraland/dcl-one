//! Thin RAII wrappers and error helpers over `ffmpeg-sys-next`.

use anyhow::{Result, anyhow, ensure};
use ffmpeg_sys_next as ff;
use std::ffi::{CStr, CString};
use std::num::NonZeroI32;
use std::os::raw::{c_char, c_int};

/// "Decoder needs more input" return code. FFmpeg has no named define for
/// it: the C API spells it `AVERROR(EAGAIN)`, with `EAGAIN` coming from the
/// platform's errno, so the bindings can only offer the same composition.
pub(crate) const AVERROR_EAGAIN: c_int = ff::AVERROR(libc::EAGAIN);

/// Outcome of a receive call on a decoder.
pub(crate) enum Decoded<T> {
    Frame(T),
    /// Decoder needs more input.
    Again,
    /// Decoder is fully drained.
    Eof,
}

/// Formats an FFmpeg error code into an [`anyhow::Error`] with context.
pub(crate) fn av_err(context: &str, code: c_int) -> anyhow::Error {
    let mut buf = [0 as c_char; 64];
    let message = unsafe {
        if ff::av_strerror(code, buf.as_mut_ptr(), buf.len()) == 0 {
            CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
        } else {
            format!("ffmpeg error {code}")
        }
    };
    anyhow!("{context}: {message}")
}

/// Returns the code unchanged for non-negative FFmpeg return codes.
pub(crate) fn check(context: &str, code: c_int) -> Result<c_int> {
    if code < 0 {
        Err(av_err(context, code))
    } else {
        Ok(code)
    }
}

pub(crate) const fn q2d(r: ff::AVRational) -> f64 {
    if r.den == 0 {
        0.0
    } else {
        r.num as f64 / r.den as f64
    }
}

/// Copies a NUL-terminated C string into a fixed name buffer, truncating
/// to leave room for the terminator; a null `src` yields an empty string.
pub(crate) fn copy_c_name<const N: usize>(dst: &mut [c_char; N], src: *const c_char) {
    let bytes: &[u8] = if src.is_null() {
        &[]
    } else {
        unsafe { CStr::from_ptr(src) }.to_bytes()
    };

    let len = bytes.len().min(N.saturating_sub(1));
    for (d, &s) in dst.iter_mut().zip(bytes.iter().take(len)) {
        *d = c_char::from_ne_bytes([s]);
    }

    // terminate right after the copied bytes: dst may hold a longer
    // previous name, and position `len` always exists for N > 0
    if let Some(terminator) = dst.get_mut(len) {
        *terminator = 0;
    }
}

/// Copies a Rust string into a fixed name buffer, truncating to leave room
/// for the terminator.
pub(crate) fn copy_str_name<const N: usize>(dst: &mut [c_char; N], src: &str) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(N.saturating_sub(1));
    for (d, &s) in dst.iter_mut().zip(bytes.iter().take(len)) {
        *d = c_char::from_ne_bytes([s]);
    }
    if let Some(terminator) = dst.get_mut(len) {
        *terminator = 0;
    }
}

pub(crate) struct StreamingProtocol(CString);

impl StreamingProtocol {
    /// Safety
    /// `raw` must be null or point to a NUL-terminated C string.
    pub(crate) unsafe fn new(raw: *const c_char) -> Result<Self> {
        ensure!(!raw.is_null(), "protocol_whitelist is null");
        let value = unsafe { CStr::from_ptr(raw) };
        ensure!(!value.to_bytes().is_empty(), "protocol_whitelist is empty");
        Ok(Self(value.to_owned()))
    }
}

/// Owning wrapper around an `AVDictionary`.
pub(crate) struct AvDict(*mut ff::AVDictionary);

impl AvDict {
    #[allow(clippy::needless_pass_by_ref_mut)] // hands out a mutable alias
    pub(crate) const fn as_mut_ptr(&mut self) -> *mut *mut ff::AVDictionary {
        &raw mut self.0
    }

    /// Only fails on allocation failure, which leaves the entry unset and
    /// surfaces later as an open error. Copies both strings.
    pub(crate) fn set(&mut self, key: &std::ffi::CStr, value: &std::ffi::CStr) {
        unsafe {
            ff::av_dict_set(&mut self.0, key.as_ptr(), value.as_ptr(), 0);
        }
    }
}

impl From<&StreamingProtocol> for AvDict {
    fn from(protocol: &StreamingProtocol) -> Self {
        let mut dict: *mut ff::AVDictionary = std::ptr::null_mut();

        // Only fails on allocation failure, which leaves `dict` null and
        // surfaces later as an open error.
        // Copies the protocol value
        unsafe {
            ff::av_dict_set(
                &mut dict,
                c"protocol_whitelist".as_ptr(),
                protocol.0.as_ptr(),
                0,
            );
        }
        Self(dict)
    }
}

impl Drop for AvDict {
    fn drop(&mut self) {
        unsafe { ff::av_dict_free(&mut self.0) };
    }
}

/// Non-owning view of an `AVStream` belonging to a live format context.
#[derive(Clone, Copy)]
pub(crate) struct Stream(*mut ff::AVStream);

impl Stream {
    /// # Safety
    /// `ptr` must point to a stream of a live `AVFormatContext`, and the
    /// view must not outlive that context.
    pub(crate) const unsafe fn from_raw(ptr: *mut ff::AVStream) -> Self {
        Self(ptr)
    }

    pub(crate) fn codecpar(self) -> *const ff::AVCodecParameters {
        unsafe { (*self.0).codecpar }
    }

    pub(crate) fn codec_id(self) -> ff::AVCodecID {
        unsafe { (*self.codecpar()).codec_id }
    }

    pub(crate) fn time_base(self) -> ff::AVRational {
        unsafe { (*self.0).time_base }
    }

    pub(crate) fn avg_frame_rate(self) -> ff::AVRational {
        unsafe { (*self.0).avg_frame_rate }
    }

    /// Tells the demuxer which packets of this stream to drop before they
    /// reach the caller. `AVDISCARD_ALL` also lets multi-variant demuxers
    /// (HLS) stop fetching a variant none of whose streams is wanted.
    pub(crate) fn set_discard(self, discard: ff::AVDiscard) {
        unsafe { (*self.0).discard = discard };
    }

    pub(crate) fn find_decoder(self) -> Result<*const ff::AVCodec> {
        let codec = unsafe { ff::avcodec_find_decoder(self.codec_id()) };
        if codec.is_null() {
            return Err(anyhow!("no decoder for codec id {:?}", self.codec_id()));
        }
        Ok(codec)
    }
}

/// Owning wrapper around an `AVCodecContext`.
pub(crate) struct OwnedDecoder(*mut ff::AVCodecContext);

impl OwnedDecoder {
    pub(crate) fn new(codec: *const ff::AVCodec) -> Result<Self> {
        let ptr = unsafe { ff::avcodec_alloc_context3(codec) };
        if ptr.is_null() {
            return Err(anyhow!("avcodec_alloc_context3 failed"));
        }
        Ok(Self(ptr))
    }

    #[allow(clippy::needless_pass_by_ref_mut)] // hands out a mutable alias
    pub(crate) const fn as_mut_ptr(&mut self) -> *mut ff::AVCodecContext {
        self.0
    }
}

impl Drop for OwnedDecoder {
    fn drop(&mut self) {
        unsafe { ff::avcodec_free_context(&mut self.0) };
    }
}

/// Owning wrapper around an `AVFrame`.
///
/// The frame's data buffers are reference counted by FFmpeg, so moving the
/// wrapper across threads (decoder thread to render thread) is safe.
pub(crate) struct OwnedFrame(*mut ff::AVFrame);

unsafe impl Send for OwnedFrame {}

impl OwnedFrame {
    pub(crate) fn new() -> Result<Self> {
        let ptr = unsafe { ff::av_frame_alloc() };
        if ptr.is_null() {
            return Err(anyhow!("av_frame_alloc failed"));
        }
        Ok(Self(ptr))
    }

    #[allow(clippy::needless_pass_by_ref_mut)] // hands out a mutable alias
    pub(crate) const fn as_mut_ptr(&mut self) -> *mut ff::AVFrame {
        self.0
    }

    /// Pixel format for video frames, sample format for audio frames.
    pub(crate) fn format(&self) -> c_int {
        unsafe { (*self.0).format }
    }

    pub(crate) fn width(&self) -> c_int {
        unsafe { (*self.0).width }
    }

    pub(crate) fn height(&self) -> c_int {
        unsafe { (*self.0).height }
    }

    pub(crate) fn best_effort_timestamp(&self) -> i64 {
        unsafe { (*self.0).best_effort_timestamp }
    }

    pub(crate) fn sample_rate(&self) -> c_int {
        unsafe { (*self.0).sample_rate }
    }

    pub(crate) fn nb_samples(&self) -> c_int {
        unsafe { (*self.0).nb_samples }
    }

    /// Channel layout of an audio frame.
    pub(crate) fn ch_layout(&self) -> &ff::AVChannelLayout {
        unsafe { &(*self.0).ch_layout }
    }

    /// Plane pointers of an audio frame, in the shape `swr_convert` takes.
    pub(crate) fn extended_data(&self) -> *const *const u8 {
        unsafe { (*self.0).extended_data.cast::<*const u8>().cast_const() }
    }

    /// Data pointer of one plane; null for planes past
    /// `AV_NUM_DATA_POINTERS`. Hardware frames repurpose the planes (for
    /// D3D11: 0 is the texture, 1 the array slice index).
    pub(crate) fn data(&self, plane: usize) -> *mut u8 {
        // copies only the 8-byte plane pointer, never the plane contents
        match unsafe { (*self.0).data.get(plane) } {
            Some(&pointer) => pointer,
            None => std::ptr::null_mut(),
        }
    }

    /// Hardware frames context of a frame holding hardware surfaces; fails
    /// on software frames, which carry none.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn hw_frames_ctx(&self) -> Result<&ff::AVHWFramesContext> {
        unsafe {
            let buffer = (*self.0).hw_frames_ctx;
            if buffer.is_null() {
                return Err(anyhow!("frame has no hw frames context"));
            }
            Ok(&*(*buffer).data.cast::<ff::AVHWFramesContext>())
        }
    }
}

impl Drop for OwnedFrame {
    fn drop(&mut self) {
        unsafe { ff::av_frame_free(&mut self.0) };
    }
}

/// Owning wrapper around an `AVChannelLayout`.
pub(crate) struct OwnedChannelLayout(ff::AVChannelLayout);

impl OwnedChannelLayout {
    /// Native layout for the given channel count.
    pub(crate) fn default_for(channels: NonZeroI32) -> Self {
        let mut layout: ff::AVChannelLayout = unsafe { std::mem::zeroed() };
        unsafe { ff::av_channel_layout_default(&mut layout, channels.get()) };
        Self(layout)
    }

    /// Deep copy of an existing layout.
    pub(crate) fn copied_from(layout: &ff::AVChannelLayout) -> Result<Self> {
        let mut owned = Self(unsafe { std::mem::zeroed() });
        check("av_channel_layout_copy", unsafe {
            ff::av_channel_layout_copy(&mut owned.0, layout)
        })?;
        Ok(owned)
    }

    pub(crate) fn matches(&self, other: &ff::AVChannelLayout) -> bool {
        unsafe { ff::av_channel_layout_compare(&self.0, other) == 0 }
    }
}

impl AsRef<ff::AVChannelLayout> for OwnedChannelLayout {
    fn as_ref(&self) -> &ff::AVChannelLayout {
        &self.0
    }
}

impl Drop for OwnedChannelLayout {
    fn drop(&mut self) {
        unsafe { ff::av_channel_layout_uninit(&mut self.0) };
    }
}

/// Owning wrapper around an `SwrContext`.
pub(crate) struct OwnedSwr(*mut ff::SwrContext);

impl OwnedSwr {
    /// Allocates and initializes a resampling context converting between
    /// the given formats.
    pub(crate) fn new(
        out_layout: &ff::AVChannelLayout,
        out_format: ff::AVSampleFormat,
        out_rate: NonZeroI32,
        in_layout: &ff::AVChannelLayout,
        in_format: ff::AVSampleFormat,
        in_rate: c_int,
    ) -> Result<Self> {
        // wrapped from the first moment, so a failed init still frees
        // whatever swr_alloc_set_opts2 allocated
        let mut swr = Self(std::ptr::null_mut());
        check("swr_alloc_set_opts2", unsafe {
            ff::swr_alloc_set_opts2(
                &mut swr.0,
                out_layout,
                out_format,
                out_rate.get(),
                in_layout,
                in_format,
                in_rate,
                0,
                std::ptr::null_mut(),
            )
        })?;
        check("swr_init", unsafe { ff::swr_init(swr.0) })?;
        Ok(swr)
    }

    /// Upper bound on the samples buffered inside the resampler, in units
    /// of `1 / base` (pass the input rate to count input samples).
    pub(crate) fn apply_delay_and_modify(&self, base: i64) -> i64 {
        unsafe { ff::swr_get_delay(self.0, base) }
    }

    /// # Safety
    /// `out` must point to writable planes with room for `out_count`
    /// samples in the output format; `input` must point to readable planes
    /// holding `in_count` samples in the input format.
    pub(crate) unsafe fn convert(
        &mut self,
        out: *const *mut u8,
        out_count: c_int,
        input: *const *const u8,
        in_count: c_int,
    ) -> Result<c_int> {
        check("swr_convert", unsafe {
            ff::swr_convert(self.0, out, out_count, input, in_count)
        })
    }
}

impl Drop for OwnedSwr {
    fn drop(&mut self) {
        unsafe { ff::swr_free(&mut self.0) };
    }
}

/// Owning wrapper around an `AVPacket`.
pub(crate) struct OwnedPacket(*mut ff::AVPacket);

impl OwnedPacket {
    pub(crate) fn new() -> Result<Self> {
        let ptr = unsafe { ff::av_packet_alloc() };
        if ptr.is_null() {
            return Err(anyhow!("av_packet_alloc failed"));
        }
        Ok(Self(ptr))
    }

    #[allow(clippy::needless_pass_by_ref_mut)] // hands out a mutable alias
    pub(crate) const fn as_mut_ptr(&mut self) -> *mut ff::AVPacket {
        self.0
    }

    /// Takes over `src`'s payload (`av_packet_move_ref`), leaving `src`
    /// blank and reusable.
    pub(crate) fn stolen_from(src: &mut Self) -> Result<Self> {
        let dst = Self::new()?;
        unsafe { ff::av_packet_move_ref(dst.0, src.0) };
        Ok(dst)
    }

    pub(crate) fn stream_index(&self) -> c_int {
        unsafe { (*self.0).stream_index }
    }

    /// Payload size in bytes.
    pub(crate) fn size(&self) -> usize {
        usize::try_from(unsafe { (*self.0).size }).unwrap_or(0)
    }

    pub(crate) fn unref(&mut self) {
        unsafe { ff::av_packet_unref(self.0) };
    }
}

impl Drop for OwnedPacket {
    fn drop(&mut self) {
        unsafe { ff::av_packet_free(&mut self.0) };
    }
}

impl OwnedFrame {
    /// Row stride in bytes of one plane; 0 for planes past
    /// `AV_NUM_DATA_POINTERS`.
    pub(crate) fn linesize(&self, plane: usize) -> c_int {
        match unsafe { (*self.0).linesize.get(plane) } {
            Some(&stride) => stride,
            None => 0,
        }
    }

    /// The plane pointer array in the shape `sws_scale` reads from.
    pub(crate) fn data_ptrs(&self) -> *const *const u8 {
        unsafe { (*self.0).data.as_ptr().cast::<*const u8>() }
    }

    /// The plane pointer array in the shape `sws_scale` writes to.
    pub(crate) fn data_ptrs_mut(&mut self) -> *const *mut u8 {
        unsafe { (*self.0).data.as_ptr() }
    }

    pub(crate) fn linesizes(&self) -> *const c_int {
        unsafe { (*self.0).linesize.as_ptr() }
    }

    /// Allocates a writable NV12 frame of the given size
    /// (`av_frame_get_buffer`, 32-byte row alignment).
    pub(crate) fn new_nv12(width: c_int, height: c_int) -> Result<Self> {
        ensure!(width > 0 && height > 0, "NV12 frame needs a positive size");
        let mut frame = Self::new()?;
        unsafe {
            (*frame.0).format = ff::AVPixelFormat::AV_PIX_FMT_NV12 as c_int;
            (*frame.0).width = width;
            (*frame.0).height = height;
        }
        check("av_frame_get_buffer(NV12)", unsafe {
            ff::av_frame_get_buffer(frame.as_mut_ptr(), 32)
        })?;
        Ok(frame)
    }

    /// Copies the non-pixel properties (timestamps, color metadata) of
    /// `src` into this frame.
    pub(crate) fn copy_props_from(&mut self, src: &Self) -> Result<()> {
        check("av_frame_copy_props", unsafe {
            ff::av_frame_copy_props(self.0, src.0)
        })?;
        Ok(())
    }

    /// Downloads a hardware frame into a new software frame in the
    /// hardware context's `sw_format`, carrying the timestamps along.
    #[cfg(target_os = "linux")]
    pub(crate) fn transfer_from_hw(src: &Self) -> Result<Self> {
        let mut dst = Self::new()?;
        check("av_hwframe_transfer_data", unsafe {
            ff::av_hwframe_transfer_data(dst.as_mut_ptr(), src.0, 0)
        })?;
        dst.copy_props_from(src)?;
        Ok(dst)
    }
}

impl OwnedPacket {
    /// A new reference to `src`'s payload (`av_packet_ref`); both packets
    /// stay usable.
    pub(crate) fn cloned_from(src: &Self) -> Result<Self> {
        let dst = Self::new()?;
        check("av_packet_ref", unsafe { ff::av_packet_ref(dst.0, src.0) })?;
        Ok(dst)
    }
}

/// `SWS_BILINEAR` from `libswscale/swscale.h`; the bindings expose the
/// flags as an enum whose discriminants the C API takes as a plain int.
const SWS_BILINEAR: c_int = 2;

/// Owning wrapper around a cached `SwsContext`: converts software frames of
/// any pixel format into NV12 at the same size (the presentation format of
/// every platform output), rebuilding the context when the source geometry
/// or format changes.
pub(crate) struct OwnedSws(*mut ff::SwsContext);

// only ever used on the playback thread that owns the decoder
unsafe impl Send for OwnedSws {}

impl OwnedSws {
    pub(crate) const fn new() -> Self {
        Self(std::ptr::null_mut())
    }

    pub(crate) fn scale_to_nv12(&mut self, src: &OwnedFrame) -> Result<OwnedFrame> {
        let (width, height) = (src.width(), src.height());
        ensure!(width > 0 && height > 0, "frame has no size");
        // SAFETY: the value was produced by FFmpeg itself, so it is a valid
        // AVPixelFormat
        let src_format = unsafe { std::mem::transmute::<c_int, ff::AVPixelFormat>(src.format()) };
        let ctx = unsafe {
            ff::sws_getCachedContext(
                self.0,
                width,
                height,
                src_format,
                width,
                height,
                ff::AVPixelFormat::AV_PIX_FMT_NV12,
                SWS_BILINEAR,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        ensure!(
            !ctx.is_null(),
            "sws_getCachedContext({src_format:?} -> NV12, {width}x{height}) failed"
        );
        self.0 = ctx;

        let mut dst = OwnedFrame::new_nv12(width, height)?;
        check("sws_scale", unsafe {
            ff::sws_scale(
                ctx,
                src.data_ptrs(),
                src.linesizes(),
                0,
                height,
                dst.data_ptrs_mut(),
                dst.linesizes(),
            )
        })?;
        dst.copy_props_from(src)?;
        Ok(dst)
    }
}

impl Drop for OwnedSws {
    fn drop(&mut self) {
        unsafe { ff::sws_freeContext(self.0) };
    }
}
