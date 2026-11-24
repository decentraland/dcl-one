//! Scene map pins (`MapPin`) for the HUD map. Only portable experiences whose scene.json declares
//! `isPortableExperience: true` are read; the HUD follows them through `GetMapPinsStream`.

use bevy::prelude::*;
use common::{rpc::RpcStreamSender, sets::SceneSets, structs::ReadsMapPins};
use dcl_component::{
    proto_components::sdk::components::PbMapPin, ComponentPosition, SceneComponentId,
};
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::AddCrdtInterfaceExt, SceneEntity,
};
use system_bridge::{MapPinInfo, SystemApi};

mod diagnostics;

const MAX_TITLE_CHARS: usize = 100;

pub(crate) struct MapPinsPlugin;

impl Plugin for MapPinsPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbMapPin, MapPin>(
            SceneComponentId::MAP_PIN,
            ComponentPosition::EntityOnly,
        );
        app.init_resource::<MapPinStreams>();
        app.add_plugins(diagnostics::MapPinsDebugPanelPlugin);
        app.add_systems(Update, stream_map_pins.in_set(SceneSets::RestrictedActions));
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
struct MapPin {
    /// `None` when the position is missing or not finite: the pin is not shown.
    parcel: Option<IVec2>,
    title: String,
}

impl From<PbMapPin> for MapPin {
    fn from(pin: PbMapPin) -> Self {
        let parcel = pin
            .position
            .filter(|p| p.x.is_finite() && p.y.is_finite())
            // halves to even, so every client puts a pin on the same parcel
            .map(|p| IVec2::new(p.x.round_ties_even() as i32, p.y.round_ties_even() as i32));
        let mut title = pin.title;
        if let Some((cut, _)) = title.char_indices().nth(MAX_TITLE_CHARS) {
            title.truncate(cut);
        }
        Self { parcel, title }
    }
}

#[derive(Resource, Default)]
struct MapPinStreams {
    senders: Vec<RpcStreamSender<Vec<MapPinInfo>>>,
    last: Option<Vec<MapPinInfo>>,
}

fn stream_map_pins(
    mut streams: ResMut<MapPinStreams>,
    mut events: EventReader<SystemApi>,
    pins: Query<(&MapPin, &SceneEntity)>,
    changed: Query<(), Changed<MapPin>>,
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
            .filter_map(|(pin, scene_entity)| {
                let context = roots.get(scene_entity.root).ok()?;
                let parcel = pin.parcel?;
                Some(MapPinInfo {
                    id: format!("{}:{}", context.hash, scene_entity.id),
                    x: parcel.x,
                    y: parcel.y,
                    title: pin.title.clone(),
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
    use dcl_component::proto_components::common::{
        texture_union::Tex, Texture, TextureUnion, Vector2,
    };

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
    fn a_long_title_is_cut_on_a_character() {
        let mut long = pb(0.0, 0.0, None);
        long.title = "é".repeat(MAX_TITLE_CHARS + 5);
        assert_eq!(MapPin::from(long).title, "é".repeat(MAX_TITLE_CHARS));
        assert_eq!(MapPin::from(pb(0.0, 0.0, None)).title, "Memory Grid");
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .init_resource::<MapPinStreams>()
            .add_systems(Update, stream_map_pins);
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
    fn a_pin_with_a_texture_is_sent_as_any_other() {
        let mut app = app();
        let px = scene_root(&mut app, "bafypx", true);
        let pin = spawn_pin(
            &mut app,
            px,
            512,
            pb(1.0, 1.0, Some(texture("images/icon.png"))),
        );
        let mut rx = subscribe(&mut app);
        app.update();
        let expected = MapPinInfo {
            id: "bafypx:dcl_512v0".to_owned(),
            x: 1,
            y: 1,
            title: "Memory Grid".to_owned(),
        };
        assert_eq!(drain(&mut rx), vec![vec![expected]]);

        set_pin(
            &mut app,
            pin,
            pb(1.0, 1.0, Some(texture("images/other.png"))),
        );
        app.update();
        assert!(drain(&mut rx).is_empty());
    }
}
