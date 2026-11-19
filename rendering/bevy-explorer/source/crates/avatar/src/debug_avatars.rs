use alloy_core::primitives::Address;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use collectibles::base_wearables;
use common::structs::{
    AppConfig, AttachPoints, AvatarDynamicState, EmoteCommand, EmoteMask, PrimaryUser,
};
use comms::profile::{name_color::name_color, ProfileManager, UserProfile};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::sdk::components::PbAvatarShape;
use rand::{rngs::StdRng, seq::IteratorRandom, Rng, SeedableRng};
use serde_json::json;

use crate::{
    nametag::NametagContent, AvatarProcessed, AvatarSelection, AvatarShape, SessionMaxAvatars,
};

/// Enough to find where frame time breaks down, well past the Max Avatars setting's range.
const MAX_DEMOS: usize = 1024;
/// The highest session cap `/avatar_demo cap` takes: every demo avatar and the player.
const MAX_CAP: usize = MAX_DEMOS + 1;
/// Demo avatars stand in a square grid in front of the player, this many to a row. A row fills
/// from its centre outwards, so the first avatar stands straight ahead.
const GRID_COLUMNS: u64 = 32;
const GRID_SPACING: f32 = 1.6;
/// The turn between one demo avatar and the next in the `around` layout: the golden angle, which
/// fills a disc evenly from its centre outwards.
const AROUND_TURN: f32 = 2.399_963;

/// Where the demo avatar in `slot` stands, relative to the player.
fn demo_offset(layout: DemoLayout, slot: u64) -> Vec3 {
    match layout {
        DemoLayout::Grid => {
            // columns 0, 1, -1, 2, -2, ... from straight ahead
            let column = slot % GRID_COLUMNS;
            let side = if column % 2 == 1 { 1.0 } else { -1.0 };
            Vec3::new(
                side * column.div_ceil(2) as f32 * GRID_SPACING,
                0.0,
                -((slot / GRID_COLUMNS) as f32) * GRID_SPACING - 2.0,
            )
        }
        // one avatar per GRID_SPACING squared, the nearest first
        DemoLayout::Around => {
            let radius = GRID_SPACING * ((slot + 1) as f32 / std::f32::consts::PI).sqrt();
            let (sin, cos) = (slot as f32 * AROUND_TURN).sin_cos();
            Vec3::new(radius * cos, 0.0, radius * sin)
        }
    }
}

/// How demo avatars are placed.
#[derive(clap::ValueEnum, Clone, Copy, Default, Debug, PartialEq, Eq)]
enum DemoLayout {
    /// a grid in front of the player, each row filled from its centre outwards
    #[default]
    Grid,
    /// a disc centred on the player, filled from the centre: a crowd on every side
    Around,
}

impl DemoLayout {
    fn name(self) -> &'static str {
        match self {
            DemoLayout::Grid => "grid",
            DemoLayout::Around => "around",
        }
    }
}
const BASE_AVATARS: &str = "urn:decentraland:off-chain:base-avatars:";
const CREATOR_EMOTES: &[(&str, &str)] = &[
    ("None", "none"),
    ("Clap", "clap"),
    ("Cry", "cry"),
    ("Dance", "dance"),
    ("Disco", "disco"),
    ("Hammer", "hammer"),
    ("Kiss", "kiss"),
    ("Money", "money"),
    ("Robot", "robot"),
    ("Tektonik", "tektonik"),
    ("Tik", "tik"),
];

pub(super) struct DebugAvatarsPlugin;

impl Plugin for DebugAvatarsPlugin {
    fn build(&self, app: &mut App) {
        if !app
            .world()
            .get_resource::<common::structs::PreviewMode>()
            .is_some_and(|p| p.is_preview)
        {
            return;
        }
        app.init_resource::<Creator>()
            .init_resource::<SessionMaxAvatars>()
            .add_preview_console_command::<DemoCommand, _>(demo_command)
            .add_preview_console_command::<LookCommand, _>(look_command)
            .add_observer(clear_on_logout)
            .add_systems(
                Update,
                (
                    forget_displayed_profile.after(crate::update_base_avatar_shape),
                    fetch_look
                        .after(look_command)
                        .run_if(any_with_component::<PendingProfile>),
                )
                    .chain(),
            )
            .add_debug_section(DebugSection {
                title: "Avatar debug",
                query: "/avatar_demo",
                fields: &[
                    ("Spawned demo avatars", "count"),
                    ("Built demo avatars", "built"),
                    ("Max avatars this session", "cap"),
                ],
                actions: &[
                    ("Use the Max Avatars setting", "/avatar_demo cap"),
                    ("Destroy demo avatars", "/avatar_demo clear"),
                    ("Destroy random number", "/avatar_demo remove-random"),
                    ("Randomize demo wearables", "/avatar_demo randomize"),
                ],
                inputs: &[
                    DebugInput {
                        label: "Force male for random avatars",
                        field: "maleOnly",
                        command: "/avatar_demo male-only {value}",
                        kind: DebugInputKind::Toggle,
                    },
                    DebugInput {
                        label: "Max avatars this session (not saved)",
                        field: "cap",
                        command: "/avatar_demo cap {value}",
                        kind: DebugInputKind::Number {
                            min: 1.0,
                            max: MAX_CAP as f64,
                            step: 1.0,
                        },
                    },
                    DebugInput {
                        label: "Spawn random avatars",
                        field: "",
                        command: "/avatar_demo spawn {value}",
                        kind: DebugInputKind::Number {
                            min: 1.0,
                            max: MAX_DEMOS as f64,
                            step: 1.0,
                        },
                    },
                ],
            })
            .add_debug_section(DebugSection {
                title: "Avatar creator",
                query: "/avatar_demo",
                fields: &[],
                actions: &[
                    ("Instantiate male", "/avatar_demo male"),
                    ("Instantiate female", "/avatar_demo female"),
                    ("Clear wearables", "/avatar_demo wearables"),
                ],
                inputs: &[
                    DebugInput {
                        label: "Wearable URNs (up to three, comma-separated)",
                        field: "wearables",
                        command: "/avatar_demo wearables {value}",
                        kind: DebugInputKind::Text,
                    },
                    DebugInput {
                        label: "Emote for new avatars",
                        field: "emote",
                        command: "/avatar_demo emote {value}",
                        kind: DebugInputKind::Select(CREATOR_EMOTES),
                    },
                ],
            })
            .add_debug_section(DebugSection {
                title: "Profile avatar shape",
                query: "/avatar_look",
                fields: &[
                    ("Displayed profile", "displayed"),
                    ("Requested profile", "requested"),
                    ("Status", "status"),
                ],
                actions: &[("Restore own appearance", "/avatar_look reset")],
                inputs: &[DebugInput {
                    label: "Preview profile address",
                    field: "",
                    command: "/avatar_look {value}",
                    kind: DebugInputKind::Text,
                }],
            });
    }
}

#[derive(Component)]
struct DemoAvatar;

fn clear_on_logout(
    trigger: Trigger<OnRemove, UserProfile>,
    mut commands: Commands,
    primary: Query<(), With<PrimaryUser>>,
    demos: Query<Entity, With<DemoAvatar>>,
) {
    if primary.contains(trigger.target()) {
        if let Ok(mut player) = commands.get_entity(trigger.target()) {
            player.try_remove::<(PendingProfile, DisplayedProfile, MissingProfile)>();
        }
        for demo in &demos {
            commands.entity(demo).try_despawn();
        }
    }
}

#[derive(Default, Resource)]
struct Creator {
    male_only: bool,
    wearables: Vec<String>,
    emote: Option<String>,
    layout: DemoLayout,
    /// With a seed, the avatar at each index always gets the same look.
    seed: Option<u64>,
    next: u64,
}

/// The seed of the look at `index` in the crowd `seed` names. The two are mixed, so one seed's
/// crowd is not the next seed's moved along by one.
fn look_seed(seed: u64, index: u64) -> u64 {
    seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// A random demo avatar's look: whether it is male, and eight base wearables.
fn random_look(rng: &mut impl Rng, male_only: bool) -> (bool, Vec<String>) {
    let male = male_only || rng.gen_bool(0.5);
    let wearables = base_wearables::base_wearable_urns()
        .iter()
        .skip(2)
        .choose_multiple(rng, 8)
        .into_iter()
        .map(ToString::to_string)
        .collect();
    (male, wearables)
}

/// Create local rendered avatars, configure their look or remove only the test population.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/avatar_demo")]
struct DemoCommand {
    #[command(subcommand)]
    action: Option<DemoAction>,
}

#[derive(clap::Subcommand)]
enum DemoAction {
    Spawn {
        count: usize,
    },
    MaleOnly {
        #[arg(action = clap::ArgAction::Set)]
        enabled: bool,
    },
    Wearables {
        #[arg(value_delimiter = ',')]
        urns: Vec<String>,
    },
    Emote {
        name: String,
    },
    /// Set Max Avatars for this session only, past the setting's range, to measure crowds; no
    /// value goes back to the setting.
    Cap {
        max: Option<usize>,
    },
    /// Where new demo avatars stand: `grid` in front of the player, or `around` them.
    Layout {
        layout: DemoLayout,
    },
    /// Give the avatar at each index the same random look in every session; no seed clears it.
    Seed {
        seed: Option<u64>,
    },
    Male,
    Female,
    Clear,
    RemoveRandom,
    Randomize,
}

fn demo_command(
    mut cmd: ConsoleCommand<DemoCommand>,
    mut commands: Commands,
    mut creator: ResMut<Creator>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut demos: Query<(Entity, &mut AvatarSelection), With<DemoAvatar>>,
    config: Res<AppConfig>,
    mut session_max: ResMut<SessionMaxAvatars>,
    (children, processed, drawn_meshes): (
        Query<&Children>,
        Query<&InheritedVisibility, With<AvatarProcessed>>,
        Query<&ViewVisibility, With<Mesh3d>>,
    ),
) {
    let Some(Ok(args)) = cmd.take() else { return };
    let mut count = demos.iter().len();
    let mut rng = rand::thread_rng();
    match args.action {
        Some(DemoAction::MaleOnly { enabled }) => creator.male_only = enabled,
        Some(DemoAction::Wearables { urns }) => {
            let wearables: Vec<_> = urns.into_iter().filter(|s| !s.is_empty()).collect();
            if wearables.len() > 3 {
                cmd.reply_failed("supply up to three wearable URNs");
                return;
            }
            creator.wearables = wearables;
        }
        Some(DemoAction::Cap { max }) => {
            if max.is_some_and(|max| !(1..=MAX_CAP).contains(&max)) {
                cmd.reply_failed(format!("choose a cap from 1 to {MAX_CAP}"));
                return;
            }
            // beside the config, not in it: a change to the config is written to the stored one
            session_max.0 = max;
        }
        Some(DemoAction::Layout { layout }) => creator.layout = layout,
        Some(DemoAction::Seed { seed }) => creator.seed = seed,
        Some(DemoAction::Emote { name }) => {
            if !CREATOR_EMOTES.iter().any(|(_, value)| *value == name) {
                cmd.reply_failed("unknown creator emote");
                return;
            }
            creator.emote = (name != "none").then_some(name);
        }
        Some(DemoAction::Clear | DemoAction::RemoveRandom) => {
            let n = if matches!(args.action, Some(DemoAction::Clear)) {
                count
            } else {
                rng.gen_range(0..=count)
            };
            for entity in demos.iter().map(|(e, _)| e).choose_multiple(&mut rng, n) {
                commands.entity(entity).despawn();
            }
            count -= n;
            if count == 0 {
                // a population spawned after a clear stands where the first one stood
                creator.next = 0;
            }
        }
        Some(DemoAction::Randomize) => {
            let catalog = base_wearables::base_wearable_urns();
            for (_, mut selection) in &mut demos {
                selection.shape.0.wearables = catalog
                    .iter()
                    .skip(2)
                    .choose_multiple(&mut rng, 8)
                    .into_iter()
                    .map(ToString::to_string)
                    .collect();
            }
        }
        Some(action @ (DemoAction::Spawn { .. } | DemoAction::Male | DemoAction::Female)) => {
            let n = match action {
                DemoAction::Spawn { count } => count,
                _ => 1,
            };
            if n == 0 || n > MAX_DEMOS.saturating_sub(count) {
                cmd.reply_failed(format!("choose a positive count that keeps the demo population at or below {MAX_DEMOS}"));
                return;
            }
            let Ok(player) = player.single() else {
                cmd.reply_failed("player unavailable");
                return;
            };
            for _ in 0..n {
                let index = creator.next;
                creator.next += 1;
                let (male, wearables) = if matches!(action, DemoAction::Spawn { .. }) {
                    match creator.seed {
                        Some(seed) => random_look(
                            &mut StdRng::seed_from_u64(look_seed(seed, index)),
                            creator.male_only,
                        ),
                        None => random_look(&mut rng, creator.male_only),
                    }
                } else {
                    (
                        matches!(action, DemoAction::Male),
                        creator.wearables.clone(),
                    )
                };
                let slot = index % MAX_DEMOS as u64;
                let offset = demo_offset(creator.layout, slot);
                let shape = AvatarShape(PbAvatarShape {
                    id: format!("debug-avatar-{index}"),
                    body_shape: Some(format!(
                        "{BASE_AVATARS}{}",
                        if male { "BaseMale" } else { "BaseFemale" }
                    )),
                    wearables,
                    ..default()
                });
                // a demo avatar stands in for a player: it carries a nametag too, every other
                // one with a claimed name
                let attach_points = AttachPoints::new(&mut commands);
                let claimed = index & 1 == 0;
                let name = format!("Demo{index}");
                let address = format!("{index:040x}");
                let nametag = NametagContent {
                    suffix: if claimed {
                        String::new()
                    } else {
                        format!("#{index:04x}")
                    },
                    color: name_color(&name, &address, claimed),
                    verified: claimed,
                    official: false,
                    name,
                };
                let mut entity = commands.spawn((
                    DemoAvatar,
                    Transform::from_translation(player.translation() + offset),
                    Visibility::Inherited,
                    AvatarDynamicState::default(),
                    AvatarSelection {
                        scene: None,
                        shape,
                        automatic_delete: false,
                        disable_dither: false,
                    },
                    nametag,
                ));
                entity
                    .add_children(&attach_points.entities())
                    .insert(attach_points);
                if let Some(name) = &creator.emote {
                    entity.insert(EmoteCommand {
                        urn: format!("urn:decentraland:off-chain:base-emotes:{name}"),
                        timestamp: 1,
                        r#loop: true,
                        mask: EmoteMask::FullBody,
                    });
                }
            }
            count += n;
        }
        None => {}
    }
    // demo avatars with a finished build: a measurement waits for these to reach the count.
    // Of those, `shown` are not hidden (by the cap, say) and `drawn` have a mesh in some view
    // this frame, the camera's or a shadow's
    let (mut built, mut shown, mut drawn) = (0, 0, 0);
    for (entity, _) in &demos {
        let Some(build) = children
            .get(entity)
            .ok()
            .and_then(|children| children.iter().find(|child| processed.contains(*child)))
        else {
            continue;
        };
        built += 1;
        if processed.get(build).is_ok_and(|shown| shown.get()) {
            shown += 1;
            if children
                .iter_descendants(build)
                .any(|mesh| drawn_meshes.get(mesh).is_ok_and(|drawn| drawn.get()))
            {
                drawn += 1;
            }
        }
    }
    cmd.reply_ok(json!({"count":count, "built":built, "shown":shown, "drawn":drawn, "cap":SessionMaxAvatars::effective(Some(&*session_max), &config), "sessionCap":session_max.0, "layout":creator.layout.name(), "seed":creator.seed, "maleOnly":creator.male_only, "wearables":creator.wearables.join(","), "emote":creator.emote.as_deref().unwrap_or("none")}).to_string());
}

/// Temporarily render a fetched profile's appearance on the local player.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/avatar_look")]
struct LookCommand {
    address: Option<String>,
}

#[derive(Component)]
struct PendingProfile(Address);
#[derive(Component)]
struct DisplayedProfile(Address);

fn forget_displayed_profile(
    mut commands: Commands,
    changed: Query<Entity, (Changed<UserProfile>, With<DisplayedProfile>)>,
) {
    for entity in &changed {
        commands.entity(entity).try_remove::<DisplayedProfile>();
    }
}
#[derive(Component)]
struct MissingProfile(Address);

fn look_command(
    mut cmd: ConsoleCommand<LookCommand>,
    mut commands: Commands,
    player: Query<
        (
            Entity,
            &UserProfile,
            Option<&PendingProfile>,
            Option<&DisplayedProfile>,
            Option<&MissingProfile>,
        ),
        With<PrimaryUser>,
    >,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    let Ok((entity, own, pending, displayed, missing)) = player.single() else {
        cmd.reply_failed("local profile unavailable");
        return;
    };
    match args.address.as_deref() {
        Some("reset") => {
            let mut shape = AvatarShape::from(own);
            shape.0.name = None;
            commands
                .entity(entity)
                .insert(shape)
                .remove::<(PendingProfile, DisplayedProfile, MissingProfile)>();
            cmd.reply_ok("restored own appearance");
        }
        Some(raw) => match raw.parse::<Address>() {
            Ok(address) => {
                commands
                    .entity(entity)
                    .insert(PendingProfile(address))
                    .remove::<MissingProfile>();
                cmd.reply_ok(format!("loading appearance for {address:#x}"));
            }
            Err(_) => cmd.reply_failed("expected a wallet address or reset"),
        },
        None => {
            let status = if pending.is_some() {
                "loading"
            } else if missing.is_some() {
                "profile unavailable"
            } else if displayed.is_some() {
                "preview"
            } else {
                "own profile"
            };
            cmd.reply_ok(json!({"status":status, "requested":pending.map(|p| p.0).or(missing.map(|p| p.0)).map(|a| format!("{a:#x}")), "displayed":displayed.map(|p| format!("{:#x}",p.0)).unwrap_or_else(|| own.content.eth_address.clone()),
                "unavailable":{"requested":"No pending profile request."}}).to_string());
        }
    }
}

fn fetch_look(
    mut commands: Commands,
    mut profiles: ProfileManager,
    players: Query<(Entity, &UserProfile, &PendingProfile), With<PrimaryUser>>,
) {
    for (entity, own, pending) in &players {
        match profiles.get_data(pending.0) {
            Ok(Some(profile)) => {
                let mut shape = AvatarShape::from(profile);
                shape.0.id.clone_from(&own.content.eth_address);
                shape.0.name = None;
                commands
                    .entity(entity)
                    .remove::<PendingProfile>()
                    .insert((DisplayedProfile(pending.0), shape));
            }
            Ok(None) => {}
            Err(_) => {
                commands
                    .entity(entity)
                    .remove::<PendingProfile>()
                    .insert(MissingProfile(pending.0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_gives_each_index_the_same_look() {
        let look = |seed: u64| random_look(&mut StdRng::seed_from_u64(seed), false);
        assert_eq!(look(7), look(7));
        assert_eq!(look(7).1.len(), 8);
        // and different indexes differ
        assert!((0..20).any(|index| look(index) != look(7)));
    }

    #[test]
    fn neighbouring_seeds_are_different_crowds() {
        assert_eq!(look_seed(1, 0), 1);
        assert_ne!(look_seed(1, 1), look_seed(2, 0));
        assert_ne!(look_seed(1, 2), look_seed(2, 1));
    }

    #[test]
    fn the_grid_stands_in_front_of_the_player() {
        for slot in 0..MAX_DEMOS as u64 {
            assert!(demo_offset(DemoLayout::Grid, slot).z <= -2.0);
        }
    }

    #[test]
    fn a_grid_row_fills_from_straight_ahead_outwards() {
        let row: Vec<_> = (0..GRID_COLUMNS)
            .map(|slot| demo_offset(DemoLayout::Grid, slot).x)
            .collect();
        assert!(row[0].abs() < 0.01);
        assert!(row[1] > 0.0 && row[2] < 0.0);
        // no avatar stands closer to the centre than the one before it
        assert!(row.windows(2).all(|pair| pair[0].abs() <= pair[1].abs()));
        // and no two share a place
        for (i, a) in row.iter().enumerate() {
            assert!(row[i + 1..]
                .iter()
                .all(|b| (a - b).abs() > GRID_SPACING / 2.0));
        }
    }

    #[test]
    fn the_around_layout_surrounds_the_player_nearest_first() {
        let offsets: Vec<_> = (0..200)
            .map(|slot| demo_offset(DemoLayout::Around, slot))
            .collect();
        // each avatar stands further out than the one before
        assert!(offsets
            .windows(2)
            .all(|pair| pair[0].length() < pair[1].length()));
        // and they are on every side of the player
        for side in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
            let on_side = offsets.iter().filter(|o| o.dot(side) > 0.0).count();
            assert!((80..=120).contains(&on_side), "{on_side} towards {side}");
        }
    }
}
