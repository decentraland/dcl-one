use std::collections::VecDeque;

use crate::{renderer_context::RendererSceneContext, ContainingScene};
use bevy::{ecs::system::SystemParam, prelude::*};
use common::{
    autopilot::Autopilot,
    dynamics::PLAYER_COLLIDER_RADIUS,
    rpc::{RpcResultReceiver, RpcResultSender},
    structs::{
        AppConfig, CurrentRealm, PermissionType, PermissionUsed, PermissionValue, PlayerModifiers,
        PrimaryCamera, PrimaryPlayerRes, PrimaryUser, StartupScenes,
    },
};
use tokio::sync::oneshot::error::TryRecvError;

pub struct PermissionRequest {
    pub realm: String,
    pub scene: Entity,
    pub is_portable: bool,
    pub additional: Option<String>,
    pub ty: PermissionType,
    pub sender: RpcResultSender<bool>,
}

#[derive(Resource, Default)]
pub struct PermissionManager {
    pub pending: VecDeque<PermissionRequest>,
}

impl PermissionManager {
    fn request(
        &mut self,
        ty: PermissionType,
        realm: String,
        scene: Entity,
        is_portable: bool,
        additional: Option<String>,
    ) -> RpcResultReceiver<bool> {
        let (sender, receiver) = RpcResultSender::channel();
        self.pending.push_back(PermissionRequest {
            realm,
            scene,
            is_portable,
            ty,
            sender,
            additional,
        });
        receiver
    }
}

#[derive(SystemParam)]
pub struct Permission<'w, 's, T: Send + Sync + 'static> {
    pub success: Local<'s, Vec<(T, PermissionType, Option<String>, Entity)>>,
    pub fail: Local<'s, Vec<(T, PermissionType, Entity)>>,
    pub pending: Local<
        's,
        Vec<(
            T,
            PermissionType,
            Entity,
            Option<String>,
            RpcResultReceiver<bool>,
        )>,
    >,
    config: Res<'w, AppConfig>,
    realm: Res<'w, CurrentRealm>,
    containing_scenes: ContainingScene<'w, 's>,
    player: Res<'w, PrimaryPlayerRes>,
    scenes: Query<'w, 's, &'static RendererSceneContext>,
    manager: ResMut<'w, PermissionManager>,
    system_scenes: Option<Res<'w, StartupScenes>>,
    uses: EventWriter<'w, PermissionUsed>,
    autopilot: Option<Res<'w, Autopilot>>,
}

/// What a request resolves to before anything is asked of a person. The autopilot has no one to
/// ask and takes no stored rule (`common::autopilot::permission`). What a person would be asked
/// about a scene the player is not in (`ask_out_of_scene`) it denies: the table's allowances are
/// for the scene the player stands in, so a neighbour or a portable cannot lock, animate or
/// otherwise act on the player.
fn resolve(
    autopilot: bool,
    config: &AppConfig,
    ty: PermissionType,
    realm: &str,
    scene_hash: &str,
    is_portable: bool,
    ask_out_of_scene: bool,
) -> PermissionValue {
    if autopilot {
        if ask_out_of_scene {
            PermissionValue::Deny
        } else {
            common::autopilot::permission(ty)
        }
    } else if ask_out_of_scene {
        PermissionValue::Ask
    } else {
        config.get_permission(ty, realm, scene_hash, is_portable)
    }
}

impl<T: Send + Sync + 'static> Permission<'_, '_, T> {
    // in scene, hash, title, is_portable
    fn get_scene_info(&self, scene: Entity) -> Option<(bool, &str, &str, bool)> {
        let in_scene = self
            .containing_scenes
            .get_area(self.player.0, PLAYER_COLLIDER_RADIUS)
            .contains(&scene);
        self.scenes
            .get(scene)
            .map(|ctx| (in_scene, &*ctx.hash, ctx.title.as_str(), ctx.is_portable))
            .ok()
    }

    pub fn is_system_scene(&self, hash: &str) -> bool {
        self.system_scenes.as_ref().is_some_and(|ss| {
            ss.scenes
                .iter()
                .any(|scene| scene.hash.as_ref().is_some_and(|sh| sh == hash) && scene.super_user)
        })
    }

    pub fn check(
        &mut self,
        ty: PermissionType,
        scene: Entity,
        value: T,
        additional: Option<String>,
        allow_out_of_scene: bool,
    ) {
        let Some((in_scene, hash, _, is_portable)) = self.get_scene_info(scene) else {
            self.fail.push((value, ty, scene));
            return;
        };

        // allow system scene to do anything
        if self.is_system_scene(hash) {
            self.success.push((value, ty, additional, scene));
            return;
        };

        let perm = resolve(
            self.autopilot.is_some(),
            &self.config,
            ty,
            &self.realm.about_url,
            hash,
            is_portable,
            !allow_out_of_scene && !in_scene,
        );

        debug!(
            "req {:?} for {:?} -> {:?} (in_scene = {}, allow_out = {}, real = {:?})",
            ty,
            scene,
            perm,
            in_scene,
            allow_out_of_scene,
            self.config
                .get_permission(ty, &self.realm.about_url, hash, is_portable)
        );
        match perm {
            common::structs::PermissionValue::Allow => {
                self.success.push((value, ty, additional, scene))
            }
            common::structs::PermissionValue::Deny => self.fail.push((value, ty, scene)),
            common::structs::PermissionValue::Ask => {
                self.pending.push((
                    value,
                    ty,
                    scene,
                    additional.clone(),
                    self.manager.request(
                        ty,
                        self.realm.about_url.clone(),
                        scene,
                        is_portable,
                        additional,
                    ),
                ));
            }
        }
    }

    pub fn check_unique(
        &mut self,
        ty: PermissionType,
        scene: Entity,
        value: T,
        additional: Option<String>,
        allow_out_of_scene: bool,
    ) where
        T: Eq,
    {
        if !self.iter_pending().any(|v| *v == value) {
            self.check(ty, scene, value, additional, allow_out_of_scene);
        }
    }

    fn update_pending(&mut self) {
        let pending = std::mem::take(&mut *self.pending);
        *self.pending = pending
            .into_iter()
            .flat_map(
                |(value, ty, scene, additional, mut rx)| match rx.try_recv() {
                    Ok(true) => {
                        self.success.push((value, ty, additional, scene));
                        None
                    }
                    Ok(false) => {
                        self.fail.push((value, ty, scene));
                        None
                    }
                    Err(TryRecvError::Closed) => {
                        let (_, _, _, is_portable) = self.get_scene_info(scene)?;
                        warn!("unexpected close of channel, re-requesting");
                        Some((
                            value,
                            ty,
                            scene,
                            additional.clone(),
                            self.manager.request(
                                ty,
                                self.realm.about_url.clone(),
                                scene,
                                is_portable,
                                additional,
                            ),
                        ))
                    }
                    Err(TryRecvError::Empty) => Some((value, ty, scene, additional, rx)),
                },
            )
            .collect();
    }

    pub fn drain_success(&mut self, ty: PermissionType) -> impl Iterator<Item = T> {
        self.update_pending();

        let (matching, not_matching): (Vec<_>, Vec<_>) = self
            .success
            .drain(..)
            .partition(|(_, perm_ty, _, _)| *perm_ty == ty);
        *self.success = not_matching;

        for item in matching.iter() {
            let (_, _, additional, scene) = item;
            let scene = *scene;
            if let Some((_, hash, _, _)) = self.get_scene_info(scene) {
                if !self.is_system_scene(hash) {
                    let used = PermissionUsed {
                        ty,
                        additional: additional.clone(),
                        scene: hash.to_owned(),
                        was_allowed: true,
                        in_force: None,
                    };
                    self.uses.write(used);
                }
            }
        }
        matching.into_iter().map(|(value, _, _, _)| value)
    }

    pub fn drain_fail(&mut self, ty: PermissionType) -> impl Iterator<Item = T> + '_ {
        let (matching, not_matching): (Vec<_>, Vec<_>) = self
            .fail
            .drain(..)
            .partition(|(_, perm_ty, _)| *perm_ty == ty);
        *self.fail = not_matching;

        for item in matching.iter() {
            let (_, _, scene) = item;
            let scene = *scene;
            if let Some((_, hash, _, _)) = self.get_scene_info(scene) {
                if !self.is_system_scene(hash) {
                    let used = PermissionUsed {
                        ty,
                        additional: None,
                        scene: hash.to_owned(),
                        was_allowed: false,
                        in_force: None,
                    };
                    self.uses.write(used);
                }
            }
        }
        matching.into_iter().map(|(value, _, _)| value)
    }

    pub fn iter_pending(&self) -> impl Iterator<Item = &T> + '_ {
        self.pending.iter().map(|(value, ..)| value)
    }
}

/// The permissions whose effect acts on the player now, each with the hash of the scene that
/// causes it: sorted, and without repeats. `system_ui` sends it to a stream that attaches.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct PermissionsInForce(pub Vec<(String, PermissionType)>);

impl PermissionsInForce {
    /// From the scenes behind each effect; `hash` names a scene, and one it does not know (a
    /// scene that is gone) is left out.
    fn new(
        camera: Option<Entity>,
        hide: &[Entity],
        locomotion: &[Entity],
        hash: impl Fn(Entity) -> Option<String>,
    ) -> Self {
        let of = |scenes: &[Entity], ty: PermissionType| {
            scenes
                .iter()
                .filter_map(|scene| Some((hash(*scene)?, ty)))
                .collect::<Vec<_>>()
        };
        let mut list = of(camera.as_slice(), PermissionType::ForceCamera);
        list.extend(of(hide, PermissionType::HideAvatarsNametags));
        list.extend(of(locomotion, PermissionType::SetLocomotion));
        list.sort_by(|(a_scene, a_ty), (b_scene, b_ty)| {
            (a_scene, *a_ty as u8).cmp(&(b_scene, *b_ty as u8))
        });
        list.dedup();
        Self(list)
    }

    /// What tells a permission-used stream that `ty` started (`in_force`) or stopped acting on
    /// the player in `scene`.
    fn report(scene: &str, ty: PermissionType, in_force: bool) -> PermissionUsed {
        PermissionUsed {
            ty,
            additional: None,
            scene: scene.to_owned(),
            was_allowed: true,
            in_force: Some(in_force),
        }
    }

    /// A start for each permission in force: what a stream that attaches now has missed.
    pub fn starts(&self) -> impl Iterator<Item = PermissionUsed> + '_ {
        self.0
            .iter()
            .map(|(scene, ty)| Self::report(scene, *ty, true))
    }

    /// What to tell the permission-used streams when the permissions in force go from `self`
    /// to `new`: an end for each one that stopped, then a start for each new one.
    fn changes_to(&self, new: &Self) -> Vec<PermissionUsed> {
        let ended = self
            .0
            .iter()
            .filter(|entry| !new.0.contains(entry))
            .map(|(scene, ty)| Self::report(scene, *ty, false));
        let started = new
            .0
            .iter()
            .filter(|entry| !self.0.contains(entry))
            .map(|(scene, ty)| Self::report(scene, *ty, true));
        ended.chain(started).collect()
    }
}

/// Reports on the permission-used streams which permissions act on the player now, each under
/// the scene that causes it: the scene behind the held camera, and the scenes whose avatar
/// modifiers are applied to the player.
pub(crate) fn publish_permissions_in_force(
    player: Query<Option<&PlayerModifiers>, With<PrimaryUser>>,
    camera: Query<&PrimaryCamera>,
    scenes: Query<&RendererSceneContext>,
    mut in_force: ResMut<PermissionsInForce>,
    mut uses: EventWriter<PermissionUsed>,
) {
    let Ok(modifiers) = player.single() else {
        return;
    };
    let camera = camera
        .single()
        .ok()
        .filter(|camera| camera.scene_override.is_some())
        .and_then(|camera| camera.scene_override_owner);
    let now = PermissionsInForce::new(
        camera,
        modifiers.map_or(&[], |modifiers| &modifiers.hide_scenes),
        modifiers.map_or(&[], |modifiers| &modifiers.locomotion_scenes),
        |scene| {
            scenes
                .get(scene)
                .ok()
                .map(|context| context.hash.to_string())
        },
    );
    if *in_force != now {
        uses.write_batch(in_force.changes_to(&now));
        *in_force = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARCEL: Entity = Entity::from_raw(1);
    const NEIGHBOUR: Entity = Entity::from_raw(2);
    const PORTABLE: Entity = Entity::from_raw(3);

    fn hash(scene: Entity) -> Option<String> {
        [
            (PARCEL, "parcel"),
            (NEIGHBOUR, "neighbour"),
            (PORTABLE, "portable"),
        ]
        .into_iter()
        .find(|(known, _)| *known == scene)
        .map(|(_, hash)| hash.to_owned())
    }

    fn in_force(entries: &[(&str, PermissionType)]) -> PermissionsInForce {
        PermissionsInForce(
            entries
                .iter()
                .map(|(scene, ty)| ((*scene).to_owned(), *ty))
                .collect(),
        )
    }

    fn changes(
        old: &PermissionsInForce,
        new: &PermissionsInForce,
    ) -> Vec<(String, PermissionType, bool)> {
        old.changes_to(new)
            .into_iter()
            .map(|used| {
                assert!(used.was_allowed);
                (used.scene, used.ty, used.in_force.unwrap())
            })
            .collect()
    }

    #[test]
    fn each_effect_is_listed_under_the_scene_that_causes_it() {
        use PermissionType::*;
        assert_eq!(PermissionsInForce::new(None, &[], &[], hash), in_force(&[]));
        // a portable holds the camera, a neighbour's area reaches the player and the parcel
        // scene blocks movement: none of them is the parcel scene's but the last
        assert_eq!(
            PermissionsInForce::new(Some(PORTABLE), &[NEIGHBOUR], &[PARCEL], hash),
            in_force(&[
                ("neighbour", HideAvatarsNametags),
                ("parcel", SetLocomotion),
                ("portable", ForceCamera),
            ])
        );
        // two areas of one scene are one entry, and a scene that is gone is none
        assert_eq!(
            PermissionsInForce::new(
                Some(Entity::from_raw(9)),
                &[PARCEL, PARCEL],
                &[PARCEL, PORTABLE],
                hash
            ),
            in_force(&[
                ("parcel", SetLocomotion),
                ("parcel", HideAvatarsNametags),
                ("portable", SetLocomotion),
            ])
        );
    }

    #[test]
    fn a_change_of_the_permissions_in_force_reports_each_start_and_end_once() {
        use PermissionType::*;
        let none = in_force(&[]);
        let camera = in_force(&[("a", ForceCamera)]);
        let both = in_force(&[("a", ForceCamera), ("a", SetLocomotion)]);
        assert_eq!(
            changes(&none, &camera),
            [("a".to_owned(), ForceCamera, true)]
        );
        assert_eq!(
            changes(&camera, &both),
            [("a".to_owned(), SetLocomotion, true)]
        );
        assert_eq!(
            changes(&both, &camera),
            [("a".to_owned(), SetLocomotion, false)]
        );
        assert!(changes(&camera, &camera).is_empty());
    }

    #[test]
    fn the_same_permission_passing_to_another_scene_ends_in_one_and_starts_in_the_other() {
        use PermissionType::*;
        let a = in_force(&[("a", ForceCamera)]);
        let b = in_force(&[("b", ForceCamera)]);
        assert_eq!(
            changes(&a, &b),
            [
                ("a".to_owned(), ForceCamera, false),
                ("b".to_owned(), ForceCamera, true)
            ]
        );
        assert_eq!(
            changes(&a, &in_force(&[])),
            [("a".to_owned(), ForceCamera, false)]
        );
    }

    #[test]
    fn a_stream_that_attaches_is_told_everything_in_force() {
        use PermissionType::*;
        let now = in_force(&[("a", ForceCamera), ("b", SetLocomotion)]);
        let starts = now
            .starts()
            .map(|used| (used.scene, used.ty, used.in_force))
            .collect::<Vec<_>>();
        assert_eq!(
            starts,
            [
                ("a".to_owned(), ForceCamera, Some(true)),
                ("b".to_owned(), SetLocomotion, Some(true))
            ]
        );
    }
    fn config_with(rules: &[(PermissionType, PermissionValue)]) -> AppConfig {
        let mut config = AppConfig::default();
        config
            .scene_permissions
            .insert("scene".to_owned(), rules.iter().copied().collect());
        config
            .realm_permissions
            .insert("realm".to_owned(), rules.iter().copied().collect());
        config.default_permissions = rules.iter().copied().collect();
        config
    }

    fn answer(
        autopilot: bool,
        config: &AppConfig,
        ty: PermissionType,
        out_of_scene: bool,
    ) -> PermissionValue {
        resolve(autopilot, config, ty, "realm", "scene", false, out_of_scene)
    }

    #[test]
    fn the_autopilot_answers_the_scene_the_player_is_in_itself_whatever_is_stored() {
        use PermissionType::*;
        // a machine whose user stored the opposite of the autopilot's policy
        let stored: Vec<_> = [Fetch, Websocket, Teleport, ChangeRealm, MovePlayer]
            .into_iter()
            .map(|ty| {
                let opposite = if common::autopilot::permission(ty) == PermissionValue::Allow {
                    PermissionValue::Deny
                } else {
                    PermissionValue::Allow
                };
                (ty, opposite)
            })
            .collect();
        let config = config_with(&stored);
        let allowed = |ty| answer(true, &config, ty, false) == PermissionValue::Allow;
        assert!(allowed(Fetch) && allowed(Websocket) && allowed(MovePlayer));
        assert!(!allowed(Teleport) && !allowed(ChangeRealm));
        assert_ne!(
            answer(true, &AppConfig::default(), OpenUrl, false),
            PermissionValue::Ask
        );
    }

    #[test]
    fn the_autopilot_denies_what_a_scene_the_player_is_not_in_asks_for() {
        use PermissionType::*;
        // the types that grant themselves to the scene the player stands in, and the ones the
        // table allows, with a stored rule that allows every one of them
        let types = [
            MovePlayer,
            ForceCamera,
            PlayEmote,
            SetLocomotion,
            DisableVoice,
            OpenExplorerUi,
            Fetch,
            Websocket,
            Teleport,
        ];
        let allow_all = config_with(&types.map(|ty| (ty, PermissionValue::Allow)));
        for ty in types {
            assert_eq!(
                answer(true, &allow_all, ty, true),
                PermissionValue::Deny,
                "{ty:?}"
            );
            assert_eq!(
                answer(true, &AppConfig::default(), ty, true),
                PermissionValue::Deny,
                "{ty:?}"
            );
        }
    }

    #[test]
    fn without_the_autopilot_the_stored_rules_and_the_user_decide() {
        let config = config_with(&[(PermissionType::Fetch, PermissionValue::Deny)]);
        let fetch =
            |config, out_of_scene| answer(false, config, PermissionType::Fetch, out_of_scene);
        assert_eq!(fetch(&config, false), PermissionValue::Deny);
        assert_eq!(fetch(&config, true), PermissionValue::Ask);
        assert_eq!(fetch(&AppConfig::default(), false), PermissionValue::Ask);
    }
}
