//! Where a nametag's boxes go. Pure: measured text sizes and the bubble's progress in, boxes
//! out. Lengths are tag units (a hundredth of the tag's world scale), x right and y up, with
//! the avatar's anchor at the origin: the idle tag stands on it, and a bubble's tail points
//! at it.

use bevy::math::{Rect, Vec2};

pub const NAME_FONT_SIZE: f32 = 18.0;
pub const MESSAGE_FONT_SIZE: f32 = 16.0;
pub const DM_TAG_FONT_SIZE: f32 = 14.0;
pub const COMMUNITY_FONT_SIZE: f32 = 12.0;
/// A line's height, in font sizes.
pub const LINE_HEIGHT: f32 = 1.21;

/// The widest a message gets before it wraps, unless the header is wider.
pub const MAX_MESSAGE_WIDTH: f32 = 300.0;
/// The widest the DM badge gets.
pub const DM_BADGE_MAX_WIDTH: f32 = 250.0;
/// The height of the community name's row over the header.
pub const COMMUNITY_ROW_HEIGHT: f32 = 19.0;
/// How far the mention ring reaches outside the tag.
pub const MENTION_RING: f32 = 3.0;
/// The tag's corner radius, and its border's width.
pub const CORNER_RADIUS: f32 = 16.0;
pub const BORDER: f32 = 1.0;
/// The side of the square the tail is a corner of, and that corner's radius.
pub const TAIL_SIZE: f32 = 28.0;
pub const TAIL_TIP_RADIUS: f32 = 6.0;
/// The box the tail shows through: its size, and how far its top sits above the tag's bottom.
pub const TAIL_CLIP: Vec2 = Vec2::new(40.0, 28.0);
const TAIL_CLIP_OVERLAP: f32 = 1.0;
/// How far below the clip box's top the tail's rounded tip reaches.
const TAIL_TIP: f32 = TAIL_SIZE * std::f32::consts::FRAC_1_SQRT_2
    - TAIL_TIP_RADIUS * (std::f32::consts::SQRT_2 - 1.0);

/// Top, right, bottom, left.
const PADDING_IDLE: [f32; 4] = [5.0, 2.0, 5.0, 10.0];
const PADDING_MESSAGE: [f32; 4] = [20.0, 16.0, 20.0, 24.0];
const MIN_WIDTH: f32 = 30.0;
/// The space after every child of the header.
const CHILD_GAP: f32 = 4.0;
pub const BADGE_SIZE: f32 = 14.0;
pub const VOICE_SIZE: Vec2 = Vec2::new(13.0, 12.0);
pub const DM_TAG_SIZE: Vec2 = Vec2::new(31.0, 21.0);
const MESSAGE_MARGIN_TOP: f32 = 10.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Voice {
    #[default]
    Off,
    /// In voice chat, not speaking.
    Idle,
    Speaking,
    /// Muted by the local user.
    Hushed,
}

/// The header row: the name and what follows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Header {
    pub name: Vec2,
    pub verified: bool,
    pub official: bool,
    pub voice: Voice,
    /// The DM badge's recipient label; zero for none.
    pub recipient: Vec2,
}

impl Header {
    /// The DM badge's width when nothing limits it.
    fn dm_badge_width(&self) -> f32 {
        DM_TAG_SIZE.x + CHILD_GAP + self.recipient.x
    }

    /// The header's width with the DM badge `dm_badge` wide.
    pub fn width(&self, dm_badge: f32) -> f32 {
        let badges = [
            (self.verified, BADGE_SIZE),
            (self.official, BADGE_SIZE),
            (
                matches!(self.voice, Voice::Idle | Voice::Speaking),
                VOICE_SIZE.x,
            ),
            (self.voice == Voice::Hushed, BADGE_SIZE),
        ];
        self.name.x
            + CHILD_GAP
            + badges
                .into_iter()
                .filter(|(shown, _)| *shown)
                .map(|(_, width)| width + CHILD_GAP)
                .sum::<f32>()
            + dm_badge
            + CHILD_GAP
    }

    /// The width a message wraps at: the header's, with the DM badge open for a DM, when that
    /// is wider than [`MAX_MESSAGE_WIDTH`].
    pub fn message_wrap_width(&self, dm: bool) -> f32 {
        let dm_badge = if dm {
            DM_TAG_SIZE.x + self.recipient.x
        } else {
            0.0
        };
        (self.width(0.0) + dm_badge).max(MAX_MESSAGE_WIDTH)
    }
}

/// The size of the message container showing a message label of `label` size.
pub fn message_container(label: Vec2) -> Vec2 {
    Vec2::new(label.x, label.y + MESSAGE_MARGIN_TOP)
}

/// How far the bubble has got; all zero for the idle tag.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BubbleProgress {
    /// 0 the idle padding, 1 the message's.
    pub padding: f32,
    /// The tail's scale.
    pub tail: f32,
    /// The mention ring's opacity.
    pub mention: f32,
    pub community_height: f32,
    pub dm_max_width: f32,
    /// The message container's size.
    pub container: Vec2,
}

/// Text clipped to a box; `origin` is the text block's top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClippedText {
    pub clip: Rect,
    pub origin: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DmBadge {
    pub clip: Rect,
    pub tag: Rect,
    /// The recipient label's top left.
    pub recipient: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tail {
    /// The top centre of the box the tail shows through: the centre of the tail's square.
    pub origin: Vec2,
    pub scale: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TagLayout {
    /// The tag's box: the background fills it.
    pub root: Rect,
    pub tail: Option<Tail>,
    /// The name's top left.
    pub name: Vec2,
    pub verified: Option<Rect>,
    pub official: Option<Rect>,
    /// The voice bars' box.
    pub voice: Option<Rect>,
    pub hushed: Option<Rect>,
    pub dm: Option<DmBadge>,
    pub community: Option<ClippedText>,
    pub message: Option<ClippedText>,
}

pub fn layout(header: &Header, progress: &BubbleProgress) -> TagLayout {
    let [top, right, bottom, left] = std::array::from_fn(|side| {
        PADDING_IDLE[side] + (PADDING_MESSAGE[side] - PADDING_IDLE[side]) * progress.padding
    });
    let community_height = progress.community_height.max(0.0);
    let dm_width = header.dm_badge_width().min(progress.dm_max_width).max(0.0);
    let header_width = header.width(dm_width);
    let mut header_height = header.name.y.max(BADGE_SIZE);
    if dm_width > 0.0 {
        header_height = header_height.max(DM_TAG_SIZE.y);
    }
    let container = progress.container.max(Vec2::ZERO);
    let content_width = header_width.max(container.x);

    let width = (left + content_width + right).max(MIN_WIDTH);
    let height = top + community_height + header_height + container.y + bottom;
    // the tail's tip stays on the anchor as it grows
    let lift = (TAIL_TIP * progress.tail - TAIL_CLIP_OVERLAP).max(0.0);
    let root = Rect::new(-width * 0.5, lift, width * 0.5, lift + height);

    let tail = (progress.tail > 0.0).then(|| Tail {
        origin: Vec2::new(0.0, root.min.y + TAIL_CLIP_OVERLAP),
        scale: progress.tail,
    });

    let content_left = root.min.x + left;
    let content_top = root.max.y - top;

    let community = (community_height > 0.0).then(|| ClippedText {
        clip: Rect::new(
            content_left,
            content_top - community_height,
            content_left + content_width,
            content_top,
        ),
        origin: Vec2::new(content_left, content_top),
    });

    let header_top = content_top - community_height;
    let header_middle = header_top - header_height * 0.5;
    let mut x = content_left;
    // a header child's box, centred on the row, advancing past it and its gap
    let mut child = |size: Vec2| {
        let rect = Rect::new(
            x,
            header_middle - size.y * 0.5,
            x + size.x,
            header_middle + size.y * 0.5,
        );
        x += size.x + CHILD_GAP;
        rect
    };
    let name = child(header.name);
    let verified = header.verified.then(|| child(Vec2::splat(BADGE_SIZE)));
    let official = header.official.then(|| child(Vec2::splat(BADGE_SIZE)));
    let voice = matches!(header.voice, Voice::Idle | Voice::Speaking).then(|| child(VOICE_SIZE));
    let hushed = (header.voice == Voice::Hushed).then(|| child(Vec2::splat(BADGE_SIZE)));
    let dm = (dm_width > 0.0).then(|| {
        let tag = child(DM_TAG_SIZE);
        let recipient = child(header.recipient);
        DmBadge {
            clip: Rect::new(
                tag.min.x,
                header_top - header_height,
                tag.min.x + dm_width,
                header_top,
            ),
            tag,
            recipient: Vec2::new(recipient.min.x, recipient.max.y),
        }
    });

    let container_top = header_top - header_height;
    let message = (container.x > 0.0 && container.y > 0.0).then(|| ClippedText {
        clip: Rect::new(
            content_left,
            container_top - container.y,
            content_left + container.x,
            container_top,
        ),
        origin: Vec2::new(content_left, container_top - MESSAGE_MARGIN_TOP),
    });

    TagLayout {
        root,
        tail,
        name: Vec2::new(name.min.x, name.max.y),
        verified,
        official,
        voice,
        hushed,
        dm,
        community,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: Vec2 = Vec2::new(60.0, NAME_FONT_SIZE * LINE_HEIGHT);

    fn header() -> Header {
        Header {
            name: NAME,
            ..Default::default()
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn the_idle_tag_stands_on_the_anchor_with_even_sides() {
        let tag = layout(&header(), &BubbleProgress::default());
        // 10 before the name, and the name's gap, the closed DM badge's gap and 2 after it
        assert!(close(tag.root.width(), 60.0 + 20.0));
        assert!(close(tag.root.height(), 5.0 + NAME.y + 5.0));
        assert!(close(tag.root.min.y, 0.0));
        assert!(close(tag.root.center().x, 0.0));
        assert_eq!(
            tag.name,
            Vec2::new(tag.root.min.x + 10.0, tag.root.max.y - 5.0)
        );
        assert!(tag.tail.is_none() && tag.message.is_none() && tag.dm.is_none());
        assert!(tag.verified.is_none() && tag.voice.is_none());
    }

    #[test]
    fn a_short_name_keeps_the_minimum_width() {
        let tag = layout(
            &Header {
                name: Vec2::new(4.0, NAME.y),
                ..Default::default()
            },
            &BubbleProgress::default(),
        );
        assert!(close(tag.root.width(), 30.0));
    }

    #[test]
    fn every_badge_follows_the_name_with_a_gap() {
        let tag = layout(
            &Header {
                verified: true,
                official: true,
                voice: Voice::Speaking,
                ..header()
            },
            &BubbleProgress::default(),
        );
        let name_end = tag.name.x + NAME.x;
        let verified = tag.verified.unwrap();
        let official = tag.official.unwrap();
        let voice = tag.voice.unwrap();
        assert!(close(verified.min.x, name_end + 4.0));
        assert!(verified.size().distance(Vec2::splat(14.0)) < 1e-4);
        assert!(close(official.min.x, verified.max.x + 4.0));
        assert!(close(voice.min.x, official.max.x + 4.0));
        assert!(close(voice.width(), 13.0) && close(voice.height(), 12.0));
        assert!(tag.hushed.is_none());
        // centred on the name's row
        let middle = tag.name.y - NAME.y * 0.5;
        assert!(close(verified.center().y, middle) && close(voice.center().y, middle));
        assert!(close(
            tag.root.width(),
            10.0 + 60.0 + 4.0 + 3.0 * 4.0 + 14.0 + 14.0 + 13.0 + 4.0 + 2.0
        ));
    }

    #[test]
    fn a_hushed_badge_replaces_the_voice_bars() {
        let tag = layout(
            &Header {
                voice: Voice::Hushed,
                ..header()
            },
            &BubbleProgress::default(),
        );
        assert!(tag.voice.is_none());
        assert!(tag.hushed.unwrap().size().distance(Vec2::splat(14.0)) < 1e-4);
    }

    #[test]
    fn a_message_wraps_at_the_wider_of_the_limit_and_the_header() {
        assert!(close(header().message_wrap_width(false), 300.0));
        let wide = Header {
            name: Vec2::new(400.0, NAME.y),
            ..Default::default()
        };
        assert!(close(wide.message_wrap_width(false), 408.0));
        let dm = Header {
            recipient: Vec2::new(90.0, NAME.y),
            ..wide
        };
        assert!(close(dm.message_wrap_width(true), 408.0 + 31.0 + 90.0));
    }

    #[test]
    fn the_shown_bubble_rests_its_tail_on_the_anchor() {
        let label = Vec2::new(120.0, 2.0 * MESSAGE_FONT_SIZE * LINE_HEIGHT);
        let progress = BubbleProgress {
            padding: 1.0,
            tail: 1.0,
            container: message_container(label),
            ..Default::default()
        };
        let tag = layout(&header(), &progress);
        assert!(close(tag.root.width(), 24.0 + 120.0 + 16.0));
        assert!(close(
            tag.root.height(),
            20.0 + NAME.y + label.y + 10.0 + 20.0
        ));

        let tail = tag.tail.unwrap();
        assert!(close(tail.origin.y, tag.root.min.y + 1.0));
        // the tip of the rounded corner, on the anchor
        assert!(close(tail.origin.y - TAIL_TIP, 0.0));
        assert!(close(TAIL_TIP, 17.314));

        let message = tag.message.unwrap();
        assert!(close(message.clip.width(), 120.0));
        assert!(close(message.clip.max.y, tag.name.y - NAME.y));
        assert!(close(message.origin.y, message.clip.max.y - 10.0));
        assert!(close(message.clip.min.y, tag.root.min.y + 20.0));
        assert!(close(message.origin.x, tag.root.min.x + 24.0));
    }

    #[test]
    fn a_community_row_sits_over_the_header() {
        let progress = BubbleProgress {
            padding: 1.0,
            community_height: COMMUNITY_ROW_HEIGHT,
            ..Default::default()
        };
        let tag = layout(&header(), &progress);
        let community = tag.community.unwrap();
        assert!(close(community.clip.height(), 19.0));
        assert!(close(community.clip.max.y, tag.root.max.y - 20.0));
        assert!(close(tag.name.y, community.clip.min.y));
        // clipped to the content's width
        assert!(close(community.clip.width(), header().width(0.0)));
    }

    #[test]
    fn the_dm_badge_opens_to_its_own_width_at_most() {
        let dm = Header {
            recipient: Vec2::new(50.0, NAME.y),
            ..header()
        };
        let half_open = layout(
            &dm,
            &BubbleProgress {
                dm_max_width: 20.0,
                ..Default::default()
            },
        );
        let badge = half_open.dm.unwrap();
        assert!(close(badge.clip.width(), 20.0));
        assert!(badge.tag.size().distance(DM_TAG_SIZE) < 1e-4);
        assert!(close(half_open.root.width(), 80.0 + 20.0));

        let open = layout(
            &dm,
            &BubbleProgress {
                dm_max_width: DM_BADGE_MAX_WIDTH,
                ..Default::default()
            },
        );
        let badge = open.dm.unwrap();
        assert!(close(badge.clip.width(), 31.0 + 4.0 + 50.0));
        assert!(close(badge.recipient.x, badge.tag.max.x + 4.0));
    }

    #[test]
    fn an_overshooting_transition_never_inverts_a_box() {
        let progress = BubbleProgress {
            padding: 1.1,
            tail: 1.1,
            mention: 1.0,
            community_height: -0.5,
            dm_max_width: -2.0,
            container: Vec2::new(-3.0, -3.0),
        };
        let tag = layout(&header(), &progress);
        assert!(tag.root.width() > 0.0 && tag.root.height() > 0.0);
        assert!(tag.message.is_none() && tag.community.is_none() && tag.dm.is_none());
    }
}
