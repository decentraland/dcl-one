//! Scene map pins (`MapPin`) for the HUD map. Only scenes whose scene.json declares
//! `isPortableExperience: true` are read; the HUD follows them through `GetMapPinsStream`.

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::{
    rpc::RpcStreamSender, sets::SceneSets, structs::DeclaresPortableExperience, util::TaskExt,
};
use dcl_component::{
    proto_components::{common::texture_union::Tex, sdk::components::PbMapPin},
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    IpfsResource,
};
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::AddCrdtInterfaceExt, SceneEntity,
};
use system_bridge::{MapPinInfo, SystemApi};

mod diagnostics;

pub struct MapPinsPlugin;

impl Plugin for MapPinsPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbMapPin, MapPin>(
            SceneComponentId::MAP_PIN,
            ComponentPosition::EntityOnly,
        );
        app.init_resource::<MapPinStreams>();
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
pub struct MapPin {
    /// `None` when the position is missing or not finite: the pin is not shown.
    pub parcel: Option<IVec2>,
    pub title: String,
    pub description: String,
    /// A file of the scene's content; other texture kinds draw the default pin.
    pub icon_src: Option<String>,
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
        Self {
            parcel,
            title: pin.title,
            description: pin.description,
            icon_src,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct MapPinIcon(pub Option<String>);

#[derive(Component)]
struct ResolvingMapPinIcon(Task<Option<String>>);

#[derive(Resource, Default)]
struct MapPinStreams {
    senders: Vec<RpcStreamSender<Vec<MapPinInfo>>>,
    last: Option<Vec<MapPinInfo>>,
}

/// Only a file in the scene's content map becomes an icon: `to_url` hands back a src it can't
/// find unchanged when it parses as a url, and an external url is not drawn.
async fn icon_url(ipfs: IpfsResource, scene_hash: String, src: String) -> Option<String> {
    let context = ipfs.context.read().await;
    IpfsPath::new(IpfsType::new_content_file(scene_hash, src.clone()))
        .to_url(&context)
        .ok()
        .filter(|url| *url != src)
}

fn resolve_map_pin_icons(
    mut commands: Commands,
    changed: Query<(Entity, &MapPin, &SceneEntity), Changed<MapPin>>,
    mut resolving: Query<(Entity, &mut ResolvingMapPinIcon)>,
    roots: Query<&RendererSceneContext, With<DeclaresPortableExperience>>,
    ipfs: Res<IpfsResource>,
) {
    for (entity, pin, scene_entity) in changed.iter() {
        let Ok(context) = roots.get(scene_entity.root) else {
            continue;
        };
        let mut entity = commands.entity(entity);
        match pin.icon_src.clone() {
            Some(src) => {
                let task =
                    IoTaskPool::get().spawn(icon_url(ipfs.clone(), context.hash.to_string(), src));
                entity
                    .remove::<MapPinIcon>()
                    .insert(ResolvingMapPinIcon(task));
            }
            None => {
                entity
                    .remove::<ResolvingMapPinIcon>()
                    .insert(MapPinIcon(None));
            }
        }
    }
    for (entity, mut task) in resolving.iter_mut() {
        if let Some(icon) = task.0.complete() {
            commands
                .entity(entity)
                .remove::<ResolvingMapPinIcon>()
                .insert(MapPinIcon(icon));
        }
    }
}

fn stream_map_pins(
    mut streams: ResMut<MapPinStreams>,
    mut events: EventReader<SystemApi>,
    pins: Query<(Ref<MapPinIcon>, &MapPin, &SceneEntity)>,
    mut removed_pins: RemovedComponents<MapPin>,
    mut removed_icons: RemovedComponents<MapPinIcon>,
    roots: Query<&RendererSceneContext, With<DeclaresPortableExperience>>,
) {
    let new: Vec<_> = events
        .read()
        .filter_map(|ev| match ev {
            SystemApi::GetMapPinsStream(sender) => Some(sender.clone()),
            _ => None,
        })
        .collect();
    let removed = removed_pins.read().count() + removed_icons.read().count() > 0;
    streams.senders.extend(new.iter().cloned());
    streams.senders.retain(|s| !s.is_closed());
    if streams.senders.is_empty() {
        streams.last = None;
        return;
    }

    if removed || streams.last.is_none() || pins.iter().any(|(icon, ..)| icon.is_changed()) {
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
                    description: pin.description.clone(),
                    icon: icon.0.clone(),
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

    fn app() -> App {
        let ipfs = ipfs::IpfsIo::new(
            false,
            Box::new(bevy::asset::io::memory::MemoryAssetReader { root: default() }),
            None,
            default(),
            4,
            tokio::sync::mpsc::unbounded_channel().0,
        );
        IoTaskPool::get_or_init(Default::default);
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .insert_resource(IpfsResource {
                inner: std::sync::Arc::new(ipfs),
            })
            .init_resource::<MapPinStreams>()
            .add_systems(Update, (resolve_map_pin_icons, stream_map_pins).chain());
        app
    }

    fn scene_root(app: &mut App, hash: &str, declares: bool) -> Entity {
        let root = app.world_mut().spawn_empty().id();
        let context = RendererSceneContext::new(
            dcl_types::SceneId(root),
            hash.to_owned(),
            "storage_root".to_owned(),
            false,
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
        if declares {
            root_entity.insert(DeclaresPortableExperience);
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

        *app.world_mut().get_mut::<MapPin>(px_pin).unwrap() = MapPin::from(pb(11.0, 0.0, None));
        app.update();
        let sent = drain(&mut rx);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0][0].x, 11);

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
    fn a_pin_waits_for_its_icon_and_a_file_outside_the_content_draws_the_default() {
        let mut app = app();
        let px = scene_root(&mut app, "bafypx", true);
        spawn_pin(
            &mut app,
            px,
            512,
            pb(1.0, 1.0, Some(texture("images/icons/missing.png"))),
        );
        let mut rx = subscribe(&mut app);
        let mut sent = Vec::new();
        for _ in 0..200 {
            app.update();
            sent.extend(drain(&mut rx));
            if sent.iter().any(|list| !list.is_empty()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let pins = sent.into_iter().find(|list| !list.is_empty()).unwrap();
        assert_eq!(pins[0].icon, None);
    }
}
