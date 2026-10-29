use anyhow::anyhow;
use bevy::{log::debug, math::Vec4};
use common::{
    inputs::{Action, BindingsData, HudPanel, InputIdentifier, SystemActionEvent},
    profile::SerializedProfile,
    rpc::{RpcCall, RpcResultReceiver, RpcResultSender, RpcStreamReceiver, RpcStreamSender},
    structs::{
        MicState, PermissionLevel, PermissionStrings, PermissionType, PermissionUsed,
        PermissionValue,
    },
};
use dcl_component::proto_components::common::Vector2;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc, sync::Arc};
use strum::IntoEnumIterator;
use system_bridge::{
    settings::SettingInfo, AnalyticsEvent, AvatarModifierState, BlockUpdateData, BlockedUserData,
    BlockingStatusData, ChatMessage, ChatReactionData, DmHistoryEntryData, DmUserStateData,
    FriendConnectivityEvent, FriendData, FriendRequestData, FriendStatusData,
    FriendshipEventUpdate, HomeScene, HoverEvent, LiveSceneInfo, PermanentPermissionItem,
    PermissionRequestEvent, ProfileChangedEvent, ProximityEvent, SatelliteView, SceneLoadingUi,
    SetAvatarData, SetPermanentPermission, SetSinglePermission, SystemApi, VoiceMessage,
    WelcomeState, WorldAccessEvent,
};
use tokio::sync::Notify;

use crate::player_identity;
use dcl_types::{interface::crdt_context::CrdtContext, RpcCalls};

use super::{State, SuperUserScene};

pub async fn op_check_for_update(
    state: Rc<RefCell<impl State>>,
) -> Result<(String, String), anyhow::Error> {
    debug!("op_check_for_update");
    let (sx, rx) = RpcResultSender::channel();
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::CheckForUpdate(sx))?;

    Ok(rx
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .unwrap_or_default())
}

pub async fn op_motd(state: Rc<RefCell<impl State>>) -> Result<String, anyhow::Error> {
    debug!("op_motd");
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::MOTD(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub fn op_get_current_login(state: &impl State) -> Option<String> {
    player_identity(state)
        .map(|id| Some(id.address))
        .unwrap_or(None)
}

pub async fn op_get_previous_login(
    state: Rc<RefCell<impl State>>,
) -> Result<Option<String>, anyhow::Error> {
    debug!("op_get_previous_login");
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetPreviousLogin(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_login_previous(state: Rc<RefCell<impl State>>) -> Result<(), anyhow::Error> {
    debug!("op_login_previous");
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::LoginPrevious(false, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

#[derive(Default)]
pub struct NewLogin {
    code: Option<RpcResultReceiver<Result<Option<i32>, String>>>,
    result: Option<RpcResultReceiver<Result<(), String>>>,
}

pub fn new_login(state: &mut impl State) -> &mut NewLogin {
    if !state.has::<NewLogin>() {
        state.put(NewLogin::default());
    }

    let mut login = state.take::<NewLogin>();

    if login.code.is_none() && login.result.is_none() {
        let (sc, code) = RpcResultSender::channel();
        let (sx, result) = RpcResultSender::channel();
        state
            .borrow_mut::<SuperUserScene>()
            .send(SystemApi::LoginNew(false, sc, sx))
            .unwrap();

        login.code = Some(code);
        login.result = Some(result);
    }

    state.put(login);
    state.borrow_mut()
}

pub async fn op_login_new_code(
    state: Rc<RefCell<impl State>>,
) -> Result<Option<String>, anyhow::Error> {
    debug!("op_login_new_code");

    let rx = {
        let mut state = state.borrow_mut();
        let login = new_login(&mut *state);
        login.code.take().unwrap()
    };

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
        .map(|code| code.map(|c| format!("{c}")))
}

pub async fn op_login_new_success(state: Rc<RefCell<impl State>>) -> Result<(), anyhow::Error> {
    debug!("op_login_new_success");

    let rx = {
        let mut state = state.borrow_mut();
        let login = new_login(&mut *state);
        login.result.take().unwrap()
    };

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn op_login_guest(state: &mut impl State) {
    debug!("op_login_guest");
    state
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::LoginGuest)
        .unwrap();
}

pub fn op_login_cancel(state: &mut impl State) {
    debug!("op_login_cancel");
    state
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::LoginCancel)
        .unwrap();
}

pub fn op_logout(state: &mut impl State) {
    debug!("op_logout");
    state
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::Logout)
        .unwrap();
}

pub async fn op_settings(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<SettingInfo>, anyhow::Error> {
    debug!("op_settings");
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetSettings(sx))?;

    let res = rx.await?;
    Ok(res)
}

pub async fn op_set_setting(
    state: Rc<RefCell<impl State>>,
    name: String,
    val: f32,
) -> Result<(), anyhow::Error> {
    debug!("op_set_setting");

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetSetting(name, val))?;

    Ok(())
}

pub async fn op_kernel_fetch_headers(
    state: Rc<RefCell<impl State>>,
    uri: String,
    method: Option<String>,
    meta: Option<String>,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    debug!("op_kernel_fetch_headers");

    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SignRequest {
            method: method.unwrap_or_else(|| String::from("get")),
            uri,
            meta,
            response: sx,
        })?;

    rx.await?.map_err(|e| anyhow!(e))
}

pub async fn op_set_avatar(
    state: Rc<RefCell<impl State>>,
    avatar: SetAvatarData,
) -> Result<u32, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetAvatar(avatar, sx))?;

    rx.await?.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_native_input(state: Rc<RefCell<impl State>>) -> String {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetNativeInput(sx))
        .unwrap();

    let identifier = rx.await.unwrap();
    serde_json::to_string(&identifier)
        .unwrap()
        .strip_prefix("\"")
        .unwrap()
        .strip_suffix("\"")
        .unwrap()
        .to_owned()
}

#[derive(Serialize, Deserialize)]
pub struct JsBindingsData {
    bindings: Vec<(Action, Vec<InputIdentifier>)>,
}

pub async fn op_get_bindings(
    state: Rc<RefCell<impl State>>,
) -> Result<JsBindingsData, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetBindings(sx))
        .unwrap();

    rx.await.map_err(|e| anyhow::anyhow!(e)).map(|bd| {
        let mut bindings: Vec<_> = bd.bindings.into_iter().collect();
        bindings.sort_by_key(|k| k.0);
        JsBindingsData { bindings }
    })
}

pub async fn op_set_bindings(
    state: Rc<RefCell<impl State>>,
    bindings: JsBindingsData,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    let bindings = BindingsData {
        bindings: bindings.bindings.into_iter().collect(),
    };

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetBindings(bindings, sx))
        .unwrap();

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub fn op_set_ui_focus(
    state: Rc<RefCell<impl State>>,
    ui: bool,
    text: bool,
    scroll: bool,
    covered: bool,
    menu: Option<HudPanel>,
) -> Result<(), anyhow::Error> {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetUiFocus {
            ui,
            text,
            scroll,
            covered,
            menu,
        })?;
    Ok(())
}

pub async fn op_console_command(
    state: Rc<RefCell<impl State>>,
    cmd: String,
    args: Vec<String>,
) -> Result<String, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::ConsoleCommand(format!("/{cmd}"), args, sx))
        .unwrap();

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_live_scene_info(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<LiveSceneInfo>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::LiveSceneInfo(sx))
        .unwrap();

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_home_scene(state: Rc<RefCell<impl State>>) -> Result<HomeScene, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetHomeScene(sx))
        .unwrap();

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_welcome(state: Rc<RefCell<impl State>>) -> Result<WelcomeState, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetWelcome(sx))
        .unwrap();

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_accept_terms(state: Rc<RefCell<impl State>>) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::AcceptTerms(sx))
        .unwrap();

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_satellite_view(
    state: Rc<RefCell<impl State>>,
) -> Result<Option<SatelliteView>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetSatelliteView(sx))
        .unwrap();

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub fn op_set_home_scene(state: Rc<RefCell<impl State>>, realm: String, parcel: Vector2) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetHomeScene(HomeScene { realm, parcel }))
        .unwrap();
}

/// A scene's open engine streams of one item type, by stream id. A scene may open the same
/// stream more than once (each consumer gets its own receiver, and the engine sends every item to
/// all of them), so receivers are keyed by the id `op_get_*` returns rather than stored by type.
/// A receiver is out of `receivers` while a read awaits it; `notifies` holds an entry for every
/// open stream, and closing removes it and wakes a parked read so the receiver drops and the
/// engine sees the stream closed.
/// An entry lives until its stream ends or is closed, and only DM streams can be closed. A consumer
/// must keep reading (as the bridge scene's `relay` does): one that drops its iterator early
/// leaves the receiver here, filling to the channel's cap while the engine keeps sending to it,
/// for the life of the scene.
struct SystemStreams<T> {
    next_id: u32,
    receivers: HashMap<u32, RpcStreamReceiver<T>>,
    notifies: HashMap<u32, Arc<Notify>>,
}

impl<T> Default for SystemStreams<T> {
    fn default() -> Self {
        Self {
            next_id: 0,
            receivers: HashMap::default(),
            notifies: HashMap::default(),
        }
    }
}

fn open_stream<T: 'static>(state: &mut impl State, rx: RpcStreamReceiver<T>) -> u32 {
    if !state.has::<SystemStreams<T>>() {
        state.put(SystemStreams::<T>::default());
    }
    let streams = state.borrow_mut::<SystemStreams<T>>();
    streams.next_id += 1;
    let rid = streams.next_id;
    streams.receivers.insert(rid, rx);
    streams.notifies.insert(rid, Arc::new(Notify::new()));
    rid
}

/// Ends a stream whose read was dropped mid-await (the receiver went with the read), so the
/// cancelled read leaves no entry behind.
struct ParkedRead<S: State, T: 'static> {
    state: Rc<RefCell<S>>,
    rid: u32,
    finished: bool,
    _item: PhantomData<T>,
}

impl<S: State, T: 'static> Drop for ParkedRead<S, T> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Ok(mut state) = self.state.try_borrow_mut() {
            if let Some(streams) = state.try_borrow_mut::<SystemStreams<T>>() {
                streams.notifies.remove(&self.rid);
            }
        }
    }
}

/// The stream's next item, or None once it has ended (closed, or the engine dropped its sender),
/// or if `rid` is unknown or already being read.
async fn read_stream<T: 'static>(state: Rc<RefCell<impl State>>, rid: u32) -> Option<T> {
    let (mut receiver, notify) = state
        .borrow_mut()
        .try_borrow_mut::<SystemStreams<T>>()
        .and_then(|streams| {
            let receiver = streams.receivers.remove(&rid)?;
            let notify = streams.notifies.get(&rid)?.clone();
            Some((receiver, notify))
        })?;

    let mut parked = ParkedRead::<_, T> {
        state: state.clone(),
        rid,
        finished: false,
        _item: PhantomData,
    };
    let res = tokio::select! {
        item = receiver.recv() => item,
        _ = notify.notified() => None,
    };
    parked.finished = true;

    let mut state = state.borrow_mut();
    let streams = state.borrow_mut::<SystemStreams<T>>();
    if !streams.notifies.contains_key(&rid) {
        // closed while the read was parked: drop the receiver
        return None;
    }
    match res {
        Some(_) => {
            streams.receivers.insert(rid, receiver);
        }
        // the engine dropped its sender
        None => {
            streams.notifies.remove(&rid);
        }
    }
    res
}

fn close_stream<T: 'static>(state: &mut impl State, rid: u32) {
    if let Some(streams) = state.try_borrow_mut::<SystemStreams<T>>() {
        streams.receivers.remove(&rid);
        if let Some(notify) = streams.notifies.remove(&rid) {
            notify.notify_one();
        }
    }
}

pub async fn op_get_system_action_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetSystemActionStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_system_action_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<SystemActionEvent>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_chat_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetChatStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_chat_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<ChatMessage>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub fn op_send_chat(state: Rc<RefCell<impl State>>, message: String, channel: String) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SendChat(message, channel))
        .unwrap();
}

pub async fn op_get_chat_reaction_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetChatReactionStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_chat_reaction_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<ChatReactionData>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub fn op_send_chat_reaction(
    state: Rc<RefCell<impl State>>,
    channel: String,
    message_id: String,
    emoji: String,
    remove: bool,
) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SendChatReaction {
            channel,
            message_id,
            emoji,
            remove,
        })
        .unwrap();
}

// Native super-user bridge transport (backs a BroadcastChannel polyfill). scene -> page:
pub fn op_bridge_to_page(state: Rc<RefCell<impl State>>, msg: String) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::BridgeToPage(msg))
        .unwrap();
}

// page -> scene: subscribe once, then read repeatedly (mirrors the chat stream).
pub async fn op_get_bridge_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetBridgeStream(sx))
        .unwrap();
    rid
}

pub async fn op_read_bridge_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<String, anyhow::Error> {
    // "" means no message (stream closed); Envelopes are JSON objects so never empty.
    Ok(read_stream(state, rid).await.unwrap_or_default())
}

/// Any user's full profile (own, nearby, or remote) as the engine holds it, resolved through the
/// same cache and fetch cascade as nametags and `getPlayerData` — but unlike the latter, not
/// squeezed into the SDK's `UserData`, so `extra_fields`, `name_color` and emotes survive.
/// Rejects once the cascade has concluded with nothing.
pub async fn op_get_user_profile(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<SerializedProfile, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    let scene = state.borrow().borrow::<CrdtContext>().scene_id.0;
    debug!("[{scene:?}] -> op_get_user_profile {address}");

    state
        .borrow_mut()
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::GetUserData {
            user: Some(address),
            response: sx,
        })?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|_| anyhow::anyhow!("Not found"))
}

pub fn op_quit(state: Rc<RefCell<impl State>>) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::Quit)
        .unwrap();
}

pub async fn op_get_permission_request_stream(state: Rc<RefCell<impl State>>) -> u32 {
    debug!("op_get_permission_request_stream");
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetPermissionRequestStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_permission_request_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<PermissionRequestEvent>, anyhow::Error> {
    debug!("op_read_permission_request_stream");
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_permission_used_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetPermissionUsedStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_permission_used_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<PermissionUsed>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub fn op_set_single_permission(state: Rc<RefCell<impl State>>, id: usize, allow: bool) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetSinglePermission(SetSinglePermission {
            id,
            allow,
        }))
        .unwrap();
}

pub async fn op_get_world_access_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetWorldAccessStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_world_access_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<WorldAccessEvent>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub fn op_set_world_password(state: Rc<RefCell<impl State>>, password: Option<String>) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetWorldPassword(
            password.map(system_bridge::world_access::WorldPassword),
        ))
        .unwrap();
}

fn get_permanent_level(
    level: &str,
    value: Option<String>,
) -> Result<PermissionLevel, anyhow::Error> {
    Ok(match level {
        "Realm" => PermissionLevel::Realm(value.ok_or(anyhow!("Realm value must be specified"))?),
        "Scene" => PermissionLevel::Scene(value.ok_or(anyhow!("Scene value must be specified"))?),
        "Global" => PermissionLevel::Global,
        _ => anyhow::bail!("invalid level {level}, must be `Realm`, `Scene` or `Global`"),
    })
}

pub fn op_set_permanent_permission(
    state: Rc<RefCell<impl State>>,
    level: &str,
    value: Option<String>,
    permission_type: PermissionType,
    allow: Option<PermissionValue>,
) -> Result<(), anyhow::Error> {
    let level = get_permanent_level(level, value)?;

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetPermanentPermission(SetPermanentPermission {
            ty: permission_type,
            level,
            allow,
        }))
        .unwrap();

    Ok(())
}

pub async fn op_get_permanent_permissions(
    state: Rc<RefCell<impl State>>,
    level: &str,
    value: Option<String>,
) -> Result<Vec<PermanentPermissionItem>, anyhow::Error> {
    let level = get_permanent_level(level, value)?;
    let (sx, result) = RpcResultSender::channel();
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetPermanentPermissions(level, sx))?;

    Ok(result.await?)
}

#[derive(Serialize, Deserialize)]
pub struct PermissionTypeDetail {
    ty: PermissionType,
    name: String,
    passive: String,
    active: String,
}

pub fn op_get_permission_types() -> Vec<PermissionTypeDetail> {
    PermissionType::iter()
        .map(|ty| PermissionTypeDetail {
            ty,
            name: ty.title().to_owned(),
            passive: ty.passive().to_owned(),
            active: ty.active().to_owned(),
        })
        .collect()
}

pub fn op_set_interactable_area(
    state: Rc<RefCell<impl State>>,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) {
    let _ = state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetInteractableArea(Vec4::new(
            left, top, right, bottom,
        )));
}

pub async fn op_set_mic_enabled(state: Rc<RefCell<impl State>>, enabled: bool) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SetMicEnabled(enabled))
        .unwrap();
}

pub async fn op_get_mic_state(state: Rc<RefCell<impl State>>) -> Result<MicState, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetMicState(sx))
        .unwrap();

    Ok(rx.await?)
}

pub async fn op_get_voice_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetVoiceStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_voice_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<VoiceMessage>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_hover_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetHoverStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_hover_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<HoverEvent>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_proximity_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetProximityStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_proximity_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<ProximityEvent>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_profile_changed_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetProfileChangedStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_profile_changed_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<ProfileChangedEvent>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_scene_loading_ui_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetSceneLoadingUiStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_scene_loading_ui_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<SceneLoadingUi>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_analytics_event_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetAnalyticsEventStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_analytics_event_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<AnalyticsEvent>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_avatar_modifiers(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<AvatarModifierState>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetAvatarModifiers(sx))?;

    Ok(rx.await?)
}

// Social / Friends ops

pub async fn op_get_friendship_event_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetFriendshipEventStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_friendship_event_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<FriendshipEventUpdate>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_get_friends(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<FriendData>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetFriends(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_mutual_friends(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<Vec<FriendData>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetMutualFriends(address, sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_sent_friend_requests(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<FriendRequestData>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetSentFriendRequests(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_received_friend_requests(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<FriendRequestData>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetReceivedFriendRequests(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_social_initialized(
    state: Rc<RefCell<impl State>>,
) -> Result<bool, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetSocialInitialized(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_online_friends(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<FriendStatusData>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetOnlineFriends(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_friend_connectivity_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetFriendConnectivityStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_friend_connectivity_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<FriendConnectivityEvent>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

pub async fn op_send_friend_request(
    state: Rc<RefCell<impl State>>,
    address: String,
    message: Option<String>,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::SendFriendRequest(address, message, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_accept_friend_request(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::AcceptFriendRequest(address, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_reject_friend_request(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::RejectFriendRequest(address, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_cancel_friend_request(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::CancelFriendRequest(address, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_delete_friend(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::DeleteFriend(address, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_block_user(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::BlockUser(address, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_unblock_user(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<(), anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::UnblockUser(address, sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_blocked_users(
    state: Rc<RefCell<impl State>>,
) -> Result<Vec<BlockedUserData>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetBlockedUsers(sx))?;

    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub async fn op_get_blocking_status(
    state: Rc<RefCell<impl State>>,
) -> Result<BlockingStatusData, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetBlockingStatus(sx))?;

    rx.await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| anyhow::anyhow!(e))
}

/// DM state of one user. Several are open at once (one per conversation the HUD shows), and
/// unlike the other streams the HUD drops them as conversations close, so they can be closed.
pub fn op_get_dm_user_state_stream(state: Rc<RefCell<impl State>>, address: String) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();

    let mut state = state.borrow_mut();
    let rid = open_stream(&mut *state, rx);

    state
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetDmUserStateStream(address, sx))
        .unwrap();

    rid
}

pub async fn op_read_dm_user_state_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<DmUserStateData>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

/// Ends the stream: wakes a parked read, and drops the receiver so the engine's sender closes.
pub fn op_close_dm_user_state_stream(state: Rc<RefCell<impl State>>, rid: u32) {
    close_stream::<DmUserStateData>(&mut *state.borrow_mut(), rid);
}

pub async fn op_get_dm_history(
    state: Rc<RefCell<impl State>>,
    address: String,
) -> Result<Vec<DmHistoryEntryData>, anyhow::Error> {
    let (sx, rx) = RpcResultSender::channel();
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetDmHistory(address, sx))?;
    rx.await.map_err(|e| anyhow::anyhow!(e))
}

pub fn op_delete_dm_history(state: Rc<RefCell<impl State>>, address: String) {
    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::DeleteDmHistory(address))
        .unwrap();
}

pub async fn op_get_block_update_stream(state: Rc<RefCell<impl State>>) -> u32 {
    let (sx, rx) = RpcStreamSender::channel();
    let rid = open_stream(&mut *state.borrow_mut(), rx);

    state
        .borrow_mut()
        .borrow_mut::<SuperUserScene>()
        .send(SystemApi::GetBlockUpdateStream(sx))
        .unwrap();

    rid
}

pub async fn op_read_block_update_stream(
    state: Rc<RefCell<impl State>>,
    rid: u32,
) -> Result<Option<BlockUpdateData>, anyhow::Error> {
    Ok(read_stream(state, rid).await)
}

#[cfg(test)]
mod kernel_fetch_tests {
    use super::*;
    use crate::test_state::TestState;
    use futures_lite::future::{block_on, poll_once};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn fetch(
        state: &Rc<RefCell<TestState>>,
    ) -> impl std::future::Future<Output = Result<Vec<(String, String)>, anyhow::Error>> {
        op_kernel_fetch_headers(
            state.clone(),
            "https://example.com".to_owned(),
            None,
            Some("{}".to_owned()),
        )
    }

    #[test]
    fn goes_to_the_system_channel() {
        let (sx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut state = TestState::default();
        state.put(RpcCalls::default());
        state.put(SuperUserScene(sx));
        let state = Rc::new(RefCell::new(state));

        assert!(block_on(poll_once(fetch(&state))).is_none());
        assert!(matches!(
            rx.try_recv(),
            Ok(SystemApi::SignRequest { meta: Some(meta), .. }) if meta == "{}"
        ));
        assert!(state.borrow().borrow::<RpcCalls>().is_empty());
    }

    // an ordinary scene's state has no system channel
    #[test]
    fn signs_nothing_without_the_system_channel() {
        let mut state = TestState::default();
        state.put(RpcCalls::default());
        let state = Rc::new(RefCell::new(state));

        let result = catch_unwind(AssertUnwindSafe(|| block_on(poll_once(fetch(&state)))));
        assert!(result.is_err());
        assert!(state.borrow().borrow::<RpcCalls>().is_empty());
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use crate::test_state::TestState;
    use futures_lite::future::{block_on, poll_once};
    use tokio::sync::mpsc::UnboundedReceiver;

    fn system_scene() -> (Rc<RefCell<TestState>>, UnboundedReceiver<SystemApi>) {
        let (sx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut state = TestState::default();
        state.put(SuperUserScene(sx));
        (Rc::new(RefCell::new(state)), rx)
    }

    // what the engine does with each request: keep the sender
    fn engine_sender(
        system: &mut UnboundedReceiver<SystemApi>,
    ) -> RpcStreamSender<ProfileChangedEvent> {
        match system.try_recv() {
            Ok(SystemApi::GetProfileChangedStream(sender)) => sender,
            _ => panic!("expected a profile changed stream request"),
        }
    }

    fn event(version: u32) -> ProfileChangedEvent {
        ProfileChangedEvent {
            address: "0x1".to_owned(),
            version,
        }
    }

    fn read(state: &Rc<RefCell<TestState>>, rid: u32) -> Option<ProfileChangedEvent> {
        block_on(op_read_profile_changed_stream(state.clone(), rid)).unwrap()
    }

    #[test]
    fn each_subscription_reads_only_its_own_receiver() {
        let (state, mut system) = system_scene();
        let a = block_on(op_get_profile_changed_stream(state.clone()));
        let b = block_on(op_get_profile_changed_stream(state.clone()));
        assert_ne!(a, b);
        let senders = [engine_sender(&mut system), engine_sender(&mut system)];

        for version in 1..=3 {
            for sender in &senders {
                sender.send(event(version)).unwrap();
            }
        }
        for version in 1..=3 {
            assert_eq!(read(&state, a), Some(event(version)));
            assert_eq!(read(&state, b), Some(event(version)));
        }
    }

    // one subscription's read parked on an empty stream must not take the other's receiver
    #[test]
    fn a_parked_read_leaves_other_subscriptions_alone() {
        let (state, mut system) = system_scene();
        let a = block_on(op_get_profile_changed_stream(state.clone()));
        let b = block_on(op_get_profile_changed_stream(state.clone()));
        let sender_a = engine_sender(&mut system);
        let sender_b = engine_sender(&mut system);

        let mut read_a = Box::pin(op_read_profile_changed_stream(state.clone(), a));
        assert!(block_on(poll_once(&mut read_a)).is_none());

        sender_b.send(event(1)).unwrap();
        assert_eq!(read(&state, b), Some(event(1)));

        sender_a.send(event(2)).unwrap();
        assert_eq!(block_on(read_a).unwrap(), Some(event(2)));
    }

    #[test]
    fn a_stream_ends_alone() {
        let (state, mut system) = system_scene();
        let a = block_on(op_get_profile_changed_stream(state.clone()));
        let b = block_on(op_get_profile_changed_stream(state.clone()));
        let sender_a = engine_sender(&mut system);
        let sender_b = engine_sender(&mut system);

        // the engine sends, then drops: the last item is still read before the end
        sender_a.send(event(1)).unwrap();
        drop(sender_a);
        assert_eq!(read(&state, a), Some(event(1)));
        assert_eq!(read(&state, a), None);
        assert_eq!(read(&state, a), None);

        sender_b.send(event(1)).unwrap();
        assert_eq!(read(&state, b), Some(event(1)));
    }

    #[test]
    fn a_dropped_read_ends_its_stream() {
        let (state, mut system) = system_scene();
        let rid = block_on(op_get_profile_changed_stream(state.clone()));
        let sender = engine_sender(&mut system);

        let mut parked = Box::pin(op_read_profile_changed_stream(state.clone(), rid));
        assert!(block_on(poll_once(&mut parked)).is_none());
        drop(parked);

        assert!(sender.is_closed());
        let state_ref = state.borrow();
        let streams = state_ref.borrow::<SystemStreams<ProfileChangedEvent>>();
        assert!(streams.receivers.is_empty());
        assert!(streams.notifies.is_empty());
    }

    #[test]
    fn closing_wakes_a_parked_read_and_drops_the_receiver() {
        let (state, mut system) = system_scene();
        let rid = op_get_dm_user_state_stream(state.clone(), "0x1".to_owned());
        let Ok(SystemApi::GetDmUserStateStream(_, sender)) = system.try_recv() else {
            panic!("expected a dm user state stream request");
        };

        let mut parked = Box::pin(op_read_dm_user_state_stream(state.clone(), rid));
        assert!(block_on(poll_once(&mut parked)).is_none());

        op_close_dm_user_state_stream(state.clone(), rid);
        assert!(block_on(parked).unwrap().is_none());
        assert!(sender.is_closed());
    }
}
