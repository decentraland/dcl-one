#![forbid(unsafe_code)]

pub mod agent_commands;
pub mod settings;
pub mod world_access;

use std::sync::Arc;

use bevy::{
    app::{AppExit, Plugin, Update},
    ecs::event::EventReader,
    log::debug,
    math::Vec4,
    prelude::{Event, EventWriter, Res, ResMut, Resource},
};
use bevy_console::{ConsoleCommandEntered, ConsoleConfiguration, ConsoleResponder};
use common::{
    inputs::{BindingsData, HudPanel, InputIdentifier, SystemActionEvent},
    profile::SerializedProfile,
    rpc::{RpcResultSender, RpcStreamSender},
    structs::{CurrentRealm, MicState, PermissionUsed},
};
use serde::{Deserialize, Serialize};
use settings::SettingBridgePlugin;
pub use system_api_types::*;

pub struct SystemBridgePlugin {
    pub bare: bool,
}

impl Plugin for SystemBridgePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_event::<SystemApi>();
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        app.insert_resource(SystemBridge { sender, receiver });
        app.add_systems(
            Update,
            (
                post_events,
                handle_service_urls,
                handle_satellite_view,
                handle_exit,
                world_access::handle_world_access,
            ),
        );
        world_access::install_prompter(app);

        if self.bare {
            return;
        }

        app.add_plugins((SettingBridgePlugin, agent_commands::AgentCommandsPlugin));
    }
}

/// Prefix on login error strings when the failure was fetching the user's profile —
/// UIs match on this to offer retrying with a default profile (see the login variants'
/// `default_on_error` bool).
pub const PROFILE_FETCH_FAILED: &str = "profile fetch failed";

/// What an account without a profile starts as, picked before it existed (the guest start
/// page): a name, and one of the curated default looks by number (`default<look>`). Either
/// missing or invalid derives from the address instead.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NewAccountPick {
    pub name: Option<String>,
    pub look: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetAvatarError {
    pub message: String,
    #[serde(default, rename = "accountChanged")]
    pub account_changed: bool,
    /// Last confirmed profile for the account that submitted the edit.
    pub profile: Option<Box<SerializedProfile>>,
}

pub type SetAvatarResult = Result<u32, SetAvatarError>;

// SetAvatar is a rare call, and boxing it only moves the lint to the enums that hold a SystemApi
#[allow(clippy::large_enum_variant)]
#[derive(Event, Clone, Debug, Serialize, Deserialize)]
pub enum SystemApi {
    ConsoleCommand(String, Vec<String>, RpcResultSender<Result<String, String>>),
    /// A whole console line from outside the process (the page's console RPC, the loopback
    /// server, the CEF HUD's page), split by `console::line::split`.
    ConsoleLine(String, RpcResultSender<Result<String, String>>),
    CheckForUpdate(RpcResultSender<Option<(String, String)>>),
    MOTD(RpcResultSender<String>),
    GetPreviousLogin(RpcResultSender<Option<String>>),
    /// bool: continue with (and deploy) a default profile if the user's current profile
    /// can't be fetched — overwrites the server-side profile, so UIs should only set it
    /// after a failed attempt and with explicit user consent.
    LoginPrevious(bool, RpcResultSender<Result<(), String>>),
    /// bool: as for LoginPrevious.
    LoginNew(
        bool,
        RpcResultSender<Result<Option<i32>, String>>,
        RpcResultSender<Result<(), String>>,
    ),
    /// Log in with an AuthIdentity the web page already holds (base64-encoded AuthIdentity
    /// JSON read from localStorage) — no auth-server round-trip. The identity is the same
    /// regardless of how the user signed in (wallet, social, OTP, magic).
    /// bools: as for LoginPrevious; the identity is a guest account (see
    /// `Wallet::is_guest_account`). The pick names and dresses the profile a first login builds.
    LoginWithIdentity(
        String,
        bool,
        bool,
        Option<NewAccountPick>,
        RpcResultSender<Result<(), String>>,
    ),
    LoginGuest,
    /// The benchmark autopilot's guest: the account whose root key is kept in the config (made
    /// the first time), with a fresh identity signed by it every run and never stored as the
    /// previous login. Desktop and autopilot only: the web shell keeps its own key, and any other
    /// launch is refused.
    LoginLocalGuest(RpcResultSender<Result<(), String>>),
    LoginCancel,
    Logout,
    /// Drop the stored login, so the next start opens on sign-in: a person signing out. A guest
    /// account stays reachable through the wallet session the bridge scene keeps.
    ForgetLogin,
    GetSettings(RpcResultSender<Vec<SettingInfo>>),
    SetSetting(String, f32),
    /// The settings list now, then again whenever a setting's value or choices change.
    GetSettingsStream(RpcStreamSender<Vec<SettingInfo>>),
    /// Sign a request with the engine wallet, with caller-supplied metadata. Scene-originated
    /// signing goes through `RpcCall::SignRequest`, whose metadata the engine builds.
    SignRequest {
        method: String,
        uri: String,
        meta: Option<String>,
        response: RpcResultSender<Result<Vec<(String, String)>, String>>,
    },
    /// Full system-scene view. The local player is read at the last confirmed version.
    GetUserProfile {
        address: String,
        response: RpcResultSender<Result<SerializedProfile, ()>>,
    },
    /// The address belongs to the player the system scene observed when it submitted the edit.
    SetAvatar(String, SetAvatarData, RpcResultSender<SetAvatarResult>),
    /// Signs and deploys the local player's `outfits` entity. The address is the account the
    /// caller expects; the engine refuses when another one is signed in.
    DeployOutfits(String, OutfitsData, RpcResultSender<Result<(), String>>),
    GetNativeInput(RpcResultSender<InputIdentifier>),
    GetBindings(RpcResultSender<BindingsData>),
    SetBindings(BindingsData, RpcResultSender<()>),
    /// HUD focus state. `ui`: a HUD surface (menu/popup) is active — all input is reserved
    /// above scenes, while the system-action stream (which reads at the same level) keeps
    /// flowing so the HUD still receives Cancel/hotkeys. `text`: a HUD text field holds
    /// keyboard focus — the keyboard is reserved above the stream too, so keystrokes are
    /// typing and resolve to no action at all. `scroll`: the cursor is over a scrollable
    /// HUD element — the Scroll ACTIONS are reserved, so every input bound to them (wheel,
    /// key, gamepad button) stands down for world consumers while the action stream still
    /// resolves Scroll itself; the HUD scrolls the hovered panel from those edges.
    /// `covered`: a full-screen HUD surface (a menu page, the HUD's loading overlay) hides
    /// the world — scenes are told they are hidden (`PBEngineInfo.scene_hidden`). `menu`: the
    /// open full-screen menu page — backs the OpenExplorerUi restricted action's verdict and
    /// the scene-facing ExplorerUiEventsResult lifecycle events.
    SetUiFocus {
        ui: bool,
        text: bool,
        scroll: bool,
        covered: bool,
        menu: Option<HudPanel>,
    },
    LiveSceneInfo(RpcResultSender<Vec<LiveSceneInfo>>),
    GetHomeScene(RpcResultSender<HomeScene>),
    SetHomeScene(HomeScene),
    ClearHomeScene,
    /// None takes the lobby stage down.
    SetLobbyStage(Option<LobbyStage>),
    /// Whether the local user may enter the named world (`name.dcl.eth`).
    GetWorldAccess(String, RpcResultSender<WorldAccessVerdict>),
    /// Credential-free resolved service endpoints.
    GetServiceUrls(RpcResultSender<Vec<(String, String)>>),
    /// The official wallets the nametags mark, lowercase `0x` addresses.
    GetOfficialWallets(RpcResultSender<Vec<String>>),
    GetWelcome(RpcResultSender<WelcomeState>),
    AcceptTerms(RpcResultSender<()>),
    GetSatelliteView(RpcResultSender<Option<SatelliteView>>),
    /// A satellite map tile as base64-encoded JPEG bytes. The path is relative to the abgen
    /// CDN's `SatelliteMaps/` (`sections/<sx>%2C<sy>/<version>/<level>/<x>%2C<y>.ktx2`);
    /// `image_processing` validates it, fetches the tile and converts it.
    GetSatelliteTile(String, RpcResultSender<Result<String, String>>),
    GetSystemActionStream(RpcStreamSender<SystemActionEvent>),
    GetChatStream(RpcStreamSender<ChatMessage>),
    GetVoiceStream(RpcStreamSender<VoiceMessage>),
    SetNearbyMuted(String, bool, RpcResultSender<Result<(), String>>),
    /// The wallets whose nearby voice is muted: the list now, then again on every change.
    GetNearbyMuteStream(RpcStreamSender<Vec<String>>),
    GetHoverStream(RpcStreamSender<HoverEvent>),
    GetProximityStream(RpcStreamSender<ProximityEvent>),
    GetPhotoModeStream(RpcStreamSender<PhotoModeState>),
    GetProfileChangedStream(RpcStreamSender<ProfileChangedEvent>),
    GetSceneLoadingUiStream(RpcStreamSender<SceneLoadingUi>),
    GetAnalyticsEventStream(RpcStreamSender<AnalyticsEvent>),
    /// The preview server's texture import status for the previewed scene, as the preview socket
    /// delivers it (`comms::preview`); nothing outside preview mode.
    GetTextureOptimizationStream(RpcStreamSender<TextureOptimizationStatus>),
    // Native-only transport for the super-user bridge scene's BroadcastChannel: the scene posts page
    // -bound Envelopes via BridgeToPage, and subscribes to page->scene Envelopes via GetBridgeStream.
    // (In web the browser provides BroadcastChannel directly; native has no cross-process equivalent.)
    BridgeToPage(String),
    GetBridgeStream(RpcStreamSender<String>),
    SendChat(String, String),
    /// Adds or removes the local user's reaction to a chat message in a channel.
    SendChatReaction {
        channel: String,
        message_id: String,
        emoji: String,
        remove: bool,
    },
    GetChatReactionStream(RpcStreamSender<ChatReactionData>),
    /// The local user reacts with an emoji, shown over their avatar.
    SendReaction(String),
    /// Situational reactions as they play, the local user's included.
    GetReactionStream(RpcStreamSender<ReactionData>),
    Quit,
    GetPermissionRequestStream(RpcStreamSender<PermissionRequestEvent>),
    SetSinglePermission(SetSinglePermission),
    /// Scene requests to open the item purchase confirmation, sent as they are accepted.
    GetItemPurchaseRequestStream(RpcStreamSender<ItemPurchaseRequest>),
    SetItemPurchaseState(SetItemPurchaseState),
    SetPermanentPermission(SetPermanentPermission),
    GetPermissionUsedStream(RpcStreamSender<PermissionUsed>),
    GetPermanentPermissions(
        PermissionLevel,
        RpcResultSender<Vec<PermanentPermissionItem>>,
    ),
    /// The portables the player can see and stop, sent on subscribe and on every change.
    GetPortablesStream(RpcStreamSender<Vec<PortableInfo>>),
    /// The map pins of portable-experience scenes, sent on subscribe and on every change.
    GetMapPinsStream(RpcStreamSender<Vec<MapPinInfo>>),
    /// Prompts for the password of a world being entered (see `ipfs::world_access`).
    GetWorldAccessStream(RpcStreamSender<WorldAccessEvent>),
    /// Answers the password prompt: the password, or `None` when the player gives up.
    SetWorldPassword(Option<world_access::WorldPassword>),
    /// The signed-in wallet's platform ban: now, then again on every change.
    GetPlatformBanStream(RpcStreamSender<PlatformBanNotice>),
    SetInteractableArea(Vec4),
    GetMicState(RpcResultSender<MicState>),
    SetMicEnabled(bool),
    /// Trusted HUD recovery request; terminal access decisions remain unchanged.
    RetryConnectivity,
    /// The current safe snapshot, followed by changes until the consumer closes.
    GetConnectivityStream(RpcStreamSender<ConnectivitySnapshot>),
    GetAvatarModifiers(RpcResultSender<Vec<AvatarModifierState>>),
    // Social / Friends
    GetFriends(RpcResultSender<Vec<FriendData>>),
    GetSentFriendRequests(RpcResultSender<Vec<FriendRequestData>>),
    GetReceivedFriendRequests(RpcResultSender<Vec<FriendRequestData>>),
    GetSocialInitialized(RpcResultSender<bool>),
    SendFriendRequest(String, Option<String>, RpcResultSender<Result<(), String>>),
    AcceptFriendRequest(String, RpcResultSender<Result<(), String>>),
    RejectFriendRequest(String, RpcResultSender<Result<(), String>>),
    CancelFriendRequest(String, RpcResultSender<Result<(), String>>),
    DeleteFriend(String, RpcResultSender<Result<(), String>>),
    GetFriendshipEventStream(RpcStreamSender<FriendshipEventUpdate>),
    GetMutualFriends(String, RpcResultSender<Vec<FriendData>>),
    GetOnlineFriends(RpcResultSender<Vec<FriendStatusData>>),
    GetFriendConnectivityStream(RpcStreamSender<FriendConnectivityEvent>),
    // Social / Blocking
    BlockUser(String, RpcResultSender<Result<(), String>>),
    UnblockUser(String, RpcResultSender<Result<(), String>>),
    GetBlockedUsers(RpcResultSender<Vec<BlockedUserData>>),
    GetBlockingStatus(RpcResultSender<Result<BlockingStatusData, String>>),
    GetBlockUpdateStream(RpcStreamSender<BlockUpdateData>),
    // Social / DMs
    /// Emits the given recipient's DM state now and whenever it changes, until dropped.
    GetDmUserStateStream(String, RpcStreamSender<DmUserStateData>),
    /// The stored DMs with a partner, oldest first.
    GetDmHistory(String, RpcResultSender<Vec<DmHistoryEntryData>>),
    /// Deletes the stored DMs with a partner.
    DeleteDmHistory(String),
    // Social / Communities
    /// A community member came online or went offline, until dropped.
    GetCommunityConnectivityStream(RpcStreamSender<CommunityMemberConnectivityData>),
    /// The communities whose chat channels the local user listens to (member of, chat not
    /// closed); replaces the previous set.
    SetCommunityChannels(Vec<ListenedCommunity>),
    /// Replaces the text of the chat bubble still showing the message `message_id`: its
    /// translation, or the original back. A message no bubble shows is ignored.
    SetChatBubbleText {
        message_id: String,
        message: String,
    },
    /// Puts a nametag on one of the system scene's avatars (`entity`, its `AvatarShape` entity as
    /// the scene numbers it, read in the system scene's own context), or takes it away (`None`).
    /// Only the system scene can ask (it is a `SystemApi`). The tag goes with the `AvatarShape`.
    SetAvatarShapeNametag {
        entity: u32,
        nametag: Option<AvatarShapeNametag>,
    },
    // Voice chat (private calls and community streams)
    /// Emits the voice chat state now and whenever it changes, until dropped.
    GetVoiceChatStateStream(RpcStreamSender<VoiceChatState>),
    /// Emits every community voice chat started / ended broadcast, until dropped.
    GetCommunityVoiceUpdateStream(RpcStreamSender<CommunityVoiceUpdate>),
    /// A voice chat command from the HUD; its outcome shows on the state stream.
    VoiceChatAction(VoiceChatAction),
}

#[derive(Resource)]
pub struct NativeUi {
    pub login: bool,
    pub chat: bool,
    pub loading_scene: bool,
}

#[derive(Resource)]
pub struct SystemBridge {
    pub sender: tokio::sync::mpsc::UnboundedSender<SystemApi>,
    receiver: tokio::sync::mpsc::UnboundedReceiver<SystemApi>,
}

pub fn post_events(
    mut bridge: ResMut<SystemBridge>,
    mut writer: EventWriter<SystemApi>,
    mut console: EventWriter<ConsoleCommandEntered>,
    console_config: Res<ConsoleConfiguration>,
) {
    while let Ok(ev) = bridge.receiver.try_recv() {
        let (cmd, args, sender) = match ev {
            SystemApi::ConsoleCommand(cmd, args, sender) => (cmd, args, sender),
            SystemApi::ConsoleLine(line, sender) => {
                match console::line::split(&line, &console_config) {
                    Ok((cmd, args)) => (cmd, args, sender),
                    Err(e) => {
                        sender.send(Err(e));
                        continue;
                    }
                }
            }
            ev => {
                debug!("system bridge: {ev:?}");
                writer.write(ev);
                continue;
            }
        };

        debug!("system bridge (cc): {cmd} {args:?}");

        // Dispatch as a `ConsoleCommandEntered` carrying its own responder, so the result
        // flows back through the channel rather than being scraped from shared console
        // output. `bevy_console` queues concurrent invocations and refires any it can't
        // process this frame, so there is no need to serialize dispatch here.
        if console_config.commands.contains_key(cmd.as_str()) {
            let responder: ConsoleResponder = Arc::new(move |result| sender.send(result));
            console.write(ConsoleCommandEntered {
                command_name: cmd,
                args,
                responder: Some(responder),
            });
        } else {
            sender.send(Err(format!(
                "Command not recognized: `{cmd}`. Recognized commands: {:?}",
                console_config.commands.keys().collect::<Vec<_>>()
            )));
        }
    }
}

fn handle_satellite_view(mut ev: EventReader<SystemApi>, realm: Res<CurrentRealm>) {
    for ev in ev.read() {
        if let SystemApi::GetSatelliteView(rpc_result_sender) = ev {
            rpc_result_sender.send(
                realm
                    .config
                    .map
                    .as_ref()
                    .and_then(|map| map.satellite_view.clone()),
            );
        }
    }
}

fn handle_service_urls(mut ev: EventReader<SystemApi>) {
    for ev in ev.read() {
        if let SystemApi::GetServiceUrls(sender) = ev {
            sender.send(common::base_domain::hud_services());
        }
    }
}

fn handle_exit(mut ev: EventReader<SystemApi>, mut exit: EventWriter<AppExit>) {
    if ev
        .read()
        .filter(|e| matches!(e, SystemApi::Quit))
        .last()
        .is_some()
    {
        exit.write_default();
    }
}
