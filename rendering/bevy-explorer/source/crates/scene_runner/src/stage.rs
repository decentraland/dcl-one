//! The one classification of a scene's load and run, for the readouts and the waits that name
//! it: [`Stage`], and the lifecycle state of a parcel's scene ([`state_of`]).

use crate::{
    initialize_scene::{NoParcelScene, SceneLoading},
    load_timing::scene_is_halted,
    renderer_context::RendererSceneContext,
};

/// The stage a scene's load or run is at: the one classification `/scene_loading_state`
/// (`debug_panel`) and the settling waits (`util::settle`) read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    Spawning,
    Definition,
    MainCrdt,
    Javascript,
    Failed,
    Broken,
    Blocked,
    /// Running, still in its startup ticks (`renderer_context::STARTUP_TICKS`).
    Starting,
    /// Started ([`RendererSceneContext::started`]) and not broken.
    Running,
    Unstarted,
}

impl Stage {
    pub(crate) fn of(
        loading: Option<&SceneLoading>,
        context: Option<&RendererSceneContext>,
    ) -> Self {
        match (loading, context) {
            (Some(SceneLoading::SceneSpawned), _) => Self::Spawning,
            (Some(SceneLoading::SceneEntity), _) => Self::Definition,
            (Some(SceneLoading::MainCrdt { .. }), _) => Self::MainCrdt,
            (Some(SceneLoading::Javascript { .. }), _) => Self::Javascript,
            (Some(SceneLoading::Failed), _) => Self::Failed,
            (None, Some(context)) if context.broken() => Self::Broken,
            (None, Some(context)) if !context.blocked.is_empty() => Self::Blocked,
            (None, Some(context)) if !context.started() => Self::Starting,
            (None, Some(_)) => Self::Running,
            (None, None) => Self::Unstarted,
        }
    }

    /// The stage as `/scene_loading_state` names it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Spawning => "Spawning",
            Self::Definition => "Reading definition",
            Self::MainCrdt => "Reading main.crdt",
            Self::Javascript => "Starting JavaScript",
            Self::Failed => "Failed",
            Self::Broken => "Broken",
            Self::Blocked => "Blocked",
            Self::Starting => "Starting",
            Self::Running => "Running",
            Self::Unstarted => "Waiting for the parcel scene to start.",
        }
    }

    /// The lifecycle state the settling replies and `/scene_loading_state` give the stage:
    /// `loading` (definition, main.crdt or JavaScript still being fetched), `failed`, `starting`,
    /// `blocked`, `broken` or `running`.
    pub(crate) fn state(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Broken => "broken",
            Self::Blocked => "blocked",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Spawning
            | Self::Definition
            | Self::MainCrdt
            | Self::Javascript
            | Self::Unstarted => "loading",
        }
    }

    /// The scene runs with its content shown: the readiness `/teleport --wait` and
    /// `/reload --wait` wait for. A halted scene ([`Self::halted`]) shows what it has and is
    /// never ready.
    pub(crate) fn is_ready(self) -> bool {
        self == Self::Running
    }

    /// Why a scene at this stage will never be ready, for one that failed to load or is halted
    /// ([`scene_is_halted`]: broken, or frozen with its glTFs loaded); `None` while it still
    /// may be. A settling wait ends on this or on [`Self::is_ready`].
    pub(crate) fn halted(self, context: Option<&RendererSceneContext>) -> Option<&'static str> {
        match self {
            Self::Failed => Some("failed"),
            Self::Broken => Some("broken"),
            Self::Blocked if context.is_some_and(scene_is_halted) => Some("frozen"),
            _ => None,
        }
    }
}

/// The lifecycle state of the scene looked up on a parcel: its stage's, `none` on a parcel with
/// no scene, `refused` on one whose scene refuses the player, and `loading` while the parcel's
/// scene is not known or has not spawned.
pub(crate) fn state_of(scene: Result<Stage, NoParcelScene>) -> &'static str {
    match scene {
        Ok(stage) => stage.state(),
        Err(NoParcelScene::Empty) => "none",
        Err(NoParcelScene::Refused) => "refused",
        Err(NoParcelScene::Unresolved | NoParcelScene::NotSpawned) => Stage::Unstarted.state(),
    }
}

/// For tests: the scene `hash` refuses the local player from here on, as a scene ban does.
#[cfg(test)]
pub(crate) fn refuse_player(world: &mut bevy::prelude::World, hash: &str) {
    let mut wallet = wallet::Wallet::default();
    wallet.finalize_as_guest_with_seed([1; 32]);
    let address = wallet.address().unwrap();
    world.insert_resource(wallet);
    let realm = world
        .resource::<common::structs::CurrentRealm>()
        .about_url
        .clone();
    world.insert_resource(
        comms::SceneAdmission::from_refusal(
            403,
            Some("User is banned from this scene"),
            false,
            hash,
        )
        .for_session(&realm, address),
    );
}
