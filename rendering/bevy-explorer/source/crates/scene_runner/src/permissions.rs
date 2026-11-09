use std::collections::VecDeque;

use crate::{renderer_context::RendererSceneContext, ContainingScene};
use bevy::{ecs::system::SystemParam, prelude::*};
use common::{
    autopilot::Autopilot,
    dynamics::PLAYER_COLLIDER_RADIUS,
    rpc::{RpcResultReceiver, RpcResultSender},
    structs::{
        AppConfig, CurrentRealm, PermissionType, PermissionUsed, PermissionValue, PrimaryPlayerRes,
        StartupScenes,
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

#[allow(clippy::type_complexity)]
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

    fn is_system_scene(&self, hash: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

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
