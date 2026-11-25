//! Nametags: the name over each player and scene avatar, with its badges and, when a player
//! writes, their message.
//!
//! A tag is a child of its player or avatar with one mesh, drawn in the world by one shared material
//! (nametag.wgsl). It stands on the player's nametag anchor; the shader turns it to the camera,
//! keeps its size on screen and fades it with distance. A name that should not show is hidden,
//! and its tag goes when its player does.

pub mod animation;
pub mod bubble;
mod diagnostics;
pub mod layout;
mod material;
mod plate;
mod text;

use alloy_core::primitives::Address;
use bevy::{
    app::PropagateStop, color::Mix, ecs::system::SystemParam, math::Vec3A, pbr::NotShadowCaster,
    platform::collections::HashSet, prelude::*, render::primitives::Aabb,
};
use common::{
    feature_flags::{FeatureFlags, FlagsSource},
    sets::{PostUpdateSets, SceneSets},
    structs::{
        AppConfig, AttachPoints, CameraView, MicState, PlayerModifiers, PrimaryCamera,
        PrimaryPlayerRes, PrimaryUser,
    },
    util::AsH160,
};
use comms::{
    global_crdt::ForeignPlayer,
    nearby_mutes::NearbyMutes,
    profile::{name_color::name_color, CurrentUserProfile, UserProfile},
    speaking::Speaking,
    BlockedOrBanned,
};
use console::{AddDebugSection, DoAddConsoleCommand};
use dcl_component::{
    proto_components::sdk::components::{PbAvatarNametag, PbAvatarShape},
    SceneComponentId, SceneEntityId,
};
use dcl_types::interface::ComponentPosition;
use scene_material::highlight::Highlight;
use scene_runner::{
    initialize_scene::SuperUserScene,
    renderer_context::RendererSceneContext,
    update_world::{
        avatar_modifier_area::HiddenNametag,
        text_shape::{GlyphAtlasMaintenance, GlyphAtlases, Page, Pending},
        AddCrdtInterfaceExt,
    },
    SceneEntity,
};
use system_bridge::{AvatarShapeNametag, SystemApi};

use crate::{
    dynamic_nametag::{avatar_units, smallest_scale, NametagHeadOffset, NametagsVisible},
    AvatarSelection, AvatarShape, BeyondMaxAvatars,
};

use self::{
    animation::{BubbleAnimation, BubblePhase, BubbleTarget, BUBBLE_LIFETIME},
    bubble::{
        bubble_text, capped, message_spans, BubbleKind, BubbleText, ChatBubbleRequest,
        ChatBubbleRetext, MAX_COMMUNITY_NAME_CHARS, MAX_MESSAGE_CHARS, MAX_NAME_CHARS,
    },
    layout::{
        layout, message_container, BubbleProgress, Header, PlateLayout, TagLayout, Voice, BORDER,
        COMMUNITY_FONT_SIZE, CORNER_RADIUS, DM_TAG_FONT_SIZE, LINE_HEIGHT, MENTION_RING,
        MESSAGE_FONT_SIZE, NAME_FONT_SIZE, PLATE_BORDER, PLATE_FONT_SIZE, PLATE_MAX_LABEL_WIDTH,
        PLATE_RADIUS,
    },
    material::{quad_mesh, Icon, NametagMaterial, Quad},
    plate::{resolve_plates, AvatarNametag, NametagPlate},
    text::{glyph_use, GlyphBudget, Shaped, Span, TextShaper},
};

/// The name of a scene's avatar that the scene gave none.
const DEFAULT_SCENE_AVATAR_NAME: &str = "NPC";
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
        // blended and never a shadow caster: the prepass and the shadow passes draw none of it
        app.add_plugins(MaterialPlugin::<NametagMaterial> {
            prepass_enabled: false,
            shadows_enabled: false,
            ..default()
        })
        .init_resource::<OfficialWallets>()
        .add_event::<ChatBubbleRequest>()
        .add_event::<ChatBubbleRetext>()
        .add_systems(Startup, load_assets)
        .add_systems(
            Update,
            fill_official_wallets.run_if(resource_exists_and_changed::<FeatureFlags>),
        )
        .add_systems(
            Update,
            (
                resolve_content,
                set_system_scene_nametags,
                resolve_scene_avatar_content,
                resolve_plates,
                resolve_voice,
                show_tags,
                retext_from_bridge,
                show_chat_bubbles,
                follow_glyph_atlas,
                build_tags,
            )
                .chain()
                .in_set(SceneSets::PostLoop)
                // a rebuilt atlas has moved every glyph: bind and build from where they are now
                .after(GlyphAtlasMaintenance),
        )
        .add_systems(Update, answer_official_wallets.in_set(SceneSets::PostLoop))
        .add_systems(
            PostUpdate,
            place_tags
                .in_set(PostUpdateSets::Nametag)
                .after(crate::dynamic_nametag::dynamic_nametag_position),
        )
        .add_observer(forget_content)
        .add_observer(spawn_tag)
        .add_observer(despawn_unnamed_tag)
        .add_observer(despawn_unplated_tag);
        app.add_crdt_lww_component::<PbAvatarNametag, AvatarNametag>(
            SceneComponentId::AVATAR_NAMETAG,
            ComponentPosition::Any,
        );
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

/// The wallets whose names carry the official badge: the `alfa-official-wallets` flag's
/// addresses once the feature-flags read settles, plus any `/nametag_official` adds this session.
#[derive(Resource, Default)]
pub struct OfficialWallets(pub HashSet<Address>);

/// The flag whose `wallets` variant payload (type `csv`) lists the official wallets.
const OFFICIAL_WALLETS_FLAG: &str = "alfa-official-wallets";

/// The addresses in a csv payload, separated by commas or line breaks. Case does not matter and
/// an entry that is not an address is skipped.
fn parse_wallets(csv: &str) -> HashSet<Address> {
    csv.split([',', '\n', '\r'])
        .filter_map(|entry| entry.trim().as_h160())
        .collect()
}

/// Adds the flag's wallets to [`OfficialWallets`] once the feature flags have settled.
fn fill_official_wallets(flags: Res<FeatureFlags>, mut wallets: ResMut<OfficialWallets>) {
    if flags.source() == FlagsSource::Pending {
        return;
    }
    let Some(payload) = flags
        .payload(OFFICIAL_WALLETS_FLAG, "wallets")
        .filter(|payload| payload.kind == "csv")
    else {
        return;
    };
    wallets.0.extend(parse_wallets(&payload.value));
}

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

/// On a player or a scene avatar: what their nametag says.
#[derive(Component, Clone, Debug, PartialEq)]
#[require(NametagVoice)]
pub struct NametagContent {
    pub name: String,
    /// `#` and the end of the address after a name that isn't claimed, or of a scene
    /// avatar's id; else empty.
    pub suffix: String,
    pub color: Color,
    pub verified: bool,
    pub official: bool,
    /// How large the tag draws on screen, as a multiple of a player's: 1 but on a system scene's
    /// avatar that was given a larger one ([`SystemSceneNametag`]).
    pub scale: f32,
}

/// The sizes a system scene may give its avatar's tag, as a multiple of a player's.
const SYSTEM_TAG_SCALES: std::ops::RangeInclusive<f32> = 0.25..=4.0;

impl NametagContent {
    fn from_profile(profile: &UserProfile, official: &OfficialWallets) -> Self {
        let content = &profile.content;
        // one spelling of the address for the tag, the colour and the badge
        let address = &content.eth_address.to_lowercase();
        // the flags are the peer's word, which nothing here checks against the catalyst: this
        // only keeps the seal off a profile that says itself it has no wallet behind it
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
            scale: 1.0,
        }
    }

    /// What a scene's avatar shows: its name and the end of its id. `None` for a name the
    /// scene leaves unset or blank, which asks for no tag. As in a player's name, whatever
    /// follows a `#` is dropped; a name with nothing before one reads `NPC`.
    fn from_shape(shape: &PbAvatarShape) -> Option<Self> {
        let given = shape
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty())?;
        let name = match given.split('#').next().unwrap_or_default().trim() {
            "" => DEFAULT_SCENE_AVATAR_NAME,
            name => name,
        };
        let name = capped(name, MAX_NAME_CHARS);
        Some(Self {
            color: name_color(&name, &shape.id, false),
            name,
            suffix: id_suffix(&shape.id),
            verified: false,
            official: false,
            scale: 1.0,
        })
    }

    /// What a system scene's avatar shows: the tag the scene put on it. A claimed NAME carries
    /// the verified badge and no suffix; one that isn't, the end of the avatar's id, as a scene
    /// avatar's does. The colour is the given one, else the engine's for that name. `None` for
    /// a blank name, which asks for no tag.
    fn from_system(tag: &AvatarShapeNametag, shape: &PbAvatarShape) -> Option<Self> {
        let name = tag.name.split('#').next().unwrap_or_default().trim();
        if name.is_empty() {
            return None;
        }
        let name = capped(name, MAX_NAME_CHARS);
        let color = match &tag.color {
            Some(color) => Color::srgb(color.r, color.g, color.b),
            None => name_color(&name, &shape.id, tag.claimed),
        };
        Some(Self {
            color,
            suffix: match tag.claimed {
                true => String::new(),
                false => id_suffix(&shape.id),
            },
            name,
            verified: tag.claimed,
            official: false,
            scale: match tag.scale.is_finite() {
                true => tag
                    .scale
                    .clamp(*SYSTEM_TAG_SCALES.start(), *SYSTEM_TAG_SCALES.end()),
                false => 1.0,
            },
        })
    }

    fn spans(&self) -> [Span; 2] {
        [
            Span::new(&self.name, self.color),
            Span::new(&self.suffix, SUFFIX_COLOR),
        ]
    }
}

/// `#` and the last four characters of a scene avatar's id; zeros for an id shorter than that.
fn id_suffix(id: &str) -> String {
    match id.char_indices().rev().nth(3) {
        // never cut: `capped` is here to keep a control character off the tag
        Some((last4, _)) => format!("#{}", capped(&id[last4..], 4)),
        None => "#0000".to_owned(),
    }
}

/// On one of a system scene's avatars: the tag the scene put on it
/// (`SystemApi::SetAvatarShapeNametag`). A system scene's avatars get no tag from their
/// [`AvatarShape`]'s name, only this way. It goes when the shape is removed.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct SystemSceneNametag(pub AvatarShapeNametag);

/// On a scene's avatar whose [`AvatarShape`] was read for its nametag, until the shape changes.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneAvatarNametag {
    /// The shape gives the avatar a [`NametagContent`]; a blank name gives none.
    pub named: bool,
}

/// On a player or a scene avatar: what their nametag's voice badge shows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NametagVoice(pub Voice);

/// A player's tag, a child of the player, and what its mesh was last built from.
#[derive(Component, Default)]
struct Nametag {
    /// The tag should show; one that should not is hidden and left as it is.
    shown: bool,
    /// The name should show, on a tag that does: without it the tag is its plate alone.
    name_shown: bool,
    name: Option<Shaped>,
    /// What `name` was shaped from.
    named: Option<NametagContent>,
    /// The plate's label; `None` for a plate with nothing written on it.
    plate: Option<Shaped>,
    /// What `plate` was shaped from.
    plated: Option<NametagPlate>,
    built: Option<Built>,
    /// The mesh waits for glyphs to be rastered.
    awaiting: bool,
    /// How far the mesh reaches from the anchor, in tag units.
    reach: f32,
}

/// What a tag's mesh was built from: the tag is rebuilt when any of it differs.
struct Built {
    /// The name drawn; `None` for a plate alone.
    content: Option<NametagContent>,
    plate: Option<NametagPlate>,
    voice: Voice,
    /// The glyph atlas' generation the mesh samples.
    generation: u32,
    /// How far the bubble had got.
    progress: BubbleProgress,
}

/// On a player: the message their tag shows, from its arrival until the tag is back to idle.
/// It is the player's, so a tag hidden for a moment still shows it when it is back.
#[derive(Component)]
pub struct ChatBubble {
    kind: BubbleKind,
    message: BubbleText,
    message_id: String,
    /// When the message came, in seconds of real time: its time up counts from here.
    arrived: f64,
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

impl BubbleTexts {
    fn iter(&self) -> impl Iterator<Item = &Shaped> {
        [
            &self.message,
            &self.community,
            &self.dm_tag,
            &self.recipient,
        ]
        .into_iter()
        .flatten()
    }
}

/// Where a bubble has got to this frame.
#[derive(Default)]
struct BubbleStep {
    progress: BubbleProgress,
    /// Its labels were laid out again, or are gone.
    texts_changed: bool,
    /// The tag is back to idle: the message can be dropped.
    over: bool,
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

    /// Lay the labels out again when what they say changed, and move the transitions on to
    /// `now`. The header gets the DM recipient's size while the bubble is up.
    fn advance(
        &mut self,
        tag: Entity,
        header: &mut Header,
        content_changed: bool,
        now: f64,
        shaper: &mut TextShaper,
    ) -> BubbleStep {
        if content_changed && self.texts.is_some() && self.stale == Stale::No {
            self.stale = Stale::Text;
        }
        let mut texts_changed = false;
        if self.stale != Stale::No || self.texts.is_none() {
            let texts = self.lay_out(tag, header, shaper);
            let target = self.target(&texts);
            let hide_at = self.arrived + BUBBLE_LIFETIME;
            match (self.animation.as_mut(), self.stale) {
                (Some(animation), Stale::Text) => animation.resize(now, target.container),
                (Some(animation), _) => animation.retarget(now, target, hide_at),
                (None, _) => self.animation = Some(BubbleAnimation::show(now, target, hide_at)),
            }
            self.texts = Some(texts);
            self.stale = Stale::No;
            texts_changed = true;
        }
        let Some(animation) = self.animation.as_mut() else {
            return BubbleStep {
                texts_changed,
                ..default()
            };
        };
        if animation.tick(now) == BubblePhase::Hidden {
            return BubbleStep {
                texts_changed: true,
                over: true,
                ..default()
            };
        }
        header.recipient = self
            .texts
            .as_ref()
            .and_then(|texts| texts.recipient.as_ref())
            .map_or(Vec2::ZERO, |recipient| recipient.size);
        BubbleStep {
            progress: animation.progress(now),
            texts_changed,
            over: false,
        }
    }

    /// Whether the message's time is up, for a tag that is not showing it: nobody sees it
    /// come down.
    fn over(&self, now: f64) -> bool {
        now >= self.arrived + BUBBLE_LIFETIME
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

/// The system scene's word on the tag of one of its avatars: put on it, or taken away. The
/// entity is read in the system scene's own context, so no other scene's entity can be reached.
/// The scene's `AvatarShape` may reach the engine after its word: the tag waits on the entity,
/// and shows once the avatar is built.
fn set_system_scene_nametags(
    mut commands: Commands,
    mut requests: EventReader<SystemApi>,
    system_scenes: Query<&RendererSceneContext, With<SuperUserScene>>,
    tagged: Query<Option<&SystemSceneNametag>>,
) {
    for request in requests.read() {
        let SystemApi::SetAvatarShapeNametag { entity, nametag } = request else {
            continue;
        };
        let entity = SceneEntityId::from_proto_u32(*entity);
        let Some((avatar, current)) = system_scenes
            .iter()
            .filter_map(|context| context.bevy_entity(entity))
            .find_map(|avatar| tagged.get(avatar).ok().map(|current| (avatar, current)))
        else {
            debug!("the system scene has no entity {entity}: its nametag is dropped");
            continue;
        };
        match nametag {
            Some(nametag) if current.is_none_or(|current| current.0 != *nametag) => {
                commands
                    .entity(avatar)
                    .try_insert(SystemSceneNametag(nametag.clone()));
            }
            None if current.is_some() => {
                commands.entity(avatar).try_remove::<SystemSceneNametag>();
            }
            _ => {}
        }
    }
}

/// Names the avatars a scene made, while they have a body that is built: without one the tag
/// would sit at their feet. A scene entity that only restyles a player has no
/// [`AvatarSelection`] and gets no tag. The avatars the HUD previews, which a system scene
/// makes, show only the tag the system scene puts on them ([`SystemSceneNametag`]), never one
/// from their name. One the scene marks as talking is [`Speaking`], as in a call: a scene that
/// turns nearby voice off leaves its bars up.
fn resolve_scene_avatar_content(
    mut commands: Commands,
    avatars: Query<
        (
            Entity,
            &SceneEntity,
            Ref<AvatarShape>,
            Option<Ref<SystemSceneNametag>>,
            Option<&NametagContent>,
            Option<&SceneAvatarNametag>,
            Has<Speaking>,
        ),
        (
            With<AvatarSelection>,
            With<NametagHeadOffset>,
            Without<ForeignPlayer>,
            Without<PrimaryUser>,
        ),
    >,
    system_scenes: Query<(), With<SuperUserScene>>,
    mut untagged: RemovedComponents<SystemSceneNametag>,
    dropped: Query<
        Entity,
        (
            With<SceneAvatarNametag>,
            Or<(
                Without<AvatarShape>,
                Without<AvatarSelection>,
                Without<NametagHeadOffset>,
            )>,
        ),
    >,
    mut unshaped: RemovedComponents<AvatarShape>,
    system_tagged: Query<(), With<SystemSceneNametag>>,
) {
    // a system scene took its tag away: the avatar is read again
    let untagged: HashSet<Entity> = untagged.read().collect();
    for (avatar, scene_entity, shape, system_tag, current, marker, speaking) in &avatars {
        if marker.is_some()
            && !shape.is_changed()
            && !system_tag.as_ref().is_some_and(Ref::is_changed)
            && !untagged.contains(&avatar)
        {
            continue;
        }
        let content = match system_scenes.contains(scene_entity.root) {
            true => system_tag.and_then(|tag| NametagContent::from_system(&tag.0, &shape.0)),
            false => NametagContent::from_shape(&shape.0),
        };
        // an avatar with no tag is marked too: its shape is not read again until it changes
        let resolved = SceneAvatarNametag {
            named: content.is_some(),
        };
        if marker != Some(&resolved) {
            commands.entity(avatar).try_insert(resolved);
        }
        match content.is_some() && shape.0.talking == Some(true) {
            true if !speaking => {
                commands
                    .entity(avatar)
                    .try_insert(Speaking { in_call: true });
            }
            false if speaking => {
                commands.entity(avatar).try_remove::<Speaking>();
            }
            _ => {}
        }
        match content {
            Some(content) if current != Some(&content) => {
                commands.entity(avatar).try_insert(content);
            }
            None if marker.is_some_and(|marker| marker.named) => {
                commands.entity(avatar).try_remove::<NametagContent>();
            }
            _ => {}
        }
    }
    for avatar in &dropped {
        commands
            .entity(avatar)
            .try_remove::<(NametagContent, SceneAvatarNametag, Speaking)>();
    }
    // the tag a system scene put on its avatar goes with the avatar's shape
    for avatar in unshaped.read() {
        if system_tagged.contains(avatar) {
            commands.entity(avatar).try_remove::<SystemSceneNametag>();
        }
    }
}

fn forget_content(trigger: Trigger<OnRemove, UserProfile>, mut commands: Commands) {
    commands
        .entity(trigger.target())
        .try_remove::<NametagContent>();
}

/// An avatar with a name or a plate to show gets its tag, as its child: it goes when they do.
/// One tag, whichever comes first.
fn spawn_tag(
    trigger: Trigger<OnAdd, (NametagContent, NametagPlate)>,
    mut commands: Commands,
    assets: Res<NametagAssets>,
) {
    let owner = trigger.target();
    let material = assets.material.clone();
    commands.queue(move |world: &mut World| {
        let tagged = world.get::<Children>(owner).is_some_and(|children| {
            children
                .iter()
                .any(|child| world.get::<Nametag>(child).is_some())
        });
        let Ok(mut owner) = world.get_entity_mut(owner) else {
            return;
        };
        if tagged {
            return;
        }
        owner.with_child((
            Nametag::default(),
            Transform::default(),
            Visibility::Hidden,
            Mesh3d::default(),
            MeshMaterial3d(material),
            Aabb::default(),
            NotShadowCaster,
            // the hover glow is the avatar's
            PropagateStop::<Highlight>::default(),
        ));
    });
}

/// The tag is its owner's own child.
fn despawn_tags_of(
    owner: Entity,
    commands: &mut Commands,
    children: &Query<&Children>,
    tags: &Query<(), With<Nametag>>,
) {
    for child in children.get(owner).into_iter().flatten() {
        if tags.contains(*child) {
            commands.entity(*child).try_despawn();
        }
    }
}

fn despawn_unnamed_tag(
    trigger: Trigger<OnRemove, NametagContent>,
    mut commands: Commands,
    children: Query<&Children>,
    plated: Query<(), With<NametagPlate>>,
    tags: Query<(), With<Nametag>>,
) {
    let owner = trigger.target();
    if !plated.contains(owner) {
        despawn_tags_of(owner, &mut commands, &children, &tags);
    }
    commands.entity(owner).try_remove::<ChatBubble>();
}

fn despawn_unplated_tag(
    trigger: Trigger<OnRemove, NametagPlate>,
    mut commands: Commands,
    children: Query<&Children>,
    named: Query<(), With<NametagContent>>,
    tags: Query<(), With<Nametag>>,
) {
    let owner = trigger.target();
    if !named.contains(owner) {
        despawn_tags_of(owner, &mut commands, &children, &tags);
    }
}

/// The voice badge of a player: bars while they speak, a hushed mark instead when the local
/// user muted their nearby voice, and dots on the local user while their microphone is open.
/// A scene that turns nearby voice off silences nearby speakers, so they show none; a call's
/// speakers still do.
fn voice_badge(
    speaking: Option<Speaking>,
    muted: bool,
    local_mic_open: bool,
    nearby_blocked: bool,
) -> Voice {
    match speaking.filter(|speaking| speaking.in_call || !nearby_blocked) {
        Some(Speaking { in_call: false }) if muted => Voice::Hushed,
        Some(_) => Voice::Speaking,
        None if local_mic_open => Voice::Idle,
        None => Voice::Off,
    }
}

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
            mic.scene_blocked(),
        );
        voice.set_if_neq(NametagVoice(badge));
    }
}

/// Where names are seen from.
struct TagView {
    position: Vec3,
    first_person: bool,
}

impl TagView {
    /// Whether the name of a player standing at `player` shows: in range, and not the local
    /// player's own seen through their eyes. A name out of view is left to frustum culling.
    fn shows(&self, player: Vec3, is_local: bool) -> bool {
        let distance = player.distance_squared(self.position);
        (MIN_DISTANCE * MIN_DISTANCE..=MAX_DISTANCE * MAX_DISTANCE).contains(&distance)
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

/// Decide which tags show, and which of those with their name. A name a scene's area hides
/// leaves the plate standing. One that doesn't show is only hidden: its tag, and its message,
/// stay.
fn show_tags(
    visible: Res<NametagsVisible>,
    camera: Query<(&Transform, &PrimaryCamera)>,
    players: Query<
        (
            &GlobalTransform,
            Has<PrimaryUser>,
            Has<HiddenNametag>,
            Has<BeyondMaxAvatars>,
            Has<BlockedOrBanned>,
            Option<&PlayerModifiers>,
            Option<&InheritedVisibility>,
            Has<SystemSceneNametag>,
            Has<NametagContent>,
            Has<NametagPlate>,
        ),
        With<AttachPoints>,
    >,
    mut tags: Query<(&ChildOf, &mut Nametag)>,
) {
    let Ok((camera_transform, camera)) = camera.single() else {
        return;
    };
    let view = TagView {
        position: camera_transform.translation,
        first_person: camera.view() == CameraView::FirstPerson,
    };

    for (owner, mut nametag) in &mut tags {
        let (shown, name_shown) = players
            .get(owner.parent())
            .ok()
            .filter(|_| visible.0)
            .filter(
                |(
                    transform,
                    is_local,
                    _,
                    beyond_max,
                    blocked,
                    modifiers,
                    drawn,
                    system_tag,
                    ..,
                )| {
                    !beyond_max
                        && !blocked
                        && !modifiers.is_some_and(|modifiers| modifiers.hide)
                        // a scene hides its avatars by their visibility, their parents' or a
                        // scale of nothing
                        && drawn.is_none_or(|drawn| drawn.get())
                        && avatar_units(transform).is_some()
                        // a system scene's avatar is drawn by its own camera, on its own layer:
                        // its tag shows for that camera, wherever the player's is
                        && (*system_tag || view.shows(transform.translation(), *is_local))
                },
            )
            .map_or((false, false), |(_, _, hidden, .., named, plated)| {
                let name_shown = named && !hidden;
                (name_shown || plated, name_shown)
            });
        if nametag.shown != shown {
            nametag.shown = shown;
        }
        if nametag.name_shown != name_shown {
            nametag.name_shown = name_shown;
        }
    }
}

/// The bridge's word that a message reads differently: a translation, or the original back.
fn retext_from_bridge(
    mut requests: EventReader<SystemApi>,
    mut retexts: EventWriter<ChatBubbleRetext>,
) {
    for request in requests.read() {
        if let SystemApi::SetChatBubbleText {
            message_id,
            message,
        } = request
        {
            retexts.write(ChatBubbleRetext {
                message_id: message_id.clone(),
                message: message.clone(),
            });
        }
    }
}

/// The HUD marks the same wallets official as the nametags.
fn answer_official_wallets(mut requests: EventReader<SystemApi>, wallets: Res<OfficialWallets>) {
    for request in requests.read() {
        if let SystemApi::GetOfficialWallets(sender) = request {
            sender.send(wallets.0.iter().map(|a| format!("{a:#x}")).collect());
        }
    }
}

fn show_chat_bubbles(
    mut commands: Commands,
    mut requests: EventReader<ChatBubbleRequest>,
    mut retexts: EventReader<ChatBubbleRetext>,
    time: Res<Time<Real>>,
    visible: Res<NametagsVisible>,
    config: Res<AppConfig>,
    local_player: Res<PrimaryPlayerRes>,
    local_profile: Res<CurrentUserProfile>,
    named: Query<(), With<NametagContent>>,
    others: Query<(&ForeignPlayer, &NametagContent)>,
    mut bubbles: Query<&mut ChatBubble>,
) {
    let now = time.elapsed_secs_f64();
    for request in requests.read() {
        if !visible.0 || !request.kind.allowed_by(config.chat_bubble_visibility) {
            continue;
        }
        if !named.contains(request.sender) {
            continue;
        }
        let message = bubble_text(&request.message, MAX_MESSAGE_CHARS);
        // nothing to read: no bubble
        if message.text.trim().is_empty() {
            continue;
        }
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
        match bubbles.get_mut(request.sender) {
            Ok(mut bubble) => {
                bubble.kind = kind;
                bubble.message = message;
                bubble.message_id = request.message_id.clone();
                bubble.arrived = now;
                bubble.mention = mention;
                bubble.recipient = recipient;
                bubble.stale = Stale::Message;
            }
            Err(_) => {
                commands.entity(request.sender).try_insert(ChatBubble {
                    kind,
                    message,
                    message_id: request.message_id.clone(),
                    arrived: now,
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
        let message = bubble_text(&retext.message, MAX_MESSAGE_CHARS);
        if message.text.trim().is_empty() {
            continue;
        }
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

/// What one frame's builds share.
struct Frame {
    /// Seconds of real time.
    now: f64,
    /// The glyph atlas' generation.
    generation: u32,
    budget: GlyphBudget,
}

/// Builds a tag's mesh from what its player shows.
#[derive(SystemParam)]
struct TagBuilder<'w, 's> {
    commands: Commands<'w, 's>,
    shaper: TextShaper<'w>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

impl TagBuilder<'_, '_> {
    fn frame(&mut self, now: f64) -> Frame {
        Frame {
            now,
            generation: self.shaper.atlas().generation(),
            budget: GlyphBudget::frame(),
        }
    }

    /// Bring `tag` up to what `player` shows now. Returns whether the tag draws: it should
    /// show, and its mesh samples the atlas as it is.
    fn build(
        &mut self,
        frame: &mut Frame,
        tag: Entity,
        player: Entity,
        nametag: &mut Nametag,
        content: Option<&NametagContent>,
        plate: Option<&NametagPlate>,
        voice: Voice,
        mut bubble: Option<Mut<ChatBubble>>,
        mesh: &mut Mut<Mesh3d>,
    ) -> bool {
        // a name that went, where one that is only hidden may come back: its glyphs are let go
        if content.is_none() && nametag.named.take().is_some() {
            nametag.name = None;
            self.keep_glyphs(tag, nametag, None);
        }
        let content = content.filter(|_| nametag.shown && nametag.name_shown);
        let Some(content) = content else {
            // a message whose time ran out unseen is not shown later
            if bubble
                .as_deref()
                .is_some_and(|bubble| bubble.over(frame.now))
            {
                self.commands.entity(player).remove::<ChatBubble>();
                self.keep_glyphs(tag, nametag, None);
            }
            if !nametag.shown {
                return false;
            }
            return self.finish(frame, tag, nametag, None, plate, voice, None, mesh);
        };

        // shaped once for what it says, however long its glyphs take to be rastered
        let content_changed = nametag.named.as_ref() != Some(content);
        if content_changed {
            nametag.name = self
                .shaper
                .shape(tag, &content.spans(), NAME_FONT_SIZE, None);
            nametag.named = Some(content.clone());
        }
        let mut header = Header {
            name: nametag
                .name
                .as_ref()
                .map_or(Vec2::new(0.0, NAME_FONT_SIZE * LINE_HEIGHT), |name| {
                    name.size
                }),
            verified: content.verified,
            official: content.official,
            voice,
            recipient: Vec2::ZERO,
        };

        let step = bubble
            .as_deref_mut()
            .map_or_else(BubbleStep::default, |bubble| {
                bubble.advance(
                    tag,
                    &mut header,
                    content_changed,
                    frame.now,
                    &mut self.shaper,
                )
            });
        if step.over {
            self.commands.entity(player).remove::<ChatBubble>();
        }
        let texts = bubble
            .as_deref()
            .filter(|_| !step.over)
            .and_then(|bubble| bubble.texts.as_ref());
        if content_changed || step.texts_changed {
            self.keep_glyphs(tag, nametag, texts);
        }
        self.finish(
            frame,
            tag,
            nametag,
            Some((content, header)),
            plate,
            voice,
            Some((&step, texts)),
            mesh,
        )
    }

    /// Rebuild the mesh of a tag that shows when what it was built from differs: its name, if
    /// that shows, with its bubble, and its plate. Returns whether the tag draws.
    fn finish(
        &mut self,
        frame: &mut Frame,
        tag: Entity,
        nametag: &mut Nametag,
        named: Option<(&NametagContent, Header)>,
        plate: Option<&NametagPlate>,
        voice: Voice,
        bubble: Option<(&BubbleStep, Option<&BubbleTexts>)>,
        mesh: &mut Mut<Mesh3d>,
    ) -> bool {
        let (step, texts) = bubble.unzip();
        let texts = texts.flatten();
        let progress = step.map(|step| step.progress).unwrap_or_default();
        let texts_changed = step.is_some_and(|step| step.texts_changed);

        if nametag.plated.as_ref() != plate {
            nametag.plate = plate.and_then(|plate| {
                self.shaper.shape_ellipsized(
                    tag,
                    &plate.label,
                    plate.label_color,
                    PLATE_FONT_SIZE,
                    PLATE_MAX_LABEL_WIDTH,
                )
            });
            nametag.plated = plate.cloned();
            self.keep_glyphs(tag, nametag, texts);
        }

        let content = named.as_ref().map(|(content, _)| *content);
        let scale = content.map_or(1.0, |content| content.scale);
        let dirty = nametag.awaiting
            || texts_changed
            || nametag.built.as_ref().is_none_or(|built| {
                built.content.as_ref() != content
                    || built.plate.as_ref() != plate
                    || built.voice != voice
                    || built.generation != frame.generation
                    || built.progress != progress
            });
        if dirty {
            let mut quads = Vec::new();
            let name = named.as_ref().map(|(_, header)| layout(header, &progress));
            let mut drawn = match &name {
                Some(name) => tag_quads(
                    name,
                    progress.mention,
                    voice,
                    nametag.name.as_ref(),
                    texts,
                    &mut self.shaper,
                    &mut frame.budget,
                    &mut quads,
                ),
                None => Ok(()),
            };
            if let (Ok(()), Some(plate)) = (&drawn, plate) {
                let label = nametag.plate.as_ref();
                drawn = plate_quads(
                    &layout::plate(
                        label.map_or(Vec2::ZERO, |label| label.size),
                        // clear of the mention ring, which is drawn whole as it fades in
                        name.as_ref().map(|name| {
                            name.root.inflate(if progress.mention > 0.0 {
                                MENTION_RING
                            } else {
                                0.0
                            })
                        }),
                    ),
                    plate,
                    label,
                    &mut self.shaper,
                    &mut frame.budget,
                    &mut quads,
                );
            }
            match drawn {
                Ok(()) => {
                    nametag.reach = self.write_mesh(mesh, &quads, scale);
                    nametag.awaiting = false;
                    nametag.built = Some(Built {
                        content: content.cloned(),
                        plate: plate.cloned(),
                        voice,
                        generation: frame.generation,
                        progress,
                    });
                }
                Err(Pending { .. }) => nametag.awaiting = true,
            }
        }

        // the last mesh shows until the glyphs are in, unless the atlas it samples has been
        // rebuilt: the shared material already binds the new one, where its UVs point at other
        // glyphs
        nametag
            .built
            .as_ref()
            .is_some_and(|built| built.generation == frame.generation)
    }

    /// The tag's glyphs, the bubble's included while it is up, are its to keep in the atlas.
    fn keep_glyphs(&mut self, tag: Entity, nametag: &Nametag, bubble: Option<&BubbleTexts>) {
        self.commands.entity(tag).insert(glyph_use(
            nametag
                .name
                .iter()
                .chain(&nametag.plate)
                .chain(bubble.into_iter().flat_map(BubbleTexts::iter)),
        ));
    }

    /// Replace the tag's mesh with `quads`, drawn `scale` times a player's size. Returns how far
    /// it reaches from the anchor.
    fn write_mesh(&mut self, mesh: &mut Mut<Mesh3d>, quads: &[Quad], scale: f32) -> f32 {
        let (tag_mesh, reach) = quad_mesh(quads, scale);
        if mesh.0 == Handle::default() {
            mesh.0 = self.meshes.add(tag_mesh);
        } else {
            // a tag keeps one mesh for as long as it lives. The renderer takes each version
            // out of the assets, so it is put back under the same id; and it reads a mesh's
            // place in its buffers only from entities that changed
            self.meshes.insert(mesh.0.id(), tag_mesh);
            mesh.set_changed();
        }
        reach
    }
}

fn build_tags(
    time: Res<Time<Real>>,
    mut tags: Query<(Entity, &ChildOf, &mut Nametag, &mut Mesh3d, &mut Visibility)>,
    mut owners: Query<(
        Option<&NametagContent>,
        Option<&NametagPlate>,
        &NametagVoice,
        Option<&mut ChatBubble>,
    )>,
    mut builder: TagBuilder,
) {
    let mut frame = builder.frame(time.elapsed_secs_f64());
    for (tag, owner, mut nametag, mut mesh, mut visibility) in &mut tags {
        let player = owner.parent();
        let Ok((content, plate, voice, bubble)) = owners.get_mut(player) else {
            continue;
        };
        if content.is_none() && plate.is_none() {
            // its name and its plate went in one go
            builder.commands.entity(tag).try_despawn();
            continue;
        }
        let draws = builder.build(
            &mut frame,
            tag,
            player,
            &mut nametag,
            content,
            plate,
            voice.0,
            bubble,
            &mut mesh,
        );
        visibility.set_if_neq(if draws {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// A display colour part of the way from `from` to `to`.
fn blend(from: Color, to: Color, amount: f32) -> Color {
    Color::Srgba(from.to_srgba().mix(&to.to_srgba(), amount.clamp(0.0, 1.0)))
}

/// The tag's quads, back to front.
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

/// A scene's plate: its border, its fill, and its label kept inside them.
fn plate_quads(
    layout: &PlateLayout,
    plate: &NametagPlate,
    label: Option<&Shaped>,
    shaper: &mut TextShaper,
    budget: &mut GlyphBudget,
    quads: &mut Vec<Quad>,
) -> Result<(), Pending> {
    let inside = layout.rect.inflate(-PLATE_BORDER);
    quads.push(Quad::rect(layout.rect, PLATE_RADIUS, plate.border));
    quads.push(Quad::rect(
        inside,
        PLATE_RADIUS - PLATE_BORDER,
        plate.background,
    ));
    if let Some(label) = label {
        shaper.quads(label, layout.label, Some(inside), budget, quads)?;
    }
    Ok(())
}

/// Stand each tag on its player's anchor. The player's transform carries it from there, and
/// the shader turns, sizes and fades it; its bounds cover it at its largest, when farthest.
/// The shader leaves out a scene's scale of its avatar, which the bounds, the avatar's child,
/// take: they are sized against it.
fn place_tags(
    camera: Query<&Projection, With<PrimaryCamera>>,
    owners: Query<(&AttachPoints, &GlobalTransform)>,
    anchors: Query<&Transform, Without<Nametag>>,
    mut tags: Query<(&ChildOf, &Nametag, &mut Transform, &mut Aabb)>,
) {
    let Ok(Projection::Perspective(perspective)) = camera.single() else {
        return;
    };
    let largest = tag_scale(MAX_DISTANCE, perspective.fov);

    for (owner, nametag, mut transform, mut aabb) in &mut tags {
        let Ok((attach_points, owner_transform)) = owners.get(owner.parent()) else {
            continue;
        };
        if let Ok(anchor) = anchors.get(attach_points.nametag) {
            if transform.translation != anchor.translation {
                transform.translation = anchor.translation;
            }
        }
        // an avatar scaled to nothing shows no tag
        let Some(smallest) = smallest_scale(owner_transform) else {
            continue;
        };
        // a cube: the tag faces the camera whichever way its player does
        let reach = Vec3A::splat(nametag.reach * largest / smallest);
        if aabb.half_extents != reach {
            *aabb = Aabb {
                center: Vec3A::ZERO,
                half_extents: reach,
            };
        }
    }
}

#[cfg(test)]
mod tests;
