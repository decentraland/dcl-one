//! Nametags: the name over each player, with its badges and, when they write, their message.
//!
//! A tag is an entity of its own with one mesh, drawn in the world by one shared material
//! (nametag.wgsl), facing the camera and scaled to keep its size on screen. Tags are pooled:
//! a player gets one while their name should show, and gives it back otherwise.

pub mod animation;
pub mod bubble;
mod diagnostics;
pub mod layout;
mod material;
mod text;

use alloy_core::primitives::Address;
use bevy::{
    color::Mix, pbr::NotShadowCaster, platform::collections::HashSet, prelude::*,
    render::mesh::MeshTag,
};
use common::{
    sets::{PostUpdateSets, SceneSets},
    structs::{
        AppConfig, AttachPoints, MicState, PlayerModifiers, PrimaryCamera, PrimaryPlayerRes,
        PrimaryUser,
    },
    util::AsH160,
};
use comms::{
    global_crdt::ForeignPlayer,
    nearby_mutes::NearbyMutes,
    profile::{CurrentUserProfile, UserProfile},
    speaking::Speaking,
};
use console::{AddDebugSection, DoAddConsoleCommand};
use scene_runner::update_world::{
    avatar_modifier_area::HiddenNametag,
    text_shape::{AtlasGlyphUse, GlyphAtlases, Page, Pending},
};

use crate::{dynamic_nametag::NametagsVisible, BeyondMaxAvatars};

use self::{
    animation::{BubbleAnimation, BubblePhase, BubbleTarget},
    bubble::{
        capped, message_spans, BubbleKind, ChatBubbleRequest, ChatBubbleRetext,
        MAX_COMMUNITY_NAME_CHARS, MAX_MESSAGE_CHARS, MAX_NAME_CHARS,
    },
    layout::{
        layout, message_container, BubbleProgress, Header, TagLayout, Voice, BORDER,
        COMMUNITY_FONT_SIZE, CORNER_RADIUS, DM_TAG_FONT_SIZE, LINE_HEIGHT, MENTION_RING,
        MESSAGE_FONT_SIZE, NAME_FONT_SIZE,
    },
    material::{opacity_mesh_tag, quad_mesh, Icon, NametagMaterial, Quad},
    text::{glyph_use, GlyphBudget, Shaped, Span, TextShaper},
};

/// A name shows when its player is this far from the camera, in metres.
const MIN_DISTANCE: f32 = 2.0;
const MAX_DISTANCE: f32 = 40.0;
/// Past this distance a tag fades, to nothing at [`MAX_DISTANCE`].
const FADE_DISTANCE: f32 = 20.0;
/// A tag's scale per metre of distance and unit of `tan(fov / 2)`: its size on screen stays
/// the same at any distance and field of view.
const SCREEN_SCALE: f32 = 0.15;
/// A tag unit, in metres, at a scale of one.
const UNIT: f32 = 0.01;
/// A tag is rescaled and refaded once its squared distance has changed by this much.
const DISTANCE_CHANGE: f32 = 1e-4;
/// Tags kept waiting for a player; the rest are despawned.
const MAX_FREE_TAGS: usize = 32;
/// The camera is in first person when it is this close behind the player.
const FIRST_PERSON_DISTANCE: f32 = 0.05;

const BACKGROUND: Color = Color::srgb_u8(0x16, 0x15, 0x18);
const BORDER_COLOR: Color = Color::srgb_u8(0x43, 0x40, 0x4A);
const MENTION_BACKGROUND: Color = Color::srgb_u8(58, 15, 80);
const MENTION_TAIL: Color = Color::srgb_u8(198, 64, 205);
const MENTION_RING_TOP: Color = Color::srgb_u8(0xFF, 0x2D, 0x55);
const MENTION_RING_BOTTOM: Color = Color::srgb_u8(0xC6, 0x40, 0xCD);
/// The ring's gradient is a blend of display colours: this many bands follow it.
const MENTION_RING_BANDS: usize = 8;
const MENTION_RING_RADIUS: f32 = CORNER_RADIUS + MENTION_RING - BORDER;
const SUFFIX_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.4);
const COMMUNITY_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.7);
const VOICE_COLOR: Color = Color::srgb_u8(0x12, 0xE3, 0x4D);
const HUSHED_COLOR: Color = Color::srgb_u8(0xFF, 0x2D, 0x55);
const DM_TAG_COLOR: Color = Color::srgba(54.0 / 255.0, 208.0 / 255.0, 117.0 / 255.0, 0.3);
const DM_TAG_TEXT_COLOR: Color = Color::srgb_u8(28, 238, 124);
const DM_TAG_RADIUS: f32 = 6.0;
/// The voice bars: their width, where each starts in the badge, and the heights a speaker's
/// swing between.
const VOICE_BAR_WIDTH: f32 = 3.0;
const VOICE_BAR_STARTS: [f32; 3] = [0.0, 5.0, 10.0];
const VOICE_BAR_LOW: f32 = 6.0;
const VOICE_BAR_HIGH: f32 = 12.0;

pub struct NametagPlugin;

impl Plugin for NametagPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<NametagMaterial>::default())
            .init_resource::<OfficialWallets>()
            .add_event::<ChatBubbleRequest>()
            .add_event::<ChatBubbleRetext>()
            .add_systems(Startup, load_assets)
            .add_systems(
                Update,
                (
                    resolve_content,
                    resolve_voice,
                    acquire_and_release_tags,
                    show_chat_bubbles,
                    follow_glyph_atlas,
                    build_tags,
                )
                    .chain()
                    .in_set(SceneSets::PostLoop),
            )
            .add_systems(
                PostUpdate,
                place_tags
                    .in_set(PostUpdateSets::Nametag)
                    .after(crate::dynamic_nametag::dynamic_nametag_position),
            )
            .add_observer(forget_content)
            .add_observer(release_tag);
        app.add_console_command::<diagnostics::NametagStateCommand, _>(diagnostics::nametag_state)
            .add_console_command::<diagnostics::NametagBubbleCommand, _>(
                diagnostics::nametag_bubble,
            )
            .add_console_command::<diagnostics::ChatBubbleTextCommand, _>(
                diagnostics::chat_bubble_text,
            )
            .add_console_command::<diagnostics::NametagOfficialCommand, _>(
                diagnostics::nametag_official,
            )
            .add_debug_section(diagnostics::SECTION);
    }
}

/// The wallets whose names carry the official badge. Empty until something fills it.
#[derive(Resource, Default)]
pub struct OfficialWallets(pub HashSet<Address>);

#[derive(Resource)]
pub struct NametagAssets {
    material: Handle<NametagMaterial>,
    font: Handle<Font>,
    /// The glyph atlas pages the material binds, and their sizes.
    bound: [Option<(AssetId<Image>, u32)>; 2],
}

fn load_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut fonts: ResMut<Assets<Font>>,
    mut materials: ResMut<Assets<NametagMaterial>>,
    mut atlases: ResMut<GlyphAtlases>,
    mut images: ResMut<Assets<Image>>,
) {
    let font = Font::try_from_bytes(include_bytes!("fonts/Inter_18pt-SemiBold.ttf").to_vec())
        .expect("the embedded nametag font is a font");
    let atlas = atlases.get_or_create(text::ATLAS, &mut images);
    let page = |page| atlas.page_image(page).map(|(image, _)| image.clone());
    commands.insert_resource(NametagAssets {
        material: materials.add(NametagMaterial {
            sdf: page(Page::Sdf).unwrap_or_default(),
            bitmap: page(Page::Color),
            verified: asset_server.load("embedded://images/nametag/icon-verified.png"),
            official: asset_server.load("embedded://images/nametag/icon-dcl-official.png"),
            hushed: asset_server.load("embedded://images/nametag/icon-hushed.png"),
        }),
        font: fonts.add(font),
        bound: [None; 2],
    });
}

/// On a player: what their nametag says.
#[derive(Component, Clone, Debug, PartialEq)]
#[require(NametagVoice)]
pub struct NametagContent {
    pub name: String,
    /// `#` and the end of the address after a name that isn't claimed; else empty.
    pub suffix: String,
    pub color: Color,
    pub verified: bool,
    pub official: bool,
}

impl NametagContent {
    fn from_profile(profile: &UserProfile, official: &OfficialWallets) -> Self {
        let content = &profile.content;
        let address = &content.eth_address;
        // a guest's profile is its own word, with no catalyst to confirm the claim
        let claimed = content.has_claimed_name && content.has_connected_web3 == Some(true);
        // the tag is always the address' own: whatever follows a `#` in the name is dropped
        let suffix = match address.len().checked_sub(4) {
            Some(last4) if !claimed && address.is_char_boundary(last4) => {
                format!("#{}", &address[last4..])
            }
            _ => String::new(),
        };
        let name = content.name.split('#').next().unwrap_or_default();
        Self {
            name: capped(name, MAX_NAME_CHARS),
            suffix,
            color: profile.name_color(),
            verified: claimed,
            official: address
                .as_h160()
                .is_some_and(|address| official.0.contains(&address)),
        }
    }

    fn spans(&self) -> [Span; 2] {
        [
            Span::new(&self.name, self.color),
            Span::new(&self.suffix, SUFFIX_COLOR),
        ]
    }
}

/// On a player: what their nametag's voice badge shows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NametagVoice(pub Voice);

/// On a tag: the player it is shown for. A tag without one waits in the pool.
#[derive(Component)]
#[relationship(relationship_target = ShownNametag)]
pub struct NametagOf(pub Entity);

/// On a player: the tag showing their name.
#[derive(Component)]
#[relationship_target(relationship = NametagOf)]
pub struct ShownNametag(Entity);

impl ShownNametag {
    pub fn tag(&self) -> Entity {
        self.0
    }
}

/// A tag, shown or pooled, and what its mesh was last built from.
#[derive(Component, Default)]
struct Nametag {
    name: Option<Shaped>,
    /// What `name` was shaped from.
    named: Option<NametagContent>,
    built: Option<Built>,
    /// A transition was still running when the mesh was last built.
    animating: bool,
    /// The squared distance and field of view the tag was last scaled and faded for.
    placed: Option<(f32, f32)>,
}

struct Built {
    content: NametagContent,
    voice: Voice,
    /// The glyph atlas' generation the mesh samples.
    generation: u32,
}

/// On a tag whose mesh waits for glyphs to be rastered.
#[derive(Component)]
struct AwaitingGlyphs;

/// On a tag: the message it shows, from its arrival until the tag is back to idle.
#[derive(Component)]
pub struct ChatBubble {
    kind: BubbleKind,
    message: String,
    message_id: String,
    /// The message mentions the local user.
    mention: bool,
    /// Who the local user's own DM is for, when they are around.
    recipient: Option<NametagContent>,
    stale: Stale,
    texts: Option<BubbleTexts>,
    animation: Option<BubbleAnimation>,
}

/// Why a bubble's labels need laying out again.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stale {
    No,
    /// A message arrived: it shows for its full time.
    Message,
    /// The same message reads differently (a translation, a renamed sender): its time up
    /// stands.
    Text,
}

struct BubbleTexts {
    message: Option<Shaped>,
    community: Option<Shaped>,
    dm_tag: Option<Shaped>,
    recipient: Option<Shaped>,
}

impl ChatBubble {
    fn is_dm(&self) -> bool {
        matches!(self.kind, BubbleKind::Dm { .. })
    }

    fn lay_out(&self, tag: Entity, header: &Header, shaper: &mut TextShaper) -> BubbleTexts {
        let recipient = self.recipient.as_ref().and_then(|recipient| {
            let [name, suffix] = recipient.spans();
            shaper.shape(
                tag,
                &[Span::new("for ", Color::WHITE), name, suffix],
                NAME_FONT_SIZE,
                None,
            )
        });
        let header = Header {
            recipient: recipient.as_ref().map_or(Vec2::ZERO, |shaped| shaped.size),
            ..*header
        };
        let message = shaper.shape(
            tag,
            &message_spans(&self.message),
            MESSAGE_FONT_SIZE,
            Some(header.message_wrap_width(self.is_dm())),
        );
        let message_width = message.as_ref().map_or(0.0, |shaped| shaped.size.x);
        let community = match &self.kind {
            BubbleKind::Community(Some(name)) => shaper.shape_ellipsized(
                tag,
                name,
                COMMUNITY_COLOR,
                COMMUNITY_FONT_SIZE,
                header.width(0.0).max(message_width),
            ),
            _ => None,
        };
        let dm_tag = self
            .is_dm()
            .then(|| {
                shaper.shape(
                    tag,
                    &[Span::new("DM", DM_TAG_TEXT_COLOR)],
                    DM_TAG_FONT_SIZE,
                    None,
                )
            })
            .flatten();
        BubbleTexts {
            message,
            community,
            dm_tag,
            recipient,
        }
    }

    fn target(&self, texts: &BubbleTexts) -> BubbleTarget {
        BubbleTarget {
            container: texts
                .message
                .as_ref()
                .map_or(Vec2::ZERO, |message| message_container(message.size)),
            mention: self.mention,
            community: texts.community.is_some(),
            dm: self.is_dm(),
        }
    }
}

#[allow(clippy::type_complexity)]
fn resolve_content(
    mut commands: Commands,
    official: Res<OfficialWallets>,
    players: Query<
        (Entity, Ref<UserProfile>, Option<&NametagContent>),
        Or<(With<ForeignPlayer>, With<PrimaryUser>)>,
    >,
) {
    for (player, profile, current) in &players {
        if current.is_some() && !profile.is_changed() && !official.is_changed() {
            continue;
        }
        let content = NametagContent::from_profile(&profile, &official);
        if current != Some(&content) {
            commands.entity(player).try_insert(content);
        }
    }
}

fn forget_content(trigger: Trigger<OnRemove, UserProfile>, mut commands: Commands) {
    commands
        .entity(trigger.target())
        .try_remove::<NametagContent>();
}

/// The voice badge of a player: bars while they speak, a hushed mark instead when the local
/// user muted their nearby voice, and dots on the local user while their microphone is open.
fn voice_badge(speaking: Option<Speaking>, muted: bool, local_mic_open: bool) -> Voice {
    match speaking {
        Some(Speaking { in_call: false }) if muted => Voice::Hushed,
        Some(_) => Voice::Speaking,
        None if local_mic_open => Voice::Idle,
        None => Voice::Off,
    }
}

#[allow(clippy::type_complexity)]
fn resolve_voice(
    mic: Res<MicState>,
    mutes: Res<NearbyMutes>,
    mut players: Query<(
        &mut NametagVoice,
        Option<&Speaking>,
        Option<&ForeignPlayer>,
        Has<PrimaryUser>,
    )>,
) {
    for (mut voice, speaking, foreign, is_local) in &mut players {
        let badge = voice_badge(
            speaking.copied(),
            foreign.is_some_and(|player| mutes.is_muted(player.address)),
            is_local && mic.enabled,
        );
        voice.set_if_neq(NametagVoice(badge));
    }
}

/// Where names are seen from.
struct TagView {
    position: Vec3,
    forward: Vec3,
    first_person: bool,
}

impl TagView {
    /// Whether the name of a player standing at `player` shows: in range, in front of the
    /// camera, and not the local player's own seen through their eyes.
    fn shows(&self, player: Vec3, is_local: bool) -> bool {
        let to_player = player - self.position;
        let distance = to_player.length_squared();
        (MIN_DISTANCE * MIN_DISTANCE..=MAX_DISTANCE * MAX_DISTANCE).contains(&distance)
            && self.forward.dot(to_player) >= 0.0
            && !(is_local && self.first_person)
    }
}

fn tag_opacity(distance: f32) -> f32 {
    (1.0 - (distance - FADE_DISTANCE) / (MAX_DISTANCE - FADE_DISTANCE)).clamp(0.0, 1.0)
}

/// The world size of a tag unit `distance` away under a vertical field of view `fov`.
fn tag_scale(distance: f32, fov: f32) -> f32 {
    (fov * 0.5).tan() * SCREEN_SCALE * distance * UNIT
}

#[allow(clippy::type_complexity)]
fn acquire_and_release_tags(
    mut commands: Commands,
    visible: Res<NametagsVisible>,
    assets: Res<NametagAssets>,
    camera: Query<(&Transform, &PrimaryCamera)>,
    players: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&ShownNametag>,
            Has<PrimaryUser>,
            Has<HiddenNametag>,
            Has<BeyondMaxAvatars>,
            Option<&PlayerModifiers>,
        ),
        (With<NametagContent>, With<AttachPoints>),
    >,
    stranded: Query<&ShownNametag, Or<(Without<NametagContent>, Without<AttachPoints>)>>,
    free: Query<Entity, (With<Nametag>, Without<NametagOf>)>,
) {
    let Ok((camera_transform, camera)) = camera.single() else {
        return;
    };
    let view = TagView {
        position: camera_transform.translation,
        forward: camera_transform.forward().as_vec3(),
        first_person: camera.distance <= FIRST_PERSON_DISTANCE,
    };

    let mut free = free.iter();
    for (player, transform, shown, is_local, hidden, beyond_max, modifiers) in &players {
        let wanted = visible.0
            && !hidden
            && !beyond_max
            && !modifiers.is_some_and(|modifiers| modifiers.hide)
            && view.shows(transform.translation(), is_local);
        match (wanted, shown) {
            (true, None) => {
                let tag = free.next().unwrap_or_else(|| {
                    commands
                        .spawn((
                            Nametag::default(),
                            Transform::default(),
                            Visibility::Hidden,
                            Mesh3d::default(),
                            MeshMaterial3d(assets.material.clone()),
                            MeshTag(0),
                            NotShadowCaster,
                        ))
                        .id()
                });
                commands.entity(tag).insert(NametagOf(player));
            }
            (false, Some(shown)) => {
                commands.entity(shown.tag()).remove::<NametagOf>();
            }
            _ => {}
        }
    }
    for shown in &stranded {
        commands.entity(shown.tag()).remove::<NametagOf>();
    }
    for surplus in free.skip(MAX_FREE_TAGS) {
        commands.entity(surplus).despawn();
    }
}

/// A tag going back to the pool (or losing its player) starts over: nothing of the last
/// player's shows on the next.
fn release_tag(
    trigger: Trigger<OnRemove, NametagOf>,
    mut commands: Commands,
    mut tags: Query<(&mut Nametag, &mut Visibility)>,
) {
    let tag = trigger.target();
    let Ok((mut nametag, mut visibility)) = tags.get_mut(tag) else {
        return;
    };
    *nametag = Nametag::default();
    *visibility = Visibility::Hidden;
    commands
        .entity(tag)
        .try_remove::<(ChatBubble, AwaitingGlyphs, AtlasGlyphUse)>();
}

#[allow(clippy::too_many_arguments)]
fn show_chat_bubbles(
    mut commands: Commands,
    mut requests: EventReader<ChatBubbleRequest>,
    mut retexts: EventReader<ChatBubbleRetext>,
    visible: Res<NametagsVisible>,
    config: Res<AppConfig>,
    local_player: Res<PrimaryPlayerRes>,
    local_profile: Res<CurrentUserProfile>,
    shown: Query<&ShownNametag>,
    others: Query<(&ForeignPlayer, &NametagContent)>,
    mut bubbles: Query<&mut ChatBubble>,
) {
    for request in requests.read() {
        if !visible.0 || !request.kind.allowed_by(config.chat_bubble_visibility) {
            continue;
        }
        let Ok(tag) = shown.get(request.sender).map(ShownNametag::tag) else {
            continue;
        };
        let mention = request.sender != local_player.0
            && local_profile.profile.as_ref().is_some_and(|profile| {
                let content = &profile.content;
                bubble::mentions(
                    &request.message,
                    &bubble::mention_name(
                        &content.name,
                        &content.eth_address,
                        content.has_claimed_name,
                    ),
                )
            });
        let recipient = match request.kind {
            BubbleKind::Dm {
                recipient: Some(address),
            } => others
                .iter()
                .find(|(player, _)| player.address == address)
                .map(|(_, content)| content.clone()),
            _ => None,
        };
        let kind = match &request.kind {
            BubbleKind::Community(Some(name)) => {
                BubbleKind::Community(Some(capped(name, MAX_COMMUNITY_NAME_CHARS)))
            }
            kind => kind.clone(),
        };
        let message = capped(&request.message, MAX_MESSAGE_CHARS);
        match bubbles.get_mut(tag) {
            Ok(mut bubble) => {
                bubble.kind = kind;
                bubble.message = message;
                bubble.message_id = request.message_id.clone();
                bubble.mention = mention;
                bubble.recipient = recipient;
                bubble.stale = Stale::Message;
            }
            Err(_) => {
                commands.entity(tag).try_insert(ChatBubble {
                    kind,
                    message,
                    message_id: request.message_id.clone(),
                    mention,
                    recipient,
                    stale: Stale::Message,
                    texts: None,
                    animation: None,
                });
            }
        }
    }

    for retext in retexts.read() {
        let message = capped(&retext.message, MAX_MESSAGE_CHARS);
        for mut bubble in &mut bubbles {
            if bubble.message_id == retext.message_id && bubble.message != message {
                bubble.message = message.clone();
                if bubble.stale == Stale::No {
                    bubble.stale = Stale::Text;
                }
            }
        }
    }
}

/// The atlas' textures are replaced when it is rebuilt, and resized as it grows: the material
/// binds whichever are current.
fn follow_glyph_atlas(
    mut atlases: ResMut<GlyphAtlases>,
    mut images: ResMut<Assets<Image>>,
    mut assets: ResMut<NametagAssets>,
    mut materials: ResMut<Assets<NametagMaterial>>,
) {
    let atlas = atlases.get_or_create(text::ATLAS, &mut images);
    let pages = [Page::Sdf, Page::Color].map(|page| atlas.page_image(page));
    let current = pages.map(|page| page.map(|(image, size)| (image.id(), size)));
    if assets.bound == current {
        return;
    }
    let [sdf, bitmap] = pages.map(|page| page.map(|(image, _)| image.clone()));
    assets.bound = current;
    if let Some(material) = materials.get_mut(&assets.material) {
        material.sdf = sdf.unwrap_or_default();
        material.bitmap = bitmap;
    }
}

#[allow(clippy::type_complexity)]
fn build_tags(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut tags: Query<(
        Entity,
        &NametagOf,
        &mut Nametag,
        Option<&mut ChatBubble>,
        &mut Mesh3d,
        &mut Visibility,
        Has<AwaitingGlyphs>,
    )>,
    owners: Query<(&NametagContent, &NametagVoice)>,
    mut shaper: TextShaper,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let now = time.elapsed_secs_f64();
    let generation = shaper.atlas().generation();
    let mut budget = GlyphBudget::frame();

    for (tag, owner, mut nametag, mut bubble, mut mesh, mut visibility, awaiting) in &mut tags {
        let Ok((content, voice)) = owners.get(owner.0) else {
            continue;
        };
        // shaped once for what it says, however long its glyphs take to be rastered
        let content_changed = nametag.named.as_ref() != Some(content);
        let mut texts_changed = content_changed;
        if content_changed {
            nametag.name = shaper.shape(tag, &content.spans(), NAME_FONT_SIZE, None);
            nametag.named = Some(content.clone());
        }
        let mut dirty = awaiting
            || nametag.animating
            || nametag.built.as_ref().is_none_or(|built| {
                built.content != *content
                    || built.voice != voice.0
                    || built.generation != generation
            });

        let mut header = Header {
            name: nametag
                .name
                .as_ref()
                .map_or(Vec2::new(0.0, NAME_FONT_SIZE * LINE_HEIGHT), |name| {
                    name.size
                }),
            verified: content.verified,
            official: content.official,
            voice: voice.0,
            recipient: Vec2::ZERO,
        };

        let mut progress = BubbleProgress::default();
        let mut animating = false;
        let mut bubble_hidden = false;
        if let Some(bubble) = bubble.as_deref_mut() {
            if content_changed && bubble.texts.is_some() && bubble.stale == Stale::No {
                bubble.stale = Stale::Text;
            }
            if bubble.stale != Stale::No || bubble.texts.is_none() {
                let texts = bubble.lay_out(tag, &header, &mut shaper);
                let target = bubble.target(&texts);
                let stale = bubble.stale;
                match (bubble.animation.as_mut(), stale) {
                    (Some(animation), Stale::Text) => animation.resize(now, target.container),
                    (Some(animation), _) => animation.retarget(now, target),
                    (None, _) => bubble.animation = Some(BubbleAnimation::show(now, target)),
                }
                bubble.texts = Some(texts);
                bubble.stale = Stale::No;
                texts_changed = true;
                dirty = true;
            }
            if let Some(animation) = bubble.animation.as_mut() {
                if animation.tick(now) == BubblePhase::Hidden {
                    commands.entity(tag).remove::<ChatBubble>();
                    // its glyphs are no longer this tag's to keep in the atlas
                    bubble_hidden = true;
                    texts_changed = true;
                    dirty = true;
                } else {
                    animating = !animation.settled(now);
                    dirty |= animating;
                    progress = animation.progress(now);
                    header.recipient = bubble
                        .texts
                        .as_ref()
                        .and_then(|texts| texts.recipient.as_ref())
                        .map_or(Vec2::ZERO, |recipient| recipient.size);
                }
            }
        }
        nametag.animating = animating;
        if !dirty {
            continue;
        }

        let bubble_texts = bubble
            .as_deref()
            .filter(|_| !bubble_hidden)
            .and_then(|bubble| bubble.texts.as_ref());
        if texts_changed {
            commands.entity(tag).insert(glyph_use(
                nametag.name.iter().chain(
                    bubble_texts
                        .into_iter()
                        .flat_map(|texts| {
                            [
                                &texts.message,
                                &texts.community,
                                &texts.dm_tag,
                                &texts.recipient,
                            ]
                        })
                        .flatten(),
                ),
            ));
        }

        let tag_layout = layout(&header, &progress);
        let mut quads = Vec::new();
        let drawn = tag_quads(
            &tag_layout,
            progress.mention,
            voice.0,
            nametag.name.as_ref(),
            bubble_texts,
            &mut shaper,
            &mut budget,
            &mut quads,
        );
        match drawn {
            Ok(()) => {
                let (tag_mesh, aabb) = quad_mesh(&quads);
                // a tag keeps one mesh for as long as it lives
                match meshes.get_mut(&mesh.0) {
                    Some(current) => *current = tag_mesh,
                    None => mesh.0 = meshes.add(tag_mesh),
                }
                let mut tag_commands = commands.entity(tag);
                tag_commands.insert(aabb);
                if awaiting {
                    tag_commands.remove::<AwaitingGlyphs>();
                }
                visibility.set_if_neq(Visibility::Inherited);
                nametag.built = Some(Built {
                    content: content.clone(),
                    voice: voice.0,
                    generation,
                });
            }
            // the last mesh shows until the glyphs are in, unless the atlas it samples has been
            // rebuilt: the shared material already binds the new one, where its UVs point at
            // other glyphs
            Err(Pending { .. }) => {
                if !awaiting {
                    commands.entity(tag).insert(AwaitingGlyphs);
                }
                if nametag
                    .built
                    .as_ref()
                    .is_some_and(|built| built.generation != generation)
                {
                    visibility.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
}

/// A display colour part of the way from `from` to `to`.
fn blend(from: Color, to: Color, amount: f32) -> Color {
    Color::Srgba(from.to_srgba().mix(&to.to_srgba(), amount.clamp(0.0, 1.0)))
}

/// The tag's quads, back to front.
#[allow(clippy::too_many_arguments)]
fn tag_quads(
    tag: &TagLayout,
    mention: f32,
    voice: Voice,
    name: Option<&Shaped>,
    bubble: Option<&BubbleTexts>,
    shaper: &mut TextShaper,
    budget: &mut GlyphBudget,
    quads: &mut Vec<Quad>,
) -> Result<(), Pending> {
    let mention = mention.clamp(0.0, 1.0);

    if let Some(tail) = tag.tail {
        quads.push(Quad::tail(tail, blend(BORDER_COLOR, MENTION_TAIL, mention)));
    }
    if mention > 0.0 {
        let ring = tag.root.inflate(MENTION_RING);
        let whole = Quad::rect(ring, MENTION_RING_RADIUS, Color::WHITE);
        let band_height = ring.height() / MENTION_RING_BANDS as f32;
        let color_at = |band: usize| {
            blend(
                MENTION_RING_BOTTOM,
                MENTION_RING_TOP,
                band as f32 / MENTION_RING_BANDS as f32,
            )
            .with_alpha(mention)
        };
        quads.extend((0..MENTION_RING_BANDS).filter_map(|band| {
            let bottom = ring.min.y + band_height * band as f32;
            let top = if band + 1 == MENTION_RING_BANDS {
                ring.max.y
            } else {
                bottom + band_height
            };
            let band_rect = Rect::new(ring.min.x, bottom, ring.max.x, top);
            Some(
                whole
                    .clipped(band_rect)?
                    .with_colors(color_at(band), color_at(band + 1)),
            )
        }));
    }
    quads.push(Quad::rect(tag.root, CORNER_RADIUS, BORDER_COLOR));
    quads.push(Quad::rect(
        tag.root.inflate(-BORDER),
        CORNER_RADIUS - BORDER,
        blend(BACKGROUND, MENTION_BACKGROUND, mention),
    ));

    if let (Some(community), Some(shaped)) = (
        tag.community,
        bubble.and_then(|texts| texts.community.as_ref()),
    ) {
        shaper.quads(
            shaped,
            community.origin,
            Some(community.clip),
            budget,
            quads,
        )?;
    }

    if let Some(name) = name {
        shaper.quads(name, tag.name, None, budget, quads)?;
    }
    if let Some(rect) = tag.verified {
        quads.push(Quad::icon(rect, Icon::Verified, Color::WHITE));
    }
    if let Some(rect) = tag.official {
        quads.push(Quad::icon(rect, Icon::Official, Color::WHITE));
    }
    if let Some(rect) = tag.voice {
        for (index, start) in VOICE_BAR_STARTS.into_iter().enumerate() {
            let left = rect.min.x + start;
            if voice == Voice::Speaking {
                // the middle bar swings against the outer two
                let (from, to) = if index == 1 {
                    (VOICE_BAR_HIGH, VOICE_BAR_LOW)
                } else {
                    (VOICE_BAR_LOW, VOICE_BAR_HIGH)
                };
                quads.push(Quad::voice_bar(
                    Rect::new(left, rect.min.y, left + VOICE_BAR_WIDTH, rect.max.y),
                    from,
                    to,
                    VOICE_COLOR,
                ));
            } else {
                let middle = rect.center().y;
                quads.push(Quad::rect(
                    Rect::new(
                        left,
                        middle - VOICE_BAR_WIDTH * 0.5,
                        left + VOICE_BAR_WIDTH,
                        middle + VOICE_BAR_WIDTH * 0.5,
                    ),
                    VOICE_BAR_WIDTH * 0.5,
                    VOICE_COLOR,
                ));
            }
        }
    }
    if let Some(rect) = tag.hushed {
        quads.push(Quad::icon(rect, Icon::Hushed, HUSHED_COLOR));
    }

    if let Some(dm) = tag.dm {
        quads.extend(Quad::rect(dm.tag, DM_TAG_RADIUS, DM_TAG_COLOR).clipped(dm.clip));
        if let Some(label) = bubble.and_then(|texts| texts.dm_tag.as_ref()) {
            let centre = dm.tag.center();
            let origin = Vec2::new(centre.x - label.size.x * 0.5, centre.y + label.size.y * 0.5);
            shaper.quads(label, origin, Some(dm.clip), budget, quads)?;
        }
        if let Some(recipient) = bubble.and_then(|texts| texts.recipient.as_ref()) {
            shaper.quads(recipient, dm.recipient, Some(dm.clip), budget, quads)?;
        }
    }

    if let (Some(message), Some(shaped)) =
        (tag.message, bubble.and_then(|texts| texts.message.as_ref()))
    {
        shaper.quads(shaped, message.origin, Some(message.clip), budget, quads)?;
    }
    Ok(())
}

/// Stand each tag on its player's anchor, facing the camera, at the size and opacity its
/// distance calls for.
#[allow(clippy::type_complexity)]
fn place_tags(
    camera: Query<(&Transform, &Projection), (With<PrimaryCamera>, Without<Nametag>)>,
    owners: Query<(&GlobalTransform, &AttachPoints, Has<ChildOf>)>,
    anchors: Query<&Transform, Without<Nametag>>,
    mut tags: Query<(
        &NametagOf,
        &mut Nametag,
        &mut Transform,
        &mut MeshTag,
        &mut Mesh3d,
    )>,
) {
    // the camera's own transform: its global one is a frame behind here, and it has no parent
    let Ok((camera, projection)) = camera.single() else {
        return;
    };
    let Projection::Perspective(perspective) = projection else {
        return;
    };

    for (owner, mut nametag, mut transform, mut mesh_tag, mut mesh) in &mut tags {
        let Ok((global, attach_points, parented)) = owners.get(owner.0) else {
            continue;
        };
        // a player with no parent is where its own transform says: its global one is a frame
        // behind here once the player has moved
        let player = match anchors.get(owner.0) {
            Ok(local) if !parented => GlobalTransform::from(*local),
            _ => *global,
        };
        let Ok(anchor) = anchors.get(attach_points.nametag) else {
            continue;
        };
        transform.translation = player.transform_point(anchor.translation);
        transform.rotation = camera.rotation;

        let distance = camera.translation.distance_squared(player.translation());
        if nametag.placed.is_some_and(|(placed, fov)| {
            (placed - distance).abs() < DISTANCE_CHANGE && fov == perspective.fov
        }) {
            continue;
        }
        nametag.placed = Some((distance, perspective.fov));
        let distance = distance.sqrt();
        transform.scale = Vec3::splat(tag_scale(distance, perspective.fov));
        let opacity = opacity_mesh_tag(tag_opacity(distance));
        if mesh_tag.0 != opacity {
            mesh_tag.0 = opacity;
            // the mesh tag reaches the renderer only with a changed mesh
            mesh.set_changed();
        }
    }
}

#[cfg(test)]
mod tests;
