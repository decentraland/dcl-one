use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use bevy::{input::common_conditions::input_just_pressed, prelude::*};

use crate::{IPFS_CACHED, IPFS_FAILED, IPFS_IN_FLIGHT, IPFS_NON_IPFS, IPFS_SUCCESS};

const FILE_GRID_ROWS: usize = 1024;
const CELL_PER_ROW: usize = 4;
const CELL_FONT_SIZE: f32 = 8.;
const SCROLL_FACTOR: f32 = 128.;

pub struct IpfsDebugPlugin;

impl Plugin for IpfsDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FileLog>();
        app.add_systems(Startup, setup);
        app.add_systems(
            Update,
            (
                // Runs before receive_debug so that it sees this frame's show or hide;
                // otherwise it could spawn rows into a grid that was just emptied.
                toggle_display
                    .run_if(input_just_pressed(KeyCode::F1))
                    .before(receive_debug),
                update_text_from_atomics,
                (
                    receive_debug,
                    scroll_bottom.run_if(in_state(IpfsFileGridState::Tail)),
                )
                    .chain()
                    .run_if(resource_exists::<IpfsDebugReceiver>),
            ),
        );
    }

    fn finish(&self, app: &mut App) {
        app.init_state::<IpfsFileGridState>();

        if !app.world().contains_resource::<IpfsDebugReceiver>() {
            warn!("IpfsDebugReceiver was not initialized. Debug overlay will not display per file info.");
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
enum IpfsFileGridState {
    Free,
    #[default]
    Tail,
}

#[derive(Resource, Deref, DerefMut)]
pub struct IpfsDebugReceiver(pub tokio::sync::mpsc::UnboundedReceiver<IpfsDebug>);

#[derive(Debug)]
pub struct IpfsDebug {
    pub path: PathBuf,
    pub status: IpfsDebugStatus,
    pub duration: Duration,
    pub length: usize,
}

#[derive(Debug)]
pub enum IpfsDebugStatus {
    Success,
    Failure,
    Cached,
    NonIpfs,
}

#[derive(Component)]
struct UiRoot;

#[derive(Component)]
struct FileGrid;

#[derive(Component, Deref)]
struct AtomicSource(&'static AtomicU32);

/// The last `FILE_GRID_ROWS` files, one string per grid column. The grid holds cells for them
/// only while the overlay shows: hidden cells would still cost UI layout every frame.
#[derive(Resource, Default)]
struct FileLog(VecDeque<[String; CELL_PER_ROW]>);

fn setup(mut commands: Commands) {
    commands.spawn((
        UiRoot,
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            left: Val::Px(32.),
            top: Val::Px(32.),
            min_width: Val::Px(32.),
            min_height: Val::Px(32.),
            row_gap: Val::Px(4.),
            margin: UiRect::all(Val::Px(4.)),
            flex_direction: FlexDirection::Column,
            ..Default::default()
        },
        BackgroundColor(Color::BLACK.with_alpha(0.5)),
        GlobalZIndex(1_000_000_000),
        children![
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.),
                    ..Default::default()
                },
                children![
                    (Text::new("In Flight"),),
                    (Text::new("0"), AtomicSource(&IPFS_IN_FLIGHT)),
                    column_divider(),
                    (Text::new("Success"),),
                    (Text::new("0"), AtomicSource(&IPFS_SUCCESS)),
                    column_divider(),
                    (Text::new("Cached"),),
                    (Text::new("0"), AtomicSource(&IPFS_CACHED)),
                    column_divider(),
                    (Text::new("Non-ipfs"),),
                    (Text::new("0"), AtomicSource(&IPFS_NON_IPFS)),
                    column_divider(),
                    (Text::new("Failed"),),
                    (Text::new("0"), AtomicSource(&IPFS_FAILED)),
                ]
            ),
            (
                FileGrid,
                Node {
                    display: Display::Grid,
                    grid_template_columns: RepeatedGridTrack::auto(CELL_PER_ROW as u16),
                    max_width: Val::Vw(75.),
                    max_height: Val::Vh(75.),
                    row_gap: Val::Px(4.),
                    column_gap: Val::Px(4.),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    ..Default::default()
                },
                Observer::new(scroll_file_grid)
            ),
            (
                Node {
                    margin: UiRect::all(Val::Px(4.)),
                    column_gap: Val::Px(4.),
                    flex_direction: FlexDirection::Row,
                    ..Default::default()
                },
                children![
                    (
                        Text::new("Clear"),
                        BackgroundColor(Color::BLACK.lighter(0.25)),
                        Observer::new(clear_file_grid)
                    ),
                    (
                        Text::new("Bottom"),
                        BackgroundColor(Color::BLACK.lighter(0.25)),
                        Observer::new(go_to_bottom)
                    )
                ]
            )
        ],
    ));
}

fn toggle_display(
    mut commands: Commands,
    ui_root: Single<&mut Node, With<UiRoot>>,
    file_grid: Single<Entity, With<FileGrid>>,
    log: Res<FileLog>,
) {
    debug!("Toggling Ipfs Debug overlay");
    let mut node = ui_root.into_inner();
    if node.display == Display::None {
        node.display = Display::Flex;
        for row in &log.0 {
            spawn_row(&mut commands, *file_grid, row);
        }
    } else {
        node.display = Display::None;
        commands.entity(*file_grid).despawn_related::<Children>();
    }
}

fn update_text_from_atomics(atomic_sources: Query<(&mut Text, &AtomicSource)>) {
    for (mut text, atomic_source) in atomic_sources {
        let value = atomic_source.load(Ordering::Relaxed).to_string();
        if text.0 != value {
            text.0 = value;
        }
    }
}

fn receive_debug(
    mut commands: Commands,
    ui_root: Single<&Node, With<UiRoot>>,
    file_grid: Single<(Entity, Option<&Children>), With<FileGrid>>,
    mut ipfs_debug_receiver: ResMut<IpfsDebugReceiver>,
    mut log: ResMut<FileLog>,
) {
    let shown = ui_root.display != Display::None;
    let (file_grid, children) = file_grid.into_inner();
    let mut cells = children.map_or(0, |children| children.len());

    loop {
        let debug = match ipfs_debug_receiver.try_recv() {
            Ok(debug) => debug,
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                commands.remove_resource::<IpfsDebugReceiver>();
                break;
            }
        };

        let row = [
            debug.path.display().to_string(),
            format!("{:?}", debug.status),
            format!("{:?}", debug.duration),
            format!("{}b", debug.length),
        ];
        if shown {
            spawn_row(&mut commands, file_grid, &row);
            cells += CELL_PER_ROW;
        }
        log.0.push_back(row);
        if log.0.len() > FILE_GRID_ROWS {
            log.0.pop_front();
        }
    }

    // cells are placed in spawn order, so trimming the oldest rows needs no renumbering
    let Some(children) = children else {
        return;
    };
    let excess = cells.saturating_sub(FILE_GRID_ROWS * CELL_PER_ROW);
    if excess > 0 {
        trace!("Trimming {} excess cells", excess);
        for child in children.iter().take(excess) {
            commands.entity(child).despawn();
        }
    }
}

fn spawn_row(commands: &mut Commands, file_grid: Entity, row: &[String; CELL_PER_ROW]) {
    for cell in row {
        commands.spawn((
            ChildOf(file_grid),
            Node::default(),
            Text::new(cell.clone()),
            TextFont {
                font_size: CELL_FONT_SIZE,
                ..Default::default()
            },
        ));
    }
}

fn scroll_bottom(
    ui_root: Single<&Node, With<UiRoot>>,
    file_grid: Single<&mut ScrollPosition, With<FileGrid>>,
) {
    if ui_root.display == Display::None {
        return;
    }
    // layout clamps the offset to the content, so this pins the grid to its last row
    file_grid.into_inner().offset_y = f32::MAX;
}

fn scroll_file_grid(
    mut trigger: Trigger<Pointer<Scroll>>,
    mut commands: Commands,
    mut file_grid: Single<&mut ScrollPosition, With<FileGrid>>,
) {
    if trigger.target() != trigger.observer() {
        return;
    }
    trigger.propagate(false);

    let scroll = trigger.event();

    commands.set_state(IpfsFileGridState::Free);
    file_grid.offset_y -= scroll.y * SCROLL_FACTOR;
}

fn clear_file_grid(
    mut trigger: Trigger<Pointer<Pressed>>,
    mut commands: Commands,
    file_grid: Single<Entity, With<FileGrid>>,
    mut log: ResMut<FileLog>,
) {
    if trigger.target() != trigger.observer() {
        return;
    }
    trigger.propagate(false);

    commands.entity(*file_grid).despawn_related::<Children>();
    log.0.clear();
}

fn go_to_bottom(mut trigger: Trigger<Pointer<Pressed>>, mut commands: Commands) {
    if trigger.target() != trigger.observer() {
        return;
    }
    trigger.propagate(false);

    commands.set_state(IpfsFileGridState::Tail);
}

fn column_divider() -> impl Bundle {
    (
        Node {
            width: Val::Px(2.),
            height: Val::Percent(100.),
            ..Default::default()
        },
        BackgroundColor(Color::WHITE),
    )
}
