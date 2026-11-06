#[cfg(any(feature = "ffmpeg", feature = "html"))]
mod ffmpeg_or_html;

// This crate links comms, and with it libwebrtc. Keep this test here, not in `media`: libwebrtc
// bundles chromium's ffmpeg, and if those symbols shadow the ffmpeg the engine links, https
// sourcing and decoding fail in exactly this binary and nowhere ffmpeg is linked on its own.
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
#[test]
fn test_ffmpeg() {
    let context = ffmpeg_next::format::input(
        &"https://vz-7c61c1b5-d59.b-cdn.net/ccea595a-b910-4de6-b160-092819db021d/play_480p.mp4"
            .to_owned(),
    )
    .unwrap();
    let stream = context
        .streams()
        .best(ffmpeg_next::media::Type::Video)
        .unwrap();
    ffmpeg_next::codec::context::Context::from_parameters(stream.parameters())
        .unwrap()
        .decoder()
        .video()
        .unwrap();
}

/// The threads behind the players above outlive their app; once it is gone they must leave
/// promptly, which is what the client's exit relies on.
///
/// The join covers every av thread in the process, so this assumes the tests run one at a
/// time (`--test-threads=1`, as CI does); alongside another test's live app it can time out.
#[cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
#[test]
fn av_threads_leave_once_their_app_is_gone() {
    ffmpeg_or_html::test_source_change();
    assert!(crate::video_stream::join_av_threads(
        std::time::Duration::from_secs(10)
    ));
}
