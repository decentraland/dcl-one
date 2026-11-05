use super::*;

/// A frame the tests tell apart by its one byte.
fn frame(kind: FrameKind, byte: u8) -> PulseFrame {
    PulseFrame {
        bytes: vec![byte],
        reliability: PulseReliability::Reliable,
        kind,
    }
}

/// The bytes of each frame, in order: what survived a collapse.
fn bytes(frames: Vec<PulseFrame>) -> Vec<u8> {
    frames.into_iter().flat_map(|frame| frame.bytes).collect()
}

#[test]
fn the_newest_input_of_a_run_survives() {
    let frames = vec![
        frame(FrameKind::Input, 1),
        frame(FrameKind::Input, 2),
        frame(FrameKind::Input, 3),
    ];

    assert_eq!(bytes(collapse_inputs(frames)), [3]);
}

#[test]
fn inputs_separated_by_another_frame_both_go() {
    let frames = vec![
        frame(FrameKind::Input, 1),
        frame(FrameKind::Emote, 2),
        frame(FrameKind::Input, 3),
    ];

    assert_eq!(bytes(collapse_inputs(frames)), [1, 2, 3]);
}

#[test]
fn non_input_frames_are_never_dropped() {
    let frames = vec![
        frame(FrameKind::Control, 1),
        frame(FrameKind::Teleport, 2),
        frame(FrameKind::Teleport, 3),
        frame(FrameKind::Emote, 4),
        frame(FrameKind::Control, 5),
    ];

    assert_eq!(bytes(collapse_inputs(frames)), [1, 2, 3, 4, 5]);
}

#[test]
fn an_empty_batch_stays_empty() {
    assert!(collapse_inputs(Vec::new()).is_empty());
}
