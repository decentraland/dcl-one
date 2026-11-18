//! Scene map pins (`MapPin`) for the HUD map. Only portable experiences whose scene.json declares
//! `isPortableExperience: true` are read; the HUD follows them through `GetMapPinsStream`.

use std::collections::HashSet;

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::{rpc::RpcStreamSender, sets::SceneSets, structs::ReadsMapPins, util::TaskExt};
use dcl_component::{
    proto_components::{common::texture_union::Tex, sdk::components::PbMapPin},
    ComponentPosition, SceneComponentId,
};
use ipfs::{
    ipfs_path::{content_file_path, IpfsPath, IpfsType},
    IpfsResource,
};
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::AddCrdtInterfaceExt, SceneEntity,
};
use system_bridge::{MapPinInfo, SystemApi};

mod diagnostics;

/// The title is the pin's accessible name.
const MAX_TITLE_CHARS: usize = 100;

pub(crate) struct MapPinsPlugin;

impl Plugin for MapPinsPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbMapPin, MapPin>(
            SceneComponentId::MAP_PIN,
            ComponentPosition::EntityOnly,
        );
        app.init_resource::<MapPinStreams>();
        app.add_observer(clear_map_pin_icon);
        app.add_plugins(diagnostics::MapPinsDebugPanelPlugin);
        app.add_systems(
            Update,
            (resolve_map_pin_icons, stream_map_pins)
                .chain()
                .in_set(SceneSets::RestrictedActions),
        );
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
struct MapPin {
    /// `None` when the position is missing or not finite: the pin is not shown.
    parcel: Option<IVec2>,
    title: String,
    icon_src: Option<String>,
}

impl From<PbMapPin> for MapPin {
    fn from(pin: PbMapPin) -> Self {
        let parcel = pin
            .position
            .filter(|p| p.x.is_finite() && p.y.is_finite())
            // halves to even, so every client puts a pin on the same parcel
            .map(|p| IVec2::new(p.x.round_ties_even() as i32, p.y.round_ties_even() as i32));
        let icon_src = match pin.texture.and_then(|t| t.tex) {
            Some(Tex::Texture(texture)) if !texture.src.is_empty() => Some(texture.src),
            _ => None,
        };
        let mut title = pin.title;
        if let Some((cut, _)) = title.char_indices().nth(MAX_TITLE_CHARS) {
            title.truncate(cut);
        }
        Self {
            parcel,
            title,
            icon_src,
        }
    }
}

/// Stays in place while a changed src resolves, so the pin never leaves the map meanwhile.
#[derive(Component, Debug, Clone, PartialEq)]
struct MapPinIcon {
    src: Option<String>,
    url: Option<String>,
}

#[derive(Component)]
struct ResolvingMapPinIcon {
    src: String,
    task: Task<Option<String>>,
}

#[derive(Resource, Default)]
struct MapPinStreams {
    senders: Vec<RpcStreamSender<Vec<MapPinInfo>>>,
    last: Option<Vec<MapPinInfo>>,
}

/// The loader fetches a content-map miss that parses as a url (after dropping any leading `/`),
/// so such a src is refused: a scene must not make the HUD request a host of its choosing.
async fn icon_url(ipfs: IpfsResource, scene_hash: String, src: String) -> Option<String> {
    if super::filename_looks_like_url(&content_file_path(&src)) {
        return None;
    }
    let context = ipfs.context.read().await;
    IpfsPath::new(IpfsType::new_content_file(scene_hash, src))
        .to_url(&context)
        .ok()
}

fn clear_map_pin_icon(trigger: Trigger<OnRemove, MapPin>, mut commands: Commands) {
    if let Ok(mut entity) = commands.get_entity(trigger.target()) {
        entity.try_remove::<(MapPinIcon, ResolvingMapPinIcon)>();
    }
}

fn resolve_map_pin_icons(
    mut commands: Commands,
    changed: Query<(Entity, &MapPin, &SceneEntity, Option<&MapPinIcon>), Changed<MapPin>>,
    mut resolving: Query<(Entity, &mut ResolvingMapPinIcon)>,
    roots: Query<&RendererSceneContext, With<ReadsMapPins>>,
    ipfs: Res<IpfsResource>,
) {
    // a task replaced or dropped this frame may still complete now, with a stale result
    let mut superseded = HashSet::new();
    for (entity, pin, scene_entity, icon) in changed.iter() {
        let Ok(context) = roots.get(scene_entity.root) else {
            continue;
        };
        let in_flight = resolving.get(entity).ok().map(|(_, r)| r.src.as_str());
        if in_flight.is_some() && in_flight == pin.icon_src.as_deref() {
            continue;
        }
        let mut commands = commands.entity(entity);
        if in_flight.is_some() {
            superseded.insert(entity);
        }
        if icon.is_some_and(|icon| icon.src == pin.icon_src) {
            commands.remove::<ResolvingMapPinIcon>();
            continue;
        }
        match pin.icon_src.clone() {
            Some(src) => {
                let task = IoTaskPool::get().spawn(icon_url(
                    ipfs.clone(),
                    context.hash.to_string(),
                    src.clone(),
                ));
                commands.insert(ResolvingMapPinIcon { src, task });
            }
            None => {
                commands.remove::<ResolvingMapPinIcon>().insert(MapPinIcon {
                    src: None,
                    url: None,
                });
            }
        }
    }
    for (entity, mut resolving) in resolving.iter_mut() {
        if superseded.contains(&entity) {
            continue;
        }
        if let Some(url) = resolving.task.complete() {
            commands
                .entity(entity)
                .remove::<ResolvingMapPinIcon>()
                .insert(MapPinIcon {
                    src: Some(resolving.src.clone()),
                    url,
                });
        }
    }
}

#[allow(clippy::type_complexity)]
fn stream_map_pins(
    mut streams: ResMut<MapPinStreams>,
    mut events: EventReader<SystemApi>,
    pins: Query<(&MapPinIcon, &MapPin, &SceneEntity)>,
    changed: Query<(), (With<MapPinIcon>, Or<(Changed<MapPin>, Changed<MapPinIcon>)>)>,
    mut removed_pins: RemovedComponents<MapPin>,
    roots: Query<&RendererSceneContext, With<ReadsMapPins>>,
) {
    let new: Vec<_> = events
        .read()
        .filter_map(|ev| match ev {
            SystemApi::GetMapPinsStream(sender) => Some(sender.clone()),
            _ => None,
        })
        .collect();
    let removed = removed_pins.read().count() > 0;
    streams.senders.extend(new.iter().cloned());
    streams.senders.retain(|s| !s.is_closed());
    if streams.senders.is_empty() {
        streams.last = None;
        return;
    }

    if removed || streams.last.is_none() || !changed.is_empty() {
        let mut list: Vec<_> = pins
            .iter()
            .filter_map(|(icon, pin, scene_entity)| {
                let context = roots.get(scene_entity.root).ok()?;
                let parcel = pin.parcel?;
                Some(MapPinInfo {
                    id: format!("{}:{}", context.hash, scene_entity.id),
                    x: parcel.x,
                    y: parcel.y,
                    title: pin.title.clone(),
                    icon: icon.url.clone(),
                })
            })
            .collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        if streams.last.as_ref() != Some(&list) {
            for sender in streams.senders.iter() {
                let _ = sender.send(list.clone());
            }
            streams.last = Some(list);
            return;
        }
    }
    if let Some(list) = streams.last.as_ref() {
        for sender in new {
            let _ = sender.send(list.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_component::proto_components::common::{Texture, TextureUnion, Vector2};
    use ipfs::{ContentMap, IpfsModifier};

    const CONTENT: &str = "https://content.test/contents/";

    fn pb(x: f32, y: f32, tex: Option<Tex>) -> PbMapPin {
        PbMapPin {
            position: Some(Vector2 { x, y }),
            texture: tex.map(|tex| TextureUnion { tex: Some(tex) }),
            icon_size: 1.0,
            title: "Memory Grid".to_owned(),
            description: "Win a round".to_owned(),
        }
    }

    fn texture(src: &str) -> Tex {
        Tex::Texture(Texture {
            src: src.to_owned(),
            ..Default::default()
        })
    }

    #[test]
    fn a_pin_is_its_parcel_rounded_halves_to_even() {
        assert_eq!(
            MapPin::from(pb(-10.5, 3.5, None)).parcel,
            Some(IVec2::new(-10, 4))
        );
        assert_eq!(
            MapPin::from(pb(12.4, -0.6, None)).parcel,
            Some(IVec2::new(12, -1))
        );
        let mut no_position = pb(0.0, 0.0, None);
        no_position.position = None;
        assert_eq!(MapPin::from(no_position).parcel, None);
        assert_eq!(MapPin::from(pb(f32::NAN, 0.0, None)).parcel, None);
    }

    #[test]
    fn only_a_scene_texture_is_an_icon_source() {
        assert_eq!(
            MapPin::from(pb(0.0, 0.0, Some(texture("images/icons/memory_grid.png")))).icon_src,
            Some("images/icons/memory_grid.png".to_owned())
        );
        assert_eq!(MapPin::from(pb(0.0, 0.0, Some(texture("")))).icon_src, None);
        let avatar = Tex::AvatarTexture(Default::default());
        assert_eq!(MapPin::from(pb(0.0, 0.0, Some(avatar))).icon_src, None);
    }

    #[test]
    fn a_long_title_is_cut_on_a_character() {
        let mut long = pb(0.0, 0.0, None);
        long.title = "é".repeat(MAX_TITLE_CHARS + 5);
        assert_eq!(MapPin::from(long).title, "é".repeat(MAX_TITLE_CHARS));
        assert_eq!(MapPin::from(pb(0.0, 0.0, None)).title, "Memory Grid");
    }

    /// `bafypx` holds two files, served from [`CONTENT`].
    fn app() -> App {
        let ipfs = ipfs::IpfsIo::new(
            false,
            Box::new(bevy::asset::io::memory::MemoryAssetReader { root: default() }),
            None,
            default(),
            4,
            tokio::sync::mpsc::unbounded_channel().0,
        );
        ipfs.add_collection(
            "bafypx".to_owned(),
            ContentMap::from_content([
                ("images/icon.png".to_owned(), "bafyicon".to_owned()),
                ("images/other.png".to_owned(), "bafyother".to_owned()),
            ]),
            Some(IpfsModifier {
                base_url: Some(CONTENT.to_owned()),
            }),
            None,
        );
        IoTaskPool::get_or_init(Default::default);
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .insert_resource(IpfsResource {
                inner: std::sync::Arc::new(ipfs),
            })
            .init_resource::<MapPinStreams>()
            .add_observer(clear_map_pin_icon)
            .add_systems(Update, (resolve_map_pin_icons, stream_map_pins).chain());
        app
    }

    fn scene_root(app: &mut App, hash: &str, reads: bool) -> Entity {
        let root = app.world_mut().spawn_empty().id();
        let context = RendererSceneContext::new(
            dcl_types::SceneId(root),
            hash.to_owned(),
            "storage_root".to_owned(),
            reads,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            std::iter::once(IVec2::ZERO).collect(),
            vec![],
            Default::default(),
            vec![],
            root,
            0.,
            false,
            "sdk7",
            false,
            false,
        );
        let mut root_entity = app.world_mut().entity_mut(root);
        root_entity.insert(context);
        if reads {
            root_entity.insert(ReadsMapPins);
        }
        root
    }

    fn spawn_pin(app: &mut App, root: Entity, id: u16, pin: PbMapPin) -> Entity {
        app.world_mut()
            .spawn((
                MapPin::from(pin),
                SceneEntity {
                    root,
                    scene_id: dcl_types::SceneId(root),
                    id: dcl_component::SceneEntityId::new(id, 0),
                },
            ))
            .id()
    }

    fn subscribe(app: &mut App) -> common::rpc::RpcStreamReceiver<Vec<MapPinInfo>> {
        let (sender, rx) = RpcStreamSender::channel();
        app.world_mut()
            .send_event(SystemApi::GetMapPinsStream(sender));
        rx
    }

    fn drain(rx: &mut common::rpc::RpcStreamReceiver<Vec<MapPinInfo>>) -> Vec<Vec<MapPinInfo>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    fn update_until(
        app: &mut App,
        rx: &mut common::rpc::RpcStreamReceiver<Vec<MapPinInfo>>,
        done: impl Fn(&[MapPinInfo]) -> bool,
    ) -> Vec<Vec<MapPinInfo>> {
        let mut sent = Vec::new();
        for _ in 0..400 {
            app.update();
            sent.extend(drain(rx));
            if sent.last().is_some_and(|list| done(list)) {
                return sent;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("never sent; got {sent:?}");
    }

    fn set_pin(app: &mut App, pin: Entity, value: PbMapPin) {
        *app.world_mut().get_mut::<MapPin>(pin).unwrap() = MapPin::from(value);
    }

    #[test]
    fn only_portable_experience_scenes_send_their_pins_and_removal_is_sent() {
        let mut app = app();
        let px = scene_root(&mut app, "bafypx", true);
        let parcel_scene = scene_root(&mut app, "bafyparcel", false);
        let px_pin = spawn_pin(&mut app, px, 512, pb(10.0, 0.0, None));
        spawn_pin(&mut app, parcel_scene, 512, pb(20.0, 0.0, None));

        let mut rx = subscribe(&mut app);
        app.update();
        app.update();
        let sent = drain(&mut rx);
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0]
                .iter()
                .map(|p| (p.x, p.id.as_str()))
                .collect::<Vec<_>>(),
            vec![(10, "bafypx:dcl_512v0")]
        );

        app.update();
        assert!(drain(&mut rx).is_empty());

        set_pin(&mut app, px_pin, pb(11.0, 0.0, None));
        app.update();
        let sent = drain(&mut rx);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0][0].x, 11);

        // a write of the same value sends nothing
        set_pin(&mut app, px_pin, pb(11.0, 0.0, None));
        app.update();
        assert!(drain(&mut rx).is_empty());

        let mut late = subscribe(&mut app);
        app.update();
        assert_eq!(drain(&mut late).len(), 1);
        assert!(drain(&mut rx).is_empty());

        app.world_mut().entity_mut(px_pin).remove::<MapPin>();
        app.update();
        assert_eq!(drain(&mut rx), vec![vec![]]);
    }

    #[test]
    fn a_despawned_scene_entity_drops_its_pin() {
        let mut app = app();
        let px = scene_root(&mut app, "bafypx", true);
        let px_pin = spawn_pin(&mut app, px, 512, pb(10.0, 0.0, None));
        let mut rx = subscribe(&mut app);
        app.update();
        assert_eq!(drain(&mut rx).len(), 1);
        app.world_mut().despawn(px_pin);
        app.update();
        assert_eq!(drain(&mut rx), vec![vec![]]);
    }

    #[test]
    fn a_content_file_is_the_icon_and_anything_else_draws_the_default() {
        let mut app = app();
        let px = scene_root(&mut app, "bafypx", true);
        let srcs = [
            "images/icon.png",
            "/Images/Icon.png",
            "images/missing.png",
            // a content-map miss that parses as a url would be fetched as one
            "https://host.example",
            "HTTPS://Host.Example/x",
            "https://host.example:443/a b.png",
            "/https://host.example/x.png",
            " http://host.example/x.png",
            "data:image/png;base64,AAAA",
        ];
        for (id, src) in srcs.iter().enumerate() {
            spawn_pin(
                &mut app,
                px,
                512 + id as u16,
                pb(id as f32, 0.0, Some(texture(src))),
            );
        }
        let mut rx = subscribe(&mut app);
        let sent = update_until(&mut app, &mut rx, |list| list.len() == srcs.len());
        let icons: Vec<_> = sent
            .last()
            .unwrap()
            .iter()
            .map(|p| p.icon.as_deref())
            .collect();
        let icon = Some("https://content.test/contents/bafyicon");
        let mut expected = vec![None; srcs.len()];
        expected[0] = icon;
        expected[1] = icon;
        assert_eq!(icons, expected);
    }

    #[test]
    fn a_moving_pin_keeps_its_icon_and_a_new_icon_replaces_it_without_a_gap() {
        let mut app = app();
        let px = scene_root(&mut app, "bafypx", true);
        let pin = spawn_pin(
            &mut app,
            px,
            512,
            pb(1.0, 1.0, Some(texture("images/icon.png"))),
        );
        let mut rx = subscribe(&mut app);
        update_until(&mut app, &mut rx, |list| list.len() == 1);

        set_pin(
            &mut app,
            pin,
            pb(2.0, 1.0, Some(texture("images/icon.png"))),
        );
        app.update();
        let sent = drain(&mut rx);
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert_eq!(sent[0][0].x, 2);
        assert_eq!(
            sent[0][0].icon.as_deref(),
            Some("https://content.test/contents/bafyicon")
        );
        assert!(!app.world().entity(pin).contains::<ResolvingMapPinIcon>());

        set_pin(
            &mut app,
            pin,
            pb(2.0, 1.0, Some(texture("images/other.png"))),
        );
        let sent = update_until(&mut app, &mut rx, |list| {
            list[0].icon.as_deref() == Some("https://content.test/contents/bafyother")
        });
        assert!(sent.iter().all(|list| list.len() == 1), "{sent:?}");
    }

    #[test]
    fn removing_a_pin_drops_its_icon() {
        let mut app = app();
        let px = scene_root(&mut app, "bafypx", true);
        let pin = spawn_pin(
            &mut app,
            px,
            512,
            pb(1.0, 1.0, Some(texture("images/icon.png"))),
        );
        app.update();
        assert!(app.world().entity(pin).contains::<ResolvingMapPinIcon>());
        app.world_mut().entity_mut(pin).remove::<MapPin>();
        app.update();
        let entity = app.world().entity(pin);
        assert!(!entity.contains::<ResolvingMapPinIcon>());
        assert!(!entity.contains::<MapPinIcon>());
    }
}
