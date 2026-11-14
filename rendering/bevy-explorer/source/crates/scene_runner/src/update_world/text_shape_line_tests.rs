use super::*;

struct LayoutHarness {
    fonts: Assets<Font>,
    font: Handle<Font>,
    pipeline: TextPipeline,
    font_system: CosmicFontSystem,
}

impl LayoutHarness {
    fn new() -> Self {
        let mut fonts = Assets::default();
        let font = fonts.add(
            Font::try_from_bytes(
                include_bytes!(concat!(
                    env!("DCL_ONE_ROOT"),
                    "/rendering/bevy-explorer/source/crates/ui_core/src/fonts/NotoSansMono-Regular.ttf"
                ))
                .to_vec(),
            )
            .unwrap(),
        );
        Self {
            fonts,
            font,
            pipeline: TextPipeline::default(),
            font_system: CosmicFontSystem::default(),
        }
    }

    fn shape(
        &mut self,
        text: &str,
        line_count: Option<i32>,
        width: Option<f32>,
        justify: JustifyText,
    ) -> LaidOutText {
        let spans = [(
            TextSpan::new(text),
            TextFont {
                font: self.font.clone(),
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Some(TextExtras {
                underline: true,
                strike: true,
                mark: Some(Color::BLACK),
            }),
        )];
        layout_text(
            Entity::PLACEHOLDER,
            &spans,
            justify,
            if width.is_some() {
                LineBreak::WordOrCharacter
            } else {
                LineBreak::NoWrap
            },
            width,
            line_count,
            10.0,
            true,
            &mut self.pipeline,
            &self.fonts,
            &mut self.font_system,
        )
        .unwrap()
    }
}

fn assert_visible_prefix(limited: &LaidOutText, complete: &LaidOutText) {
    assert_eq!(
        limited.size, complete.size,
        "line visibility must not reflow alignment"
    );
    assert_eq!(limited.effective_pt, complete.effective_pt);
    assert_eq!(limited.opaque, complete.opaque);
    for (actual, expected) in limited.glyphs.iter().zip(&complete.glyphs) {
        assert_eq!(actual.key, expected.key);
        assert_eq!(actual.pen, expected.pen);
        assert_eq!(actual.color, expected.color);
    }
    for (actual, expected) in limited.bars.iter().zip(&complete.bars) {
        assert_eq!(actual.min, expected.min);
        assert_eq!(actual.max, expected.max);
        assert_eq!(actual.z, expected.z);
        assert_eq!(actual.color, expected.color);
    }
}

#[test]
fn line_count_caps_explicit_lines_and_their_decorations_without_realigning() {
    let mut harness = LayoutHarness::new();
    for justify in [JustifyText::Left, JustifyText::Center, JustifyText::Right] {
        let complete = harness.shape("first\nsecond\nthird\nfourth", None, None, justify);
        let limited = harness.shape("first\nsecond\nthird\nfourth", Some(2), None, justify);
        let visible = harness.shape("first\nsecond", None, None, justify);
        assert!(complete.glyphs.len() > visible.glyphs.len());
        assert_eq!(complete.bars.len(), 12);
        assert_eq!(
            limited.glyphs.len(),
            visible.glyphs.len(),
            "only first and second are emitted"
        );
        assert_eq!(
            limited.bars.len(),
            6,
            "only their three decorations are emitted"
        );
        assert_visible_prefix(&limited, &complete);
    }
}

#[test]
fn line_count_caps_wrapped_visual_lines_instead_of_source_newlines() {
    let mut harness = LayoutHarness::new();
    let single = harness.shape("A", None, None, JustifyText::Left);
    let width = Some(single.size.x + 0.01);
    let complete = harness.shape("ABCDEF", None, width, JustifyText::Left);
    let limited = harness.shape("ABCDEF", Some(2), width, JustifyText::Left);
    assert_eq!(complete.glyphs.len(), 6);
    assert_eq!(complete.bars.len(), 18, "one character per wrapped line");
    assert_eq!(limited.glyphs.len(), 2);
    assert_eq!(limited.bars.len(), 6);
    assert_visible_prefix(&limited, &complete);
}

#[test]
fn line_count_defaults_zero_and_negative_values_keep_the_visibility_contract() {
    let mut harness = LayoutHarness::new();
    let complete = harness.shape("A\nB\nC", None, None, JustifyText::Left);
    for count in [0, i32::MAX] {
        let actual = harness.shape("A\nB\nC", Some(count), None, JustifyText::Left);
        assert_eq!(actual.glyphs.len(), 3);
        assert_eq!(actual.bars.len(), 9);
        assert_visible_prefix(&actual, &complete);
    }
    for count in [1, -1, i32::MIN] {
        let actual = harness.shape("A\nB\nC", Some(count), None, JustifyText::Left);
        assert_eq!(actual.glyphs.len(), 1);
        assert_eq!(actual.bars.len(), 3);
        assert_visible_prefix(&actual, &complete);
    }
}

#[test]
fn line_count_counts_empty_visual_lines() {
    let mut harness = LayoutHarness::new();
    let complete = harness.shape("A\n\nB\nC", None, None, JustifyText::Left);
    let limited = harness.shape("A\n\nB\nC", Some(2), None, JustifyText::Left);
    assert_eq!(complete.glyphs.len(), 3);
    assert_eq!(
        limited.glyphs.len(),
        1,
        "the empty second line consumes the cap"
    );
    assert_eq!(limited.bars.len(), 3);
    assert_visible_prefix(&limited, &complete);
}
